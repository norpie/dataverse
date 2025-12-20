//! Scrollable element code generation.

use proc_macro2::TokenStream;
use quote::quote;

use crate::macros::view::ast::{AttrValue, ElementNode};

use super::layout::generate_layout;
use super::style::generate_style;

/// Generate code for a scrollable element
pub fn generate_scrollable_element(elem: &ElementNode) -> TokenStream {
    let style = generate_style(&elem.attrs);
    let layout = generate_layout(&elem.attrs);

    // Find the bind: attribute - required for scrollable elements
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

    let scrollable_component = match bind_expr {
        Some(expr) => expr,
        None => {
            return syn::Error::new_spanned(
                &elem.name,
                "scrollable elements require a `bind:` attribute, e.g. `scrollable(bind: self.my_scroll)`",
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
                        "vertical" => Some(quote! { rafter::components::ScrollDirection::Vertical }),
                        "horizontal" => Some(quote! { rafter::components::ScrollDirection::Horizontal }),
                        "both" => Some(quote! { rafter::components::ScrollDirection::Both }),
                        _ => None,
                    }
                }
                _ => None,
            }
        } else {
            None
        }
    });

    // If direction specified, set it on the component
    let direction_setter = direction.map(|dir| {
        quote! {
            __component.set_direction(#dir);
        }
    });

    // Generate children - wrap in a column if multiple children
    let children: Vec<_> = elem.children.iter().map(super::generate_node).collect();
    let child_node = if children.len() == 1 {
        children.into_iter().next().unwrap()
    } else {
        quote! {
            rafter::node::Node::Column {
                children: vec![#(#children),*],
                style: rafter::style::Style::new(),
                layout: rafter::node::Layout::default(),
            }
        }
    };

    quote! {
        {
            let __component = (#scrollable_component).clone();
            #direction_setter
            rafter::node::Node::Scrollable {
                child: Box::new(#child_node),
                id: __component.id_string(),
                style: #style,
                layout: #layout,
                component: __component,
            }
        }
    }
}
