//! Select element code generation.

use proc_macro2::TokenStream;
use quote::quote;

use crate::macros::page::ast::{AttrValue, ElementNode};

use super::layout::generate_layout;
use super::style::generate_style;

/// Generate code for a select element
///
/// Usage in view!:
/// ```ignore
/// select(bind: self.priority, options: priorities, placeholder: "Select priority")
/// ```
pub fn generate_select_element(elem: &ElementNode) -> TokenStream {
    let style = generate_style(&elem.attrs);
    let layout = generate_layout(&elem.attrs);

    // Find the bind: attribute - required for select elements
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

    let select_component = match bind_expr {
        Some(expr) => expr,
        None => {
            return syn::Error::new_spanned(
                &elem.name,
                "select elements require a `bind:` attribute, e.g. `select(bind: self.my_select, options: items)`",
            )
            .to_compile_error();
        }
    };

    // Find the options: attribute - required for select elements
    let options_expr = elem.attrs.iter().find_map(|attr| {
        if attr.name == "options" {
            match &attr.value {
                Some(AttrValue::Expr(e)) => Some(e.clone()),
                Some(AttrValue::Ident(i)) => Some(syn::parse_quote! { #i }),
                _ => None,
            }
        } else {
            None
        }
    });

    let options = match options_expr {
        Some(expr) => expr,
        None => {
            return syn::Error::new_spanned(
                &elem.name,
                "select elements require an `options:` attribute, e.g. `select(bind: self.my_select, options: items)`",
            )
            .to_compile_error();
        }
    };

    // Parse optional attributes
    let mut on_change = quote! { None };

    for attr in &elem.attrs {
        let name_str = attr.name.to_string();
        match name_str.as_str() {
            "on_change" => {
                if let Some(AttrValue::Ident(i)) = &attr.value {
                    let handler_name = i.to_string();
                    on_change =
                        quote! { Some(rafter::keybinds::HandlerId(#handler_name.to_string())) };
                }
            }
            _ => {}
        }
    }

    // Generate code that:
    // 1. Clones the select widget
    // 2. Collects options and builds children nodes (text nodes for each option label)
    // 3. Creates the Node::Widget with children
    quote! {
        {
            let __widget = (#select_component).clone();

            // Build option children for the overlay
            let __options: Vec<_> = (#options).into_iter().collect();
            let __children: Vec<rafter::node::Node> = __options.iter().map(|opt| {
                use rafter::widgets::SelectItem;
                rafter::node::Node::Text {
                    content: opt.select_label(),
                    style: rafter::style::Style::default(),
                }
            }).collect();

            // Update the widget with options count
            __widget.set_options_count(__children.len());
            __widget.set_option_labels(__children.iter().filter_map(|n| {
                if let rafter::node::Node::Text { content, .. } = n {
                    Some(content.clone())
                } else {
                    None
                }
            }).collect());

            rafter::node::Node::Widget {
                widget: Box::new(__widget) as Box<dyn rafter::widgets::AnyWidget>,
                handlers: rafter::widgets::WidgetHandlers {
                    on_change: #on_change,
                    ..Default::default()
                },
                style: #style,
                layout: #layout,
                children: __children,
            }
        }
    }
}
