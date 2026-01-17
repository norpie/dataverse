//! Style generation for the page! macro.
//!
//! Generates a single merged `.style()` call from style attributes.

use proc_macro2::TokenStream;
use quote::quote;

use crate::macros::page::ast::{Attr, AttrValue};

use super::{generate_conditional_attr_value, is_conditional};

/// Generate a single merged style call from style attributes.
/// Returns None if no style attributes are present.
pub fn generate_merged_style(attrs: &[&Attr]) -> Option<TokenStream> {
    if attrs.is_empty() {
        return None;
    }

    let style_methods: Vec<_> = attrs
        .iter()
        .filter_map(|attr| generate_style_method(attr))
        .collect();

    if style_methods.is_empty() {
        return None;
    }

    Some(quote! {
        .style(tuidom::Style::new()#(#style_methods)*)
    })
}

/// Generate a style method call for a single attribute
fn generate_style_method(attr: &Attr) -> Option<TokenStream> {
    let name_str = attr.name.to_string();

    match name_str.as_str() {
        "bg" | "background" => {
            let color = generate_color(&attr.value);
            Some(quote! { .background(#color) })
        }
        "fg" | "foreground" | "color" => {
            let color = generate_color(&attr.value);
            Some(quote! { .foreground(#color) })
        }
        "bold" => {
            if is_true_value(&attr.value) {
                Some(quote! { .bold() })
            } else {
                None
            }
        }
        "italic" => {
            if is_true_value(&attr.value) {
                Some(quote! { .italic() })
            } else {
                None
            }
        }
        "underline" => {
            if is_true_value(&attr.value) {
                Some(quote! { .underline() })
            } else {
                None
            }
        }
        "dim" => {
            if is_true_value(&attr.value) {
                Some(quote! { .dim() })
            } else {
                None
            }
        }
        "border" => {
            let border = generate_border(&attr.value);
            Some(quote! { .border(#border) })
        }
        _ => None,
    }
}

/// Check if an attribute value is truthy (true, or just present)
fn is_true_value(value: &AttrValue) -> bool {
    match value {
        AttrValue::Ident(ident) => ident == "true",
        AttrValue::Lit(syn::Lit::Bool(b)) => b.value,
        _ => true, // Expression or other - assume true
    }
}

/// Generate color value from attribute
pub fn generate_color(value: &AttrValue) -> TokenStream {
    // Handle conditional values
    if is_conditional(value) {
        return generate_conditional_attr_value(value, generate_color_leaf);
    }

    generate_color_leaf(value)
}

/// Generate color value for a leaf (non-conditional) attribute value
fn generate_color_leaf(value: &AttrValue) -> TokenStream {
    match value {
        AttrValue::Ident(ident) => {
            // All identifiers are treated as theme variable names
            let ident_str = ident.to_string();
            quote! { tuidom::Color::var(#ident_str) }
        }
        AttrValue::Lit(syn::Lit::Str(s)) => {
            let val = s.value();
            if val.starts_with('#') {
                // Hex color - parse at macro expansion time
                if let Some((r, g, b)) = parse_hex_color(&val) {
                    quote! { tuidom::Color::rgb(#r, #g, #b) }
                } else {
                    // Invalid hex, fall back to theme variable
                    quote! { tuidom::Color::var(#val) }
                }
            } else {
                // Theme variable
                quote! { tuidom::Color::var(#val) }
            }
        }
        AttrValue::Expr(expr) => {
            quote! { #expr }
        }
        _ => quote! { tuidom::Color::default() },
    }
}

/// Generate border value from attribute
fn generate_border(value: &AttrValue) -> TokenStream {
    // Handle conditional values
    if is_conditional(value) {
        return generate_conditional_attr_value(value, generate_border_leaf);
    }

    generate_border_leaf(value)
}

/// Generate border value for a leaf (non-conditional) attribute value
fn generate_border_leaf(value: &AttrValue) -> TokenStream {
    match value {
        AttrValue::Ident(ident) => {
            let ident_str = ident.to_string();
            match ident_str.as_str() {
                "none" => quote! { tuidom::Border::None },
                "single" => quote! { tuidom::Border::Single },
                "double" => quote! { tuidom::Border::Double },
                "rounded" => quote! { tuidom::Border::Rounded },
                "thick" => quote! { tuidom::Border::Thick },
                _ => quote! { tuidom::Border::None },
            }
        }
        AttrValue::Expr(expr) => quote! { #expr },
        _ => quote! { tuidom::Border::None },
    }
}

/// Parse a hex color string like "#ff0000" or "#f00" into RGB values
fn parse_hex_color(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.trim_start_matches('#');

    match hex.len() {
        // Short form: #rgb
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            Some((r, g, b))
        }
        // Long form: #rrggbb
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some((r, g, b))
        }
        _ => None,
    }
}
