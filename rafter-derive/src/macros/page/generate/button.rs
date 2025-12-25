//! Button element code generation.

use proc_macro2::TokenStream;
use quote::quote;

use crate::macros::page::ast::{AttrValue, ElementNode};

use super::layout::generate_layout;
use super::style::generate_style;

/// Generate code for a button element
pub fn generate_button_element(elem: &ElementNode) -> TokenStream {
    let style = generate_style(&elem.attrs);
    let layout = generate_layout(&elem.attrs);
    let mut label = quote! { String::new() };
    let mut on_click = quote! { None };
    let mut custom_id: Option<TokenStream> = None;

    for attr in &elem.attrs {
        let name_str = attr.name.to_string();
        match name_str.as_str() {
            "label" => {
                if let Some(AttrValue::Str(s)) = &attr.value {
                    label = quote! { #s.to_string() };
                } else if let Some(AttrValue::Expr(e)) = &attr.value {
                    label = quote! { #e.to_string() };
                } else if let Some(AttrValue::Ident(i)) = &attr.value {
                    // Bare identifier - treat as a variable reference
                    label = quote! { #i.to_string() };
                }
            }
            "id" => {
                if let Some(AttrValue::Str(s)) = &attr.value {
                    custom_id = Some(quote! { #s });
                } else if let Some(AttrValue::Ident(i)) = &attr.value {
                    // Variable reference - use the variable value as the ID
                    custom_id = Some(quote! { #i });
                } else if let Some(AttrValue::Expr(e)) = &attr.value {
                    // Expression - evaluate it
                    custom_id = Some(quote! { #e });
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

    // Use custom ID if provided, otherwise generate a compile-time stable ID
    let widget_creation = if let Some(id) = custom_id {
        quote! { rafter::widgets::Button::with_id(#id, #label) }
    } else {
        // For inline buttons without explicit ID, we still need a stable ID.
        // We use the label as part of the ID to make it somewhat unique.
        // This is a fallback - users should provide explicit IDs for proper stability.
        quote! { rafter::widgets::Button::new(#label) }
    };

    quote! {
        {
            let __widget = #widget_creation;
            rafter::node::Node::Widget {
                widget: Box::new(__widget) as Box<dyn rafter::widgets::AnyWidget>,
                handlers: rafter::widgets::WidgetHandlers {
                    on_click: #on_click,
                    ..Default::default()
                },
                style: #style,
                layout: #layout,
                children: Vec::new(),
            }
        }
    }
}
