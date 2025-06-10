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

            // detect async fn returning Result<_,_>
            let is_async = sig.asyncness.is_some();
            let returns_result = matches!(
                &sig.output,
                syn::ReturnType::Type(_, ty)
                    if matches!(**ty, syn::Type::Path(ref p)
                        if p.path.segments.last().unwrap().ident == "Result")
            );

            // collect args for JSON
            let arg_vals = sig.inputs.iter().filter_map(|arg| {
                if let FnArg::Typed(p) = arg {
                    let pat = &p.pat;
                    Some(quote! { serde_json::to_value(&#pat).unwrap() })
                } else {
                    None
                }
            });

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
                        let json = Self::#helper_fn(
                            format!("http://localhost:6969/{}", #name_str),
                            payload,
                        )
                        .await
                        .unwrap_or_else(|e| panic!("mock HTTP failed: {:?}", e));

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
        impl #self_ty {
            // inlined helper; no extra deps in downstream crate
            async fn #helper_fn(
                url: String,
                body: serde_json::Value,
            ) -> Result<serde_json::Value, String> {
                use ic_cdk::api::management_canister::http_request::{
                    CanisterHttpRequestArgument, HttpHeader, HttpMethod, http_request,
                };
                // no headers for now
                let req = CanisterHttpRequestArgument {
                    url,
                    method: HttpMethod::POST,
                    body: Some(serde_json::to_string(&body).unwrap().into_bytes()),
                    max_response_bytes: None,
                    transform: None,
                    headers: Vec::<HttpHeader>::new(),
                };
                match http_request(req, 200_949_972_000).await {
                    Ok((resp,)) => {
                        let s = String::from_utf8(resp.body)
                            .map_err(|_| "Invalid UTF-8".to_string())?;
                        serde_json::from_str(&s)
                            .map_err(|e| format!("JSON parse error: {:?}", e))
                    }
                    Err((_, m)) => Err(format!("http_request error: {}", m)),
                }
            }

            #(#methods)*
        }
    };

   expanded.into()
}
