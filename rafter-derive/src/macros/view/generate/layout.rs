//! Layout code generation.

use proc_macro2::TokenStream;
use quote::quote;

use crate::macros::view::ast::{Attr, AttrValue};

/// Generate layout struct from attributes
pub fn generate_layout(attrs: &[Attr]) -> TokenStream {
    let mut width = quote! { rafter::node::Size::Auto };
    let mut height = quote! { rafter::node::Size::Auto };
    let mut flex = quote! { None };
    let mut min_width = quote! { None };
    let mut max_width = quote! { None };
    let mut min_height = quote! { None };
    let mut max_height = quote! { None };
    let mut padding = quote! { 0 };
    let mut gap = quote! { 0 };
    let mut justify = quote! { rafter::node::Justify::Start };
    let mut align = quote! { rafter::node::Align::Stretch };
    let mut border = quote! { rafter::node::Border::None };

    for attr in attrs {
        let name_str = attr.name.to_string();
        match name_str.as_str() {
            "width" => {
                width = parse_size_value(&attr.value);
            }
            "height" => {
                height = parse_size_value(&attr.value);
            }
            "flex" => {
                if let Some(AttrValue::Int(v)) = &attr.value {
                    let v = *v as u16;
                    flex = quote! { Some(#v) };
                }
            }
            "min_width" => {
                if let Some(AttrValue::Int(v)) = &attr.value {
                    let v = *v as u16;
                    min_width = quote! { Some(#v) };
                }
            }
            "max_width" => {
                if let Some(AttrValue::Int(v)) = &attr.value {
                    let v = *v as u16;
                    max_width = quote! { Some(#v) };
                }
            }
            "min_height" => {
                if let Some(AttrValue::Int(v)) = &attr.value {
                    let v = *v as u16;
                    min_height = quote! { Some(#v) };
                }
            }
            "max_height" => {
                if let Some(AttrValue::Int(v)) = &attr.value {
                    let v = *v as u16;
                    max_height = quote! { Some(#v) };
                }
            }
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
            width: #width,
            height: #height,
            flex: #flex,
            min_width: #min_width,
            max_width: #max_width,
            min_height: #min_height,
            max_height: #max_height,
            padding: #padding,
            gap: #gap,
            justify: #justify,
            align: #align,
            border: #border,
            ..Default::default()
        }
    }
}

/// Parse a size value (fixed, percentage, or fill)
fn parse_size_value(value: &Option<AttrValue>) -> TokenStream {
    match value {
        Some(AttrValue::Int(v)) => {
            let v = *v as u16;
            quote! { rafter::node::Size::Fixed(#v) }
        }
        Some(AttrValue::Ident(ident)) => {
            let name = ident.to_string();
            if name == "fill" {
                // fill = flex: 1 behavior (use Flex(1) for now)
                quote! { rafter::node::Size::Flex(1) }
            } else {
                // "auto" or any unrecognized ident defaults to Auto
                quote! { rafter::node::Size::Auto }
            }
        }
        Some(AttrValue::Str(s)) => {
            // Handle percentage strings like "50%"
            if let Some(stripped) = s.strip_suffix('%')
                && let Ok(pct) = stripped.parse::<f32>()
            {
                let pct = pct / 100.0;
                return quote! { rafter::node::Size::Percent(#pct) };
            }
            quote! { rafter::node::Size::Auto }
        }
        _ => quote! { rafter::node::Size::Auto },
    }
}
