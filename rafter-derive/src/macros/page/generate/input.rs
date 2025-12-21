//! Input element code generation.

use proc_macro2::TokenStream;
use quote::quote;

use crate::macros::page::ast::{AttrValue, ElementNode};

use super::style::generate_style;

/// Generate code for an input element
pub fn generate_input_element(elem: &ElementNode) -> TokenStream {
    let style = generate_style(&elem.attrs);

    // Find the bind: attribute - required for input elements
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

    let input_component = match bind_expr {
        Some(expr) => expr,
        None => {
            return syn::Error::new_spanned(
                &elem.name,
                "input elements require a `bind:` attribute, e.g. `input(bind: self.my_input)`",
            )
            .to_compile_error();
        }
    };

    // Parse optional attributes
    let mut placeholder_override: Option<TokenStream> = None;
    let mut on_change = quote! { None };
    let mut on_submit = quote! { None };

    for attr in &elem.attrs {
        let name_str = attr.name.to_string();
        match name_str.as_str() {
            "placeholder" => {
                if let Some(AttrValue::Str(s)) = &attr.value {
                    placeholder_override = Some(quote! { #s.to_string() });
                } else if let Some(AttrValue::Expr(e)) = &attr.value {
                    placeholder_override = Some(quote! { (#e).to_string() });
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

    let placeholder = match placeholder_override {
        Some(p) => p,
        None => quote! { __component.placeholder() },
    };

    quote! {
        {
            let __component = (#input_component).clone();
            rafter::node::Node::Input {
                value: __component.value(),
                placeholder: #placeholder,
                on_change: #on_change,
                on_submit: #on_submit,
                id: __component.id_string(),
                style: #style,
                widget: Some(__component),
            }
        }
    }
}
