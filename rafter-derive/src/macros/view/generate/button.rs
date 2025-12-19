//! Button element code generation.

use proc_macro2::TokenStream;
use quote::quote;

use crate::macros::view::ast::{AttrValue, ElementNode};

use super::style::generate_style;

/// Generate code for a button element
pub fn generate_button_element(elem: &ElementNode) -> TokenStream {
    let style = generate_style(&elem.attrs);
    let mut label = quote! { String::new() };
    let mut id = quote! { String::new() };
    let mut on_click = quote! { None };

    for attr in &elem.attrs {
        let name_str = attr.name.to_string();
        match name_str.as_str() {
            "label" => {
                if let Some(AttrValue::Str(s)) = &attr.value {
                    label = quote! { #s.to_string() };
                } else if let Some(AttrValue::Expr(e)) = &attr.value {
                    label = quote! { #e.to_string() };
                }
            }
            "id" => {
                if let Some(AttrValue::Str(s)) = &attr.value {
                    id = quote! { #s.to_string() };
                } else if let Some(AttrValue::Expr(e)) = &attr.value {
                    id = quote! { #e.to_string() };
                }
            }
            "on_click" => {
                if let Some(AttrValue::Ident(i)) = &attr.value {
                    let handler_name = i.to_string();
                    on_click =
                        quote! { Some(rafter::keybinds::HandlerId(#handler_name.to_string())) };
                }
            }
            _ => {}
        }
    }

    quote! {
        rafter::node::Node::Button {
            label: #label,
            on_click: #on_click,
            id: #id,
            style: #style,
        }
    }
}
