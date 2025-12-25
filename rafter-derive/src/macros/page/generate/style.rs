//! Style and color code generation.

use proc_macro2::TokenStream;
use quote::quote;

use crate::macros::page::ast::{Attr, AttrValue};

/// Generate style struct from attributes
pub fn generate_style(attrs: &[Attr]) -> TokenStream {
    let mut bold = quote! { false };
    let mut italic = quote! { false };
    let mut underline = quote! { false };
    let mut dim = quote! { false };
    let mut fg = quote! { None };
    let mut bg = quote! { None };
    let mut opacity = quote! { None };
    let mut transition_duration = quote! { None };
    let mut transition_easing = quote! { rafter::runtime::Easing::Linear };

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
            "opacity" => {
                // opacity: 0.5 or opacity: {expr}
                if let Some(AttrValue::Float(f)) = &attr.value {
                    opacity = quote! { Some(#f as f32) };
                } else if let Some(AttrValue::Int(i)) = &attr.value {
                    let f = *i as f32;
                    opacity = quote! { Some(#f) };
                } else if let Some(AttrValue::Expr(e)) = &attr.value {
                    opacity = quote! { Some(#e) };
                }
            }
            "transition" => {
                // transition: 200 (milliseconds)
                if let Some(AttrValue::Int(ms)) = &attr.value {
                    let ms_u64 = *ms as u64;
                    transition_duration =
                        quote! { Some(std::time::Duration::from_millis(#ms_u64)) };
                } else if let Some(AttrValue::Expr(e)) = &attr.value {
                    transition_duration = quote! { Some(#e) };
                }
            }
            "easing" => {
                // easing: ease_in, ease_out, ease_in_out, linear
                if let Some(AttrValue::Ident(ident)) = &attr.value {
                    let easing_name = ident.to_string();
                    transition_easing = match easing_name.as_str() {
                        "linear" => quote! { rafter::runtime::Easing::Linear },
                        "ease_in" => quote! { rafter::runtime::Easing::EaseIn },
                        "ease_out" => quote! { rafter::runtime::Easing::EaseOut },
                        "ease_in_out" => quote! { rafter::runtime::Easing::EaseInOut },
                        _ => quote! { rafter::runtime::Easing::Linear },
                    };
                }
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
            opacity: #opacity,
            transition_duration: #transition_duration,
            transition_easing: #transition_easing,
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
