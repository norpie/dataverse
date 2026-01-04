//! Transition generation for the page! macro.

use proc_macro2::TokenStream;
use quote::quote;

use crate::macros::page::ast::TransitionAttr;

/// Generate a `.transitions(...)` call from transition attributes.
///
/// Returns `None` if there are no transitions.
pub fn generate_transitions(attrs: &[TransitionAttr]) -> Option<TokenStream> {
    if attrs.is_empty() {
        return None;
    }

    let methods: Vec<TokenStream> = attrs.iter().map(generate_transition_method).collect();

    Some(quote! {
        .transitions(tuidom::Transitions::new()#(#methods)*)
    })
}

/// Generate a single transition method call.
fn generate_transition_method(attr: &TransitionAttr) -> TokenStream {
    let property = &attr.property;
    let property_str = property.to_string();
    let duration_ms = attr.duration_ms;

    // Map property names to tuidom::Transitions methods
    let method_name = match property_str.as_str() {
        "bg" | "background" => "background",
        "fg" | "foreground" => "foreground",
        "all" => "all",
        "position" => "position",
        "size" => "size",
        "colors" => "colors",
        "width" => "width",
        "height" => "height",
        "left" => "left",
        "top" => "top",
        "right" => "right",
        "bottom" => "bottom",
        _ => {
            // Unknown property, use as-is
            &property_str
        }
    };

    let method_ident = syn::Ident::new(method_name, property.span());

    // Generate easing
    let easing = generate_easing(attr.easing.as_ref());

    quote! {
        .#method_ident(std::time::Duration::from_millis(#duration_ms), #easing)
    }
}

/// Generate easing enum from optional ident.
fn generate_easing(easing: Option<&syn::Ident>) -> TokenStream {
    match easing {
        Some(ident) => {
            let easing_str = ident.to_string();
            match easing_str.as_str() {
                "linear" => quote! { tuidom::Easing::Linear },
                "ease_in" | "easein" => quote! { tuidom::Easing::EaseIn },
                "ease_out" | "easeout" => quote! { tuidom::Easing::EaseOut },
                "ease_in_out" | "easeinout" => quote! { tuidom::Easing::EaseInOut },
                _ => {
                    // Unknown easing, pass as-is (will be a compile error if wrong)
                    quote! { #ident }
                }
            }
        }
        None => {
            // Default to Linear
            quote! { tuidom::Easing::Linear }
        }
    }
}
