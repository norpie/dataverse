//! Input element code generation.

use proc_macro2::TokenStream;
use quote::quote;

use crate::macros::view::ast::{AttrValue, ElementNode};

use super::style::generate_style;

/// Generate code for an input element
pub fn generate_input_element(elem: &ElementNode) -> TokenStream {
    let style = generate_style(&elem.attrs);
    let mut value = quote! { String::new() };
    let mut placeholder = quote! { String::new() };
    let mut id = quote! { String::new() };
    let mut on_change = quote! { None };
    let mut on_submit = quote! { None };

    for attr in &elem.attrs {
        let name_str = attr.name.to_string();
        match name_str.as_str() {
            "value" => {
                if let Some(AttrValue::Str(s)) = &attr.value {
                    value = quote! { #s.to_string() };
                } else if let Some(AttrValue::Expr(e)) = &attr.value {
                    value = quote! { (#e).to_string() };
                } else if let Some(AttrValue::Ident(i)) = &attr.value {
                    value = quote! { #i.to_string() };
                }
            }
            "placeholder" => {
                if let Some(AttrValue::Str(s)) = &attr.value {
                    placeholder = quote! { #s.to_string() };
                } else if let Some(AttrValue::Expr(e)) = &attr.value {
                    placeholder = quote! { #e.to_string() };
                }
            }
            "id" => {
                if let Some(AttrValue::Str(s)) = &attr.value {
                    id = quote! { #s.to_string() };
                } else if let Some(AttrValue::Expr(e)) = &attr.value {
                    id = quote! { #e.to_string() };
                }
            }
            "on_change" => {
                if let Some(AttrValue::Ident(i)) = &attr.value {
                    let handler_name = i.to_string();
                    on_change =
                        quote! { Some(rafter::keybinds::HandlerId(#handler_name.to_string())) };
                }
            }
            "on_submit" => {
                if let Some(AttrValue::Ident(i)) = &attr.value {
                    let handler_name = i.to_string();
                    on_submit =
                        quote! { Some(rafter::keybinds::HandlerId(#handler_name.to_string())) };
                }
            }
            _ => {}
        }
    }

    quote! {
        rafter::node::Node::Input {
            value: #value,
            placeholder: #placeholder,
            on_change: #on_change,
            on_submit: #on_submit,
            id: #id,
            style: #style,
        }
    }
}
