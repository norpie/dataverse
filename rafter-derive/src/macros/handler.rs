use proc_macro2::TokenStream;
use quote::quote;
use syn::{ItemFn, parse2};

/// Handler parameter requirements detected from function signature
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerParams {
    /// No context parameters
    None,
    /// Takes &AppContext only
    AppContext,
    /// Takes &ModalContext<R> only
    ModalContext,
    /// Takes both &AppContext and &ModalContext<R>
    Both,
}

impl HandlerParams {
    /// Check if handler needs AppContext
    pub fn needs_app_context(&self) -> bool {
        matches!(self, Self::AppContext | Self::Both)
    }

    /// Check if handler needs ModalContext
    #[allow(dead_code)] // Used by modal_impl
    pub fn needs_modal_context(&self) -> bool {
        matches!(self, Self::ModalContext | Self::Both)
    }
}

/// Detect what context parameters a function takes
pub fn detect_handler_params(func: &ItemFn) -> HandlerParams {
    let mut has_app_context = false;
    let mut has_modal_context = false;

    for arg in &func.sig.inputs {
        if let syn::FnArg::Typed(pat_type) = arg {
            let ty = &pat_type.ty;
            let ty_str = quote!(#ty).to_string();

            // Check for AppContext (but not ModalContext which also contains "Context")
            if ty_str.contains("AppContext") {
                has_app_context = true;
            }
            // Check for ModalContext<...>
            if ty_str.contains("ModalContext") {
                has_modal_context = true;
            }
        }
    }

    match (has_app_context, has_modal_context) {
        (false, false) => HandlerParams::None,
        (true, false) => HandlerParams::AppContext,
        (false, true) => HandlerParams::ModalContext,
        (true, true) => HandlerParams::Both,
    }
}

/// Encode handler params as a string for metadata
fn encode_params(params: HandlerParams) -> &'static str {
    match params {
        HandlerParams::None => "none",
        HandlerParams::AppContext => "app",
        HandlerParams::ModalContext => "modal",
        HandlerParams::Both => "both",
    }
}

/// Decode handler params from metadata string
#[allow(dead_code)] // Reserved for future use
pub fn decode_params(s: &str) -> HandlerParams {
    match s {
        "none" => HandlerParams::None,
        "app" => HandlerParams::AppContext,
        "modal" => HandlerParams::ModalContext,
        "both" => HandlerParams::Both,
        _ => HandlerParams::AppContext, // fallback for legacy
    }
}

pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let func: ItemFn = match parse2(item) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error(),
    };

    let params = detect_handler_params(&func);

    // Encode metadata as doc attribute that #[app_impl] / #[modal_impl] can parse
    // Format: __rafter_handler:params
    let metadata = format!("__rafter_handler:{}", encode_params(params));

    let vis = &func.vis;
    let sig = &func.sig;
    let block = &func.block;
    let other_attrs: Vec<_> = func
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("handler"))
        .collect();

    quote! {
        #[doc = #metadata]
        #(#other_attrs)*
        #vis #sig #block
    }
}
