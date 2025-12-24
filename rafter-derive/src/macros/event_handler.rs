//! The `#[event_handler]` attribute macro for event subscriber methods.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{FnArg, ItemFn, Pat, Type, parse2};

/// Extract the event type from a function's parameters.
///
/// Looks for the first non-self parameter that isn't an AppContext.
fn extract_event_type(func: &ItemFn) -> Option<Type> {
    for arg in &func.sig.inputs {
        if let FnArg::Typed(pat_type) = arg {
            let ty = &pat_type.ty;
            let ty_str = quote!(#ty).to_string();

            // Skip self and context parameters
            if ty_str.contains("AppContext") {
                continue;
            }

            // Skip if it's self
            if let Pat::Ident(pat) = pat_type.pat.as_ref() {
                if pat.ident == "self" {
                    continue;
                }
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

pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let func: ItemFn = match parse2(item) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error(),
    };

    // Validate async
    if func.sig.asyncness.is_none() {
        return syn::Error::new_spanned(&func.sig, "event_handler must be async")
            .to_compile_error();
    }

    // Extract event type
    let Some(event_type) = extract_event_type(&func) else {
        return syn::Error::new_spanned(
            &func.sig,
            "event_handler requires an event parameter (first non-self, non-cx parameter)",
        )
        .to_compile_error();
    };

    // Validate cx parameter
    if !has_app_context(&func) {
        return syn::Error::new_spanned(&func.sig, "event_handler requires an &AppContext parameter")
            .to_compile_error();
    }

    // Encode metadata: event type path as string
    let event_type_str = quote!(#event_type).to_string().replace(' ', "");
    let metadata = format!("__rafter_event_handler:{}", event_type_str);

    let vis = &func.vis;
    let sig = &func.sig;
    let block = &func.block;
    let other_attrs: Vec<_> = func
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("event_handler"))
        .collect();

    quote! {
        #[doc = #metadata]
        #(#other_attrs)*
        #vis #sig #block
    }
}
