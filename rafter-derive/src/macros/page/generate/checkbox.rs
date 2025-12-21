//! Checkbox element code generation.

use proc_macro2::TokenStream;
use quote::quote;

use crate::macros::page::ast::{AttrValue, ElementNode};

use super::layout::generate_layout;
use super::style::generate_style;

/// Generate code for a checkbox element
pub fn generate_checkbox_element(elem: &ElementNode) -> TokenStream {
    let style = generate_style(&elem.attrs);
    let layout = generate_layout(&elem.attrs);

    // Find the bind: attribute - required for checkbox elements
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

    let checkbox_component = match bind_expr {
        Some(expr) => expr,
        None => {
            return syn::Error::new_spanned(
                &elem.name,
                "checkbox elements require a `bind:` attribute, e.g. `checkbox(bind: self.my_checkbox)`",
            )
            .to_compile_error();
        }
    };

    // Parse optional attributes
    let mut on_change = quote! { None };

    for attr in &elem.attrs {
        let name_str = attr.name.to_string();
        if name_str == "on_change"
            && let Some(AttrValue::Ident(i)) = &attr.value
        {
            let handler_name = i.to_string();
            on_change = quote! { Some(rafter::keybinds::HandlerId(#handler_name.to_string())) };
        }
    }

    quote! {
        {
            let __widget = (#checkbox_component).clone();
            rafter::node::Node::Widget {
                widget: Box::new(__widget) as Box<dyn rafter::widgets::AnyWidget>,
                handlers: rafter::widgets::WidgetHandlers {
                    on_change: #on_change,
                    ..Default::default()
                },
                style: #style,
                layout: #layout,
                children: Vec::new(),
            }
        }
    }
}
