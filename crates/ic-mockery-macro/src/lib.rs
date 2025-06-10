// crates/ic-mockery-macro/src/lib.rs

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, FnArg, ImplItem, ItemImpl};

#[proc_macro_attribute]
pub fn mock_async_calls(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the impl block
    let input = parse_macro_input!(item as ItemImpl);
    let self_ty = &input.self_ty;
    // We'll inject the helper once per impl
    let helper_fn = format_ident!("__ic_mockery_http_post_call");

    // Build each method
    let methods = input.items.into_iter().map(|item| {
        if let ImplItem::Fn(method) = item {
            let vis = &method.vis;
            let sig = &method.sig;
            let name_str = sig.ident.to_string();

            let is_async = sig.asyncness.is_some();
            let returns_result = match &sig.output {
                syn::ReturnType::Type(_, ty) => {
                    if let syn::Type::Path(p) = ty.as_ref() {
                        p.path.segments.last().map_or(false, |seg| seg.ident == "Result")
                    } else {
                        false
                    }
                }
                _ => false,
            };


            let arg_vals: Vec<_> = sig
            .inputs
            .iter()
            .filter_map(|arg| {
                match arg {
                    FnArg::Typed(pat) => {
                        let name = &pat.pat;
                        Some(quote! { serde_json::to_value(&#name).unwrap() })
                    }
                    FnArg::Receiver(_) => None, // skip self/&self/&mut self
                }
            })
            .collect();

            if is_async && returns_result {
                quote! {
                    #vis #sig {
                        use serde_json::json;
                        // build payload
                        let payload = json!({
                            "method": #name_str,
                            "args": [#(#arg_vals),*]
                        });
                        // call our injected helper
                        let json = match #helper_fn(
                            format!("http://localhost:6969/{}", #name_str),
                            payload,
                        )
                        .await {
                            Some(val) => val,
                            None => {
                                panic!("Error on fetch")
                            },
                        };

                      let typed = match serde_json::from_value(json.clone()){
                            Ok(v) => Ok(v),
                            Err(e) =>  {
                                panic!("Error on deserialization {:?} \n {:?}",e, json)
                            }
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

    // Emit a private async fn helper inside the same module
    let expanded = quote! {
        // inlined helper; no extra deps in downstream crate
        async fn #helper_fn<T: Serialize + std::fmt::Debug>(
            url: String,
            body: T,
        ) -> Option<serde_json::Value> {
            use ic_cdk::api::management_canister::http_request::{
                CanisterHttpRequestArgument, HttpHeader, HttpMethod, http_request,
            };
            // no headers for now
            let request = CanisterHttpRequestArgument {
                url,
                method: HttpMethod::POST,
                body: Some(serde_json::to_string(&body).unwrap().into_bytes()),
                max_response_bytes: None,
                transform: None,
                headers: Vec::<HttpHeader>::new(),
            };
            match http_request(request, 200_949_972_000).await {
                //See:https://docs.rs/ic-cdk/latest/ic_cdk/api/management_canister/http_request/struct.HttpResponse.html
                Ok((response,)) => {
                    let str_body = String::from_utf8(response.body).expect("Transformed response is not UTF-8 encoded.");
        
                    let parsed: serde_json::Value = serde_json::from_str(&str_body).expect("JSON was not well-formatted");
        
                    Some(parsed)
                }
                Err((_, m)) => {
                    //Return the error as a string and end the method
                    panic!("The http_request resulted into error. Error: {m}")
                }
            }
        }
        impl #self_ty {
            #(#methods)*
        }
    };

    TokenStream::from(expanded)
}
