//! Collapsible element code generation.

use proc_macro2::TokenStream;
use quote::quote;

use crate::macros::page::ast::{AttrValue, ElementNode};

use super::layout::generate_layout;
use super::style::generate_style;

/// Generate code for a collapsible element
pub fn generate_collapsible_element(elem: &ElementNode) -> TokenStream {
    let style = generate_style(&elem.attrs);
    let layout = generate_layout(&elem.attrs);

    // Find the bind: attribute - required for collapsible elements
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

    let collapsible_component = match bind_expr {
        Some(expr) => expr,
        None => {
            return syn::Error::new_spanned(
                &elem.name,
                "collapsible elements require a `bind:` attribute, e.g. `collapsible(bind: self.my_section)`",
            )
            .to_compile_error();
        }
    };

    // Parse optional title attribute
    let title_setter = elem.attrs.iter().find_map(|attr| {
        if attr.name == "title" {
            match &attr.value {
                Some(AttrValue::Str(s)) => Some(quote! {
                    __widget.set_title(#s);
                }),
                Some(AttrValue::Expr(e)) => Some(quote! {
                    __widget.set_title(#e);
                }),
                Some(AttrValue::Ident(i)) => Some(quote! {
                    __widget.set_title(#i);
                }),
                _ => None,
            }
        } else {
            None
        }
    });

    // Parse optional event handlers
    let mut on_expand = quote! { None };
    let mut on_collapse = quote! { None };
    let mut on_change = quote! { None };

    for attr in &elem.attrs {
        let name_str = attr.name.to_string();
        match name_str.as_str() {
            "on_expand" => {
                if let Some(AttrValue::Ident(i)) = &attr.value {
                    let handler_name = i.to_string();
                    on_expand =
                        quote! { Some(rafter::keybinds::HandlerId(#handler_name.to_string())) };
                }
            }
            "on_collapse" => {
                if let Some(AttrValue::Ident(i)) = &attr.value {
                    let handler_name = i.to_string();
                    on_collapse =
                        quote! { Some(rafter::keybinds::HandlerId(#handler_name.to_string())) };
                }
            }
            "on_change" => {
                if let Some(AttrValue::Ident(i)) = &attr.value {
                    let handler_name = i.to_string();
                    on_change =
                        quote! { Some(rafter::keybinds::HandlerId(#handler_name.to_string())) };
                }
            }
            _ => {}
        }
    }

    // Generate children
    let children: Vec<_> = elem.children.iter().map(super::generate_node).collect();

    // Children are only included in the Node tree when expanded.
    // This ensures collapsed children are not focusable and don't participate in layout.
    quote! {
        {
            let __widget = (#collapsible_component).clone();
            #title_setter
            let __children = if __widget.is_expanded() {
                vec![#(#children),*]
            } else {
                Vec::new()
            };
            rafter::node::Node::Widget {
                widget: Box::new(__widget) as Box<dyn rafter::widgets::AnyWidget>,
                handlers: rafter::widgets::WidgetHandlers {
                    on_expand: #on_expand,
                    on_collapse: #on_collapse,
                    on_change: #on_change,
                    ..Default::default()
                },
                style: #style,
                layout: #layout,
                children: __children,
            }
        }
    }
}
