//! List element code generation.

use proc_macro2::TokenStream;
use quote::quote;

use crate::macros::view::ast::{AttrValue, ElementNode};

use super::layout::generate_layout;
use super::style::generate_style;

/// Generate code for a list element
pub fn generate_list_element(elem: &ElementNode) -> TokenStream {
    let style = generate_style(&elem.attrs);
    let layout = generate_layout(&elem.attrs);

    // Find the bind: attribute - required for list elements
    let bind_expr = elem.attrs.iter().find_map(|attr| {
        if attr.name == "bind" {
            match &attr.value {
                Some(AttrValue::Expr(e)) => Some(e.clone()),
                Some(AttrValue::Ident(i)) => Some(syn::parse_quote! { #i }),
                _ => None,
            }
        } else {
            None
        }
    });

    let list_component = match bind_expr {
        Some(expr) => expr,
        None => {
            return syn::Error::new_spanned(
                &elem.name,
                "list elements require a `bind:` attribute, e.g. `list(bind: self.files)`",
            )
            .to_compile_error();
        }
    };

    // Parse optional event handlers
    let mut on_activate = quote! { None };
    let mut on_selection_change = quote! { None };
    let mut on_cursor_move = quote! { None };

    for attr in &elem.attrs {
        let name_str = attr.name.to_string();
        match name_str.as_str() {
            "on_activate" => {
                if let Some(AttrValue::Ident(i)) = &attr.value {
                    let handler_name = i.to_string();
                    on_activate =
                        quote! { Some(rafter::keybinds::HandlerId(#handler_name.to_string())) };
                }
            }
            "on_selection_change" => {
                if let Some(AttrValue::Ident(i)) = &attr.value {
                    let handler_name = i.to_string();
                    on_selection_change =
                        quote! { Some(rafter::keybinds::HandlerId(#handler_name.to_string())) };
                }
            }
            "on_cursor_move" => {
                if let Some(AttrValue::Ident(i)) = &attr.value {
                    let handler_name = i.to_string();
                    on_cursor_move =
                        quote! { Some(rafter::keybinds::HandlerId(#handler_name.to_string())) };
                }
            }
            _ => {}
        }
    }

    quote! {
        {
            let __component = (#list_component).clone();
            rafter::node::Node::List {
                id: __component.id_string(),
                style: #style,
                layout: #layout,
                component: Box::new(__component),
                on_activate: #on_activate,
                on_selection_change: #on_selection_change,
                on_cursor_move: #on_cursor_move,
            }
        }
    }
}
