//! The `#[request_handler]` attribute macro for request responder methods.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{FnArg, ItemFn, Pat, ReturnType, Type, parse2};

/// Extract the request type from a function's parameters.
///
/// Looks for the first non-self parameter that isn't an AppContext.
fn extract_request_type(func: &ItemFn) -> Option<Type> {
    for arg in &func.sig.inputs {
        if let FnArg::Typed(pat_type) = arg {
            let ty = &pat_type.ty;
            let ty_str = quote!(#ty).to_string();

            // Skip self and context parameters
            if ty_str.contains("AppContext") {
                continue;
            }

            // Skip if it's self
            if let Pat::Ident(pat) = pat_type.pat.as_ref()
                && pat.ident == "self" {
                    continue;
                }

            return Some((**ty).clone());
        }
    }
    None
}

/// Validate that the function has an AppContext parameter.
fn has_app_context(func: &ItemFn) -> bool {
    for arg in &func.sig.inputs {
        if let FnArg::Typed(pat_type) = arg {
            let ty_str = quote!(#pat_type.ty).to_string();
            if ty_str.contains("AppContext") {
                return true;
            }
        }
    }
    false
}

/// Extract the return type from the function.
fn extract_return_type(func: &ItemFn) -> Option<Type> {
    if let ReturnType::Type(_, ty) = &func.sig.output {
        Some((**ty).clone())
    } else {
        None
    }
}

pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let func: ItemFn = match parse2(item) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error(),
    };

    // Validate async
    if func.sig.asyncness.is_none() {
        return syn::Error::new_spanned(&func.sig, "request_handler must be async")
            .to_compile_error();
    }

    // Extract request type
    let Some(request_type) = extract_request_type(&func) else {
        return syn::Error::new_spanned(
            &func.sig,
            "request_handler requires a request parameter (first non-self, non-cx parameter)",
        )
        .to_compile_error();
    };

    // Validate cx parameter
    if !has_app_context(&func) {
        return syn::Error::new_spanned(
            &func.sig,
            "request_handler requires an &AppContext parameter",
        )
        .to_compile_error();
    }

    // Validate return type exists
    if extract_return_type(&func).is_none() {
        return syn::Error::new_spanned(&func.sig, "request_handler must have a return type")
            .to_compile_error();
    }

    // Encode metadata: request type path as string
    let request_type_str = quote!(#request_type).to_string().replace(' ', "");
    let metadata = format!("__rafter_request_handler:{}", request_type_str);

    let vis = &func.vis;
    let sig = &func.sig;
    let block = &func.block;
    let other_attrs: Vec<_> = func
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("request_handler"))
        .collect();

    quote! {
        #[doc = #metadata]
        #(#other_attrs)*
        #vis #sig #block
    }
}
