use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, ItemFn, parse2};

/// Extract handler name
fn handler_name(func: &ItemFn) -> &Ident {
    &func.sig.ident
}

/// Check if the function takes a context parameter (AppContext)
fn takes_context(func: &ItemFn) -> bool {
    // Check if any parameter's type contains "AppContext" or "Context"
    func.sig.inputs.iter().any(|arg| {
        if let syn::FnArg::Typed(pat_type) = arg {
            let ty_str = quote::quote!(#pat_type.ty).to_string();
            ty_str.contains("AppContext") || ty_str.contains("Context")
        } else {
            false
        }
    })
}

pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let func: ItemFn = match parse2(item) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error(),
    };

    let _name = handler_name(&func);
    let has_context = takes_context(&func);

    // We preserve the function as-is but add a doc attribute containing metadata
    // that #[app_impl] can parse. This is a bit hacky but works with proc macros.
    // The metadata is encoded as: __rafter_handler:has_context
    let metadata = format!("__rafter_handler:{}", has_context);

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
