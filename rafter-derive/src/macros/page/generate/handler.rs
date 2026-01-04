//! Handler generation for the page! macro.
//!
//! Generates closures for event handlers with captured arguments.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::macros::page::ast::{HandlerArg, HandlerAttr};

/// Generate handler method calls for a widget.
///
/// Each handler becomes a method call like:
/// ```ignore
/// .on_click({
///     let __self = self.clone();
///     let __arg0 = (item.id).clone();
///     move |cx: &rafter::AppContext, gx: &rafter::GlobalContext| {
///         __self.handler_name(__arg0, cx);
///     }
/// })
/// ```
pub fn generate_handler_calls(handlers: &[HandlerAttr]) -> Vec<TokenStream> {
    handlers.iter().map(generate_single_handler).collect()
}

/// Generate a single handler method call
fn generate_single_handler(handler: &HandlerAttr) -> TokenStream {
    let event_method = &handler.event; // on_click, on_change, etc.
    let handler_name = &handler.handler;

    // Separate context args from capture args
    // Context args (cx, gx) are passed at event time, not captured
    let capture_indices: Vec<usize> = handler
        .args
        .iter()
        .enumerate()
        .filter_map(|(i, arg)| {
            if matches!(arg, HandlerArg::Context(_)) {
                None
            } else {
                Some(i)
            }
        })
        .collect();

    // Generate capture statements for non-context args
    let capture_stmts: Vec<TokenStream> = handler
        .args
        .iter()
        .enumerate()
        .filter_map(|(i, arg)| {
            if let HandlerArg::Expr(expr) = arg {
                let var = format_ident!("__arg{}", i);
                Some(quote! { let #var = (#expr).clone(); })
            } else {
                None
            }
        })
        .collect();

    // Generate call args in order, using captured vars or context names
    let call_args: Vec<TokenStream> = handler
        .args
        .iter()
        .enumerate()
        .map(|(i, arg)| match arg {
            HandlerArg::Expr(_) => {
                let var = format_ident!("__arg{}", i);
                quote! { #var }
            }
            HandlerArg::Context(ident) => quote! { #ident },
        })
        .collect();

    // Generate inner clones for captured args (needed for move closure)
    let inner_clones: Vec<TokenStream> = capture_indices
        .iter()
        .map(|&i| {
            let var = format_ident!("__arg{}", i);
            quote! { let #var = #var.clone(); }
        })
        .collect();

    // If no args, generate simpler closure
    if handler.args.is_empty() {
        return quote! {
            .#event_method({
                let __self = self.clone();
                move |_cx: &rafter::AppContext, _gx: &rafter::GlobalContext| {
                    __self.#handler_name();
                }
            })
        };
    }

    quote! {
        .#event_method({
            let __self = self.clone();
            #(#capture_stmts)*
            move |cx: &rafter::AppContext, gx: &rafter::GlobalContext| {
                #(#inner_clones)*
                __self.#handler_name(#(#call_args),*);
            }
        })
    }
}
