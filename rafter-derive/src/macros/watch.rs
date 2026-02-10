//! The `#[watch]` attribute macro for reactive async derived state.
//!
//! Marks an async method to be automatically triggered when its `State<T>`
//! dependencies change. Dependencies are detected by the same AST analysis
//! used by `#[derived]`.
//!
//! This is a validation-only pass-through macro. The actual code generation
//! happens in the `*_impl` macros (app_impl, system_impl, modal_impl)
//! which detect `#[watch]` methods and generate `check_watches()`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{ImplItemFn, parse2};

use super::dep_detection::find_dependencies;

pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let func: ImplItemFn = match parse2(item) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error(),
    };

    // Validate: must be async
    if func.sig.asyncness.is_none() {
        return syn::Error::new_spanned(&func.sig.fn_token, "#[watch] functions must be async")
            .to_compile_error();
    }

    // Validate: must have at least one State dependency
    let deps = find_dependencies(&func);
    if deps.is_empty() {
        return syn::Error::new_spanned(
            &func.sig.fn_token,
            "#[watch] function has no State dependencies - it won't trigger",
        )
        .to_compile_error();
    }

    // Validate: must not have a return type
    if !matches!(func.sig.output, syn::ReturnType::Default) {
        return syn::Error::new_spanned(
            &func.sig.output,
            "#[watch] functions must not have a return type (they are side-effect only)",
        )
        .to_compile_error();
    }

    // Pass through unchanged — *_impl macros handle code generation
    quote! { #func }
}
