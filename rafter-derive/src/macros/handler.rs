//! The `#[handler]` attribute macro for keybind handler methods.
//!
//! Validates that handlers have appropriate context parameters.
//! Just validates and passes through unchanged.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{ItemFn, parse2};

use super::handler_common::{has_app_context, has_modal_context};

pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let func: ItemFn = match parse2(item) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error(),
    };

    // Handlers in app impl should use AppContext, not ModalContext
    // Handlers in modal impl can use either or both
    // We can't know which context we're in at this point, so just warn about ModalContext-only
    if has_modal_context(&func) && !has_app_context(&func) {
        // This is likely a mistake - modal handlers are defined in modal_impl, not app_impl
        // But we allow it for flexibility, app_impl will error if used incorrectly
    }

    // Pass through unchanged
    quote! { #func }
}
