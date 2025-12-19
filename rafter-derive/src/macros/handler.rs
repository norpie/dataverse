use proc_macro2::TokenStream;
use quote::quote;
use syn::{Expr, ExprLit, Ident, ItemFn, Lit, Meta, parse2};

/// Handler attributes
#[derive(Default)]
struct HandlerAttrs {
    /// Handler supersedes previous invocations (cancels them)
    supersedes: bool,
    /// Handler calls are queued and run sequentially
    queues: bool,
    /// Debounce duration in milliseconds
    debounce_ms: Option<u64>,
}

impl HandlerAttrs {
    fn parse(attr: TokenStream) -> syn::Result<Self> {
        let mut result = Self::default();

        if attr.is_empty() {
            return Ok(result);
        }

        // Try parsing as a list of attributes
        let meta: Meta = parse2(attr)?;

        match meta {
            Meta::Path(path) => {
                // Single word: #[handler(supersedes)]
                if path.is_ident("supersedes") {
                    result.supersedes = true;
                } else if path.is_ident("queues") {
                    result.queues = true;
                }
            }
            Meta::NameValue(nv) => {
                // Name = value: #[handler(debounce = 300ms)]
                if nv.path.is_ident("debounce")
                    && let Expr::Lit(ExprLit {
                        lit: Lit::Int(lit), ..
                    }) = &nv.value
                {
                    // Parse as milliseconds
                    result.debounce_ms = Some(lit.base10_parse()?);
                }
            }
            Meta::List(list) => {
                // Multiple items: #[handler(supersedes, debounce = 300)]
                list.parse_nested_meta(|meta| {
                    if meta.path.is_ident("supersedes") {
                        result.supersedes = true;
                    } else if meta.path.is_ident("queues") {
                        result.queues = true;
                    } else if meta.path.is_ident("debounce") {
                        let value: Expr = meta.value()?.parse()?;
                        if let Expr::Lit(ExprLit {
                            lit: Lit::Int(lit), ..
                        }) = value
                        {
                            result.debounce_ms = Some(lit.base10_parse()?);
                        }
                    }
                    Ok(())
                })?;
            }
        }

        Ok(result)
    }
}

/// Check if a function is async
fn is_async_fn(func: &ItemFn) -> bool {
    func.sig.asyncness.is_some()
}

/// Extract handler name
fn handler_name(func: &ItemFn) -> &Ident {
    &func.sig.ident
}

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attrs = match HandlerAttrs::parse(attr) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error(),
    };

    let func: ItemFn = match parse2(item) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error(),
    };

    let _name = handler_name(&func);
    let is_async = is_async_fn(&func);

    // Generate handler metadata as an attribute that #[app_impl] can read
    let supersedes = attrs.supersedes;
    let queues = attrs.queues;
    let debounce_ms = attrs.debounce_ms.unwrap_or(0);

    // We preserve the function as-is but add a doc attribute containing metadata
    // that #[app_impl] can parse. This is a bit hacky but works with proc macros.
    // The metadata is encoded as: __rafter_handler:async:supersedes:queues:debounce_ms
    let metadata = format!(
        "__rafter_handler:{}:{}:{}:{}",
        is_async, supersedes, queues, debounce_ms
    );

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
