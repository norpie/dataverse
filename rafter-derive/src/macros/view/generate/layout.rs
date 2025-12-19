//! Layout code generation.

use proc_macro2::TokenStream;
use quote::quote;

use crate::macros::view::ast::{Attr, AttrValue};

/// Generate layout struct from attributes
pub fn generate_layout(attrs: &[Attr]) -> TokenStream {
    let mut padding = quote! { 0 };
    let mut gap = quote! { 0 };
    let mut justify = quote! { rafter::node::Justify::Start };
    let mut align = quote! { rafter::node::Align::Stretch };
    let mut border = quote! { rafter::node::Border::None };

    for attr in attrs {
        let name_str = attr.name.to_string();
        match name_str.as_str() {
            "padding" => {
                if let Some(AttrValue::Int(v)) = &attr.value {
                    let v = *v as u16;
                    padding = quote! { #v };
                }
            }
            "gap" => {
                if let Some(AttrValue::Int(v)) = &attr.value {
                    let v = *v as u16;
                    gap = quote! { #v };
                }
            }
            "justify" | "align_items" => {
                if let Some(AttrValue::Ident(ident)) = &attr.value {
                    justify = match ident.to_string().as_str() {
                        "start" => quote! { rafter::node::Justify::Start },
                        "center" => quote! { rafter::node::Justify::Center },
                        "end" => quote! { rafter::node::Justify::End },
                        "space_between" => quote! { rafter::node::Justify::SpaceBetween },
                        "space_around" => quote! { rafter::node::Justify::SpaceAround },
                        _ => quote! { rafter::node::Justify::Start },
                    };
                }
            }
            "align" | "align_content" => {
                if let Some(AttrValue::Ident(ident)) = &attr.value {
                    align = match ident.to_string().as_str() {
                        "start" => quote! { rafter::node::Align::Start },
                        "center" => quote! { rafter::node::Align::Center },
                        "end" => quote! { rafter::node::Align::End },
                        "stretch" => quote! { rafter::node::Align::Stretch },
                        _ => quote! { rafter::node::Align::Stretch },
                    };
                }
            }
            "border" => {
                if let Some(AttrValue::Ident(ident)) = &attr.value {
                    border = match ident.to_string().as_str() {
                        "none" => quote! { rafter::node::Border::None },
                        "single" => quote! { rafter::node::Border::Single },
                        "double" => quote! { rafter::node::Border::Double },
                        "rounded" => quote! { rafter::node::Border::Rounded },
                        "thick" => quote! { rafter::node::Border::Thick },
                        _ => quote! { rafter::node::Border::None },
                    };
                }
            }
            _ => {}
        }
    }

    quote! {
        rafter::node::Layout {
            padding: #padding,
            gap: #gap,
            justify: #justify,
            align: #align,
            border: #border,
            ..Default::default()
        }
    }
}
