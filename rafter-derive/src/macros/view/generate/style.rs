//! Style and color code generation.

use proc_macro2::TokenStream;
use quote::quote;

use crate::macros::view::ast::{Attr, AttrValue};

/// Generate style struct from attributes
pub fn generate_style(attrs: &[Attr]) -> TokenStream {
    let mut bold = quote! { false };
    let mut italic = quote! { false };
    let mut underline = quote! { false };
    let mut dim = quote! { false };
    let mut fg = quote! { None };
    let mut bg = quote! { None };

    for attr in attrs {
        let name_str = attr.name.to_string();
        match name_str.as_str() {
            "bold" => {
                if let Some(AttrValue::Bool(v)) = &attr.value {
                    bold = quote! { #v };
                } else {
                    bold = quote! { true };
                }
            }
            "italic" => {
                if let Some(AttrValue::Bool(v)) = &attr.value {
                    italic = quote! { #v };
                } else {
                    italic = quote! { true };
                }
            }
            "underline" => {
                if let Some(AttrValue::Bool(v)) = &attr.value {
                    underline = quote! { #v };
                } else {
                    underline = quote! { true };
                }
            }
            "dim" => {
                if let Some(AttrValue::Bool(v)) = &attr.value {
                    dim = quote! { #v };
                } else {
                    dim = quote! { true };
                }
            }
            "color" | "fg" => {
                fg = generate_color_value(&attr.value);
            }
            "bg" | "background" => {
                bg = generate_color_value(&attr.value);
            }
            _ => {}
        }
    }

    quote! {
        rafter::style::Style {
            fg: #fg,
            bg: #bg,
            bold: #bold,
            italic: #italic,
            underline: #underline,
            dim: #dim,
        }
    }
}

/// Generate color value from attribute
pub fn generate_color_value(value: &Option<AttrValue>) -> TokenStream {
    match value {
        Some(AttrValue::Ident(ident)) => {
            // Color name like "primary", "error", etc.
            // Use StyleColor::Named for theme color lookup
            let name_str = ident.to_string();
            quote! { Some(rafter::color::StyleColor::Named(#name_str.to_string())) }
        }
        Some(AttrValue::Str(s)) => {
            // Hex color or color name - parse and wrap as concrete StyleColor
            quote! { Some(rafter::color::StyleColor::Concrete(rafter::color::Color::parse(#s).unwrap_or_default())) }
        }
        Some(AttrValue::Expr(e)) => {
            // Expression should produce a StyleColor or Color
            quote! { Some((#e).into()) }
        }
        _ => quote! { None },
    }
}

/// Generate both style and layout from attributes
pub fn generate_style_and_layout(attrs: &[Attr]) -> (TokenStream, TokenStream) {
    let style = generate_style(attrs);
    let layout = super::layout::generate_layout(attrs);
    (style, layout)
}
