//! Common utilities for handler validation.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{FnArg, ItemFn, Pat, ReturnType};

/// Check if function has an AppContext parameter.
pub fn has_app_context(func: &ItemFn) -> bool {
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

/// Check if function has a ModalContext parameter.
pub fn has_modal_context(func: &ItemFn) -> bool {
    for arg in &func.sig.inputs {
        if let FnArg::Typed(pat_type) = arg {
            let ty_str = quote!(#pat_type.ty).to_string();
            if ty_str.contains("ModalContext") {
                return true;
            }
        }
    }
    false
}

/// Extract the message type (event/request) from function parameters.
/// Returns the first non-self, non-context parameter type as a string.
pub fn extract_message_type(func: &ItemFn) -> Option<String> {
    for arg in &func.sig.inputs {
        if let FnArg::Typed(pat_type) = arg {
            let ty = &pat_type.ty;
            let ty_str = quote!(#ty).to_string();

            // Skip context parameters
            if ty_str.contains("AppContext") || ty_str.contains("ModalContext") {
                continue;
            }

            // Skip self
            if let Pat::Ident(pat) = pat_type.pat.as_ref() {
                if pat.ident == "self" {
                    continue;
                }
            }

            return Some(ty_str.replace(' ', ""));
        }
    }
    None
}

/// Check if function has a return type.
pub fn has_return_type(func: &ItemFn) -> bool {
    matches!(&func.sig.output, ReturnType::Type(_, _))
}

/// Validate a message handler (event or request).
/// - Must be async
/// - Must have a message parameter
/// - Must have AppContext parameter
/// - If `requires_return`, must have a return type
pub fn validate_message_handler(func: &ItemFn, requires_return: bool) -> Result<(), syn::Error> {
    let handler_kind = if requires_return {
        "request_handler"
    } else {
        "event_handler"
    };

    // Validate async
    if func.sig.asyncness.is_none() {
        return Err(syn::Error::new_spanned(
            &func.sig,
            format!("{} must be async", handler_kind),
        ));
    }

    // Validate message parameter exists
    if extract_message_type(func).is_none() {
        return Err(syn::Error::new_spanned(
            &func.sig,
            format!(
                "{} requires a message parameter (first non-self, non-context parameter)",
                handler_kind
            ),
        ));
    }

    // Validate AppContext parameter
    if !has_app_context(func) {
        return Err(syn::Error::new_spanned(
            &func.sig,
            format!("{} requires an &AppContext parameter", handler_kind),
        ));
    }

    // For request handlers, validate return type
    if requires_return && !has_return_type(func) {
        return Err(syn::Error::new_spanned(
            &func.sig,
            "request_handler must have a return type",
        ));
    }

    Ok(())
}

/// Expand a message handler macro (shared by event_handler and request_handler).
/// Just validates and passes through unchanged.
pub fn expand_message_handler(item: TokenStream, requires_return: bool) -> TokenStream {
    let func: ItemFn = match syn::parse2(item) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error(),
    };

    if let Err(e) = validate_message_handler(&func, requires_return) {
        return e.to_compile_error();
    }

    // Pass through unchanged
    quote! { #func }
}
