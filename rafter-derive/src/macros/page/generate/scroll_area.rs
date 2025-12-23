//! ScrollArea element code generation.

use proc_macro2::TokenStream;
use quote::quote;

use crate::macros::page::ast::{AttrValue, ElementNode};

use super::layout::generate_layout;
use super::style::generate_style;

/// Generate code for a scroll_area element
pub fn generate_scroll_area_element(elem: &ElementNode) -> TokenStream {
    let style = generate_style(&elem.attrs);
    let layout = generate_layout(&elem.attrs);

    // Find the bind: attribute - required for scroll_area elements
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

    let scroll_area_component = match bind_expr {
        Some(expr) => expr,
        None => {
            return syn::Error::new_spanned(
                &elem.name,
                "scroll_area elements require a `bind:` attribute, e.g. `scroll_area(bind: self.my_scroll)`",
            )
            .to_compile_error();
        }
    };

    // Parse optional direction attribute
    let direction = elem.attrs.iter().find_map(|attr| {
        if attr.name == "direction" {
            match &attr.value {
                Some(AttrValue::Ident(i)) => {
                    let dir_str = i.to_string();
                    match dir_str.as_str() {
                        "vertical" => Some(quote! { rafter::widgets::ScrollDirection::Vertical }),
                        "horizontal" => {
                            Some(quote! { rafter::widgets::ScrollDirection::Horizontal })
                        }
                        "both" => Some(quote! { rafter::widgets::ScrollDirection::Both }),
                        _ => None,
                    }
                }
                _ => None,
            }
        } else {
            None
        }
    });

    // If direction specified, set it on the widget
    let direction_setter = direction.map(|dir| {
        quote! {
            __widget.set_direction(#dir);
        }
    });

    // Generate children
    let children: Vec<_> = elem.children.iter().map(super::generate_node).collect();

    quote! {
        {
            let __widget = (#scroll_area_component).clone();
            #direction_setter
            rafter::node::Node::Widget {
                widget: Box::new(__widget) as Box<dyn rafter::widgets::AnyWidget>,
                handlers: rafter::widgets::WidgetHandlers::default(),
                style: #style,
                layout: #layout,
                children: vec![#(#children),*],
            }
        }
    }
}
