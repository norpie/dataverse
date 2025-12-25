//! Autocomplete element code generation.

use proc_macro2::TokenStream;
use quote::quote;

use crate::macros::page::ast::{AttrValue, ElementNode};

use super::layout::generate_layout;
use super::style::generate_style;

/// Generate code for an autocomplete element
///
/// Usage in view!:
/// ```ignore
/// autocomplete(
///     bind: self.search,
///     options: items,
///     placeholder: "Search...",
///     on_change: Self::on_search_change,
///     on_select: Self::on_item_selected,
/// )
/// ```
pub fn generate_autocomplete_element(elem: &ElementNode) -> TokenStream {
    let style = generate_style(&elem.attrs);
    let layout = generate_layout(&elem.attrs);

    // Find the bind: attribute - required for autocomplete elements
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

    let autocomplete_component = match bind_expr {
        Some(expr) => expr,
        None => {
            return syn::Error::new_spanned(
                &elem.name,
                "autocomplete elements require a `bind:` attribute, e.g. `autocomplete(bind: self.search, options: items)`",
            )
            .to_compile_error();
        }
    };

    // Find the options: attribute - required for autocomplete elements
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
                "autocomplete elements require an `options:` attribute, e.g. `autocomplete(bind: self.search, options: items)`",
            )
            .to_compile_error();
        }
    };

    // Parse optional attributes
    let mut on_change = quote! { None };
    let mut on_select = quote! { None };

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
            "on_select" => {
                if let Some(AttrValue::Ident(i)) = &attr.value {
                    let handler_name = i.to_string();
                    on_select =
                        quote! { Some(rafter::keybinds::HandlerId(#handler_name.to_string())) };
                }
            }
            _ => {}
        }
    }

    // Generate code that:
    // 1. Clones the autocomplete widget
    // 2. Collects options and builds children nodes (text nodes for each option label)
    // 3. Creates the Node::Widget with children
    quote! {
        {
            let __widget = (#autocomplete_component).clone();

            // Build option children for the overlay
            let __options: Vec<_> = (#options).into_iter().collect();
            let __children: Vec<rafter::node::Node> = __options.iter().map(|opt| {
                use rafter::widgets::AutocompleteItem;
                rafter::node::Node::Text {
                    content: opt.autocomplete_label(),
                    style: rafter::style::Style::default(),
                }
            }).collect();

            // Update the widget with option labels
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
                    on_select: #on_select,
                    ..Default::default()
                },
                style: #style,
                layout: #layout,
                children: __children,
            }
        }
    }
}
