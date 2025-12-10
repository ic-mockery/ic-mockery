use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, parse_quote, FnArg, GenericArgument, ImplItem, ItemImpl, PathArguments,
    ReturnType, Type,
};

#[proc_macro_attribute]
pub fn mock_async_calls(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemImpl);
    let self_ty = &input.self_ty;
    let helper_fn = format_ident!("__ic_mockery_http_post_call");

    let methods = input.items.into_iter().map(|item| {
        if let ImplItem::Fn(mut method) = item {
            let vis = &method.vis;
            let sig = &mut method.sig;
            let name_str = sig.ident.to_string();

            let is_async = sig.asyncness.is_some();

            let error_type_opt = match &sig.output {
                ReturnType::Type(_, ty) => match ty.as_ref() {
                    Type::Path(type_path) => {
                        let segment = type_path.path.segments.last().unwrap();

                        if segment.ident == "Result" {
                            if let PathArguments::AngleBracketed(args) = &segment.arguments {
                                let mut type_args = args.args.iter();
                                type_args.next();
                                match type_args.next() {
                                    Some(GenericArgument::Type(err_ty)) => Some(err_ty.clone()),
                                    _ => None,
                                }
                            } else {
                                None
                            }
                        } else if segment.ident == "CallResult" {
                            // Known alias: type CallResult<T> = Result<T, (RejectionCode, String)>
                            Some(parse_quote!((RejectionCode, String)))
                        } else {
                            None
                        }
                    }
                    _ => None,
                },
                _ => None,
            };

            let returns_result = error_type_opt.is_some();

            let arg_vals: Vec<_> = sig.inputs.iter().filter_map(|arg| match arg {
                FnArg::Typed(pat) => {
                    let name = &pat.pat;
                    Some(quote! { serde_json::to_value(&#name).unwrap() })
                }
                FnArg::Receiver(_) => None,
            }).collect();

            if is_async && returns_result {
                let json_call = match &error_type_opt {
                    Some(error_ty) => {
                        let is_string = matches!(
                            error_ty,
                            Type::Path(p) if p.path.segments.last().unwrap().ident == "String"
                        );

                        let is_rejection_pair = matches!(
                            error_ty,
                            Type::Tuple(t) if t.elems.len() == 2 &&
                                matches!(&t.elems[0], Type::Path(p) if p.path.segments.last().unwrap().ident == "RejectionCode") &&
                                matches!(&t.elems[1], Type::Path(p) if p.path.segments.last().unwrap().ident == "String")
                        );

                        if is_string {
                            quote! {
                                let json = #helper_fn(
                                    format!("http://localhost:6969/{}", #name_str),
                                    payload,
                                ).await?;
                            }
                        } else if is_rejection_pair {
                            quote! {
                                let json = #helper_fn(
                                    format!("http://localhost:6969/{}", #name_str),
                                    payload,
                                ).await.map_err(|e| (ic_cdk::api::call::RejectionCode::CanisterReject, e))?;
                            }
                        } else {
                            quote! {
                                let json = #helper_fn(
                                    format!("http://localhost:6969/{}", #name_str),
                                    payload,
                                ).await?;
                            }
                        }
                    }
                    None => quote! {
                        let json = #helper_fn(
                            format!("http://localhost:6969/{}", #name_str),
                            payload,
                        ).await?;
                    },
                };

                quote! {
                    #vis #sig {
                        use serde_json::json;

                        let payload = json!({
                            "method": #name_str,
                            "args": [#(#arg_vals),*]
                        });

                        #json_call

                        let typed = match serde_json::from_value(json.clone()) {
                            Ok(v) => Ok(v),
                            Err(e) => panic!("Error on deserialization {:?} \n {:?}", e, json),
                        };

                        typed
                    }
                }
            } else {
                quote! { #method }
            }
        } else {
            quote! { #item }
        }
    });

    let expanded = quote! {
        async fn #helper_fn<T: serde::Serialize + std::fmt::Debug>(
            url: String,
            body: T,
        ) -> std::result::Result<serde_json::Value, String> {
            use ic_cdk::management_canister::{
                http_request, HttpHeader, HttpMethod, HttpRequestArgs,
            };

            let request = HttpRequestArgs {
                url,
                max_response_bytes: None,
                method: HttpMethod::POST,
                headers: Vec::<HttpHeader>::new(),
                body: Some(serde_json::to_vec(&body).expect("Failed to serialize body")),
                transform: None,
                is_replicated: None,
            };

            match http_request(&request).await {
                Ok(response) => {
                    let str_body = String::from_utf8(response.body)
                        .expect("HTTP response body is not UTF-8 encoded");
                    let parsed: serde_json::Value = serde_json::from_str(&str_body)
                        .expect("HTTP response JSON was not well-formatted");
                    Ok(parsed)
                }
                Err(e) => Err(format!("http_request error: {:?}", e,)),
            }
        }

        impl #self_ty {
            #(#methods)*
        }
    };

    TokenStream::from(expanded)
}
