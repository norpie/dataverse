//! Element generation for col/row/box/text and widgets.

use proc_macro2::TokenStream;
use quote::quote;

use crate::macros::page::ast::{Attr, AttrValue, ElementNode};

use super::generate_view_node;
use super::handler;
use super::style;
use super::transition;

/// Generate code for an element node
pub fn generate(elem: &ElementNode) -> TokenStream {
    let name_str = elem.name.to_string();

    match name_str.as_str() {
        "column" | "col" => generate_container(elem, quote! { tuidom::Element::col() }),
        "row" => generate_container(elem, quote! { tuidom::Element::row() }),
        "box" | "box_" => generate_container(elem, quote! { tuidom::Element::box_() }),
        "text" => generate_text(elem),
        _ => generate_widget(elem),
    }
}

/// Generate a container element (col/row/box)
fn generate_container(elem: &ElementNode, constructor: TokenStream) -> TokenStream {
    let layout_refs: Vec<_> = elem.layout_attrs.iter().collect();
    let style_refs: Vec<_> = elem.style_attrs.iter().collect();
    let layout_calls = generate_layout_calls(&layout_refs);
    let style_call = style::generate_merged_style(&style_refs);
    let transition_call = transition::generate_transitions(&elem.transition_attrs);
    let children: Vec<_> = elem.children.iter().map(generate_view_node).collect();

    if children.is_empty() {
        quote! {
            #constructor
                #(#layout_calls)*
                #style_call
                #transition_call
        }
    } else {
        quote! {
            #constructor
                #(#layout_calls)*
                #style_call
                #transition_call
                .children(vec![#(#children),*])
        }
    }
}

/// Generate a text element
fn generate_text(elem: &ElementNode) -> TokenStream {
    // Find content attribute in layout attrs (content is a special text attr)
    let content = elem
        .layout_attrs
        .iter()
        .find(|a| a.name == "content")
        .map(|a| generate_attr_value(&a.value))
        .unwrap_or_else(|| quote! { "" });

    let layout_refs: Vec<_> = elem.layout_attrs.iter().collect();
    let style_refs: Vec<_> = elem.style_attrs.iter().collect();
    let layout_calls = generate_layout_calls(&layout_refs);
    let style_call = style::generate_merged_style(&style_refs);
    let transition_call = transition::generate_transitions(&elem.transition_attrs);

    quote! {
        tuidom::Element::text(#content)
            #(#layout_calls)*
            #style_call
            #transition_call
    }
}

/// Generate a widget element (unknown element name = widget)
fn generate_widget(elem: &ElementNode) -> TokenStream {
    let name = &elem.name;

    // Layout and style attrs are now separate from widget attrs
    let layout_refs: Vec<_> = elem.layout_attrs.iter().collect();
    let style_refs: Vec<_> = elem.style_attrs.iter().collect();
    let layout_calls = generate_layout_calls(&layout_refs);
    let style_call = style::generate_merged_style(&style_refs);
    let transition_call = transition::generate_transitions(&elem.transition_attrs);

    // Widget attrs are everything in layout_attrs that isn't a layout attr
    let widget_attrs: Vec<_> = elem
        .layout_attrs
        .iter()
        .filter(|a| !is_layout_attr(&a.name.to_string()))
        .collect();
    let widget_attr_calls = generate_widget_attr_calls(&widget_attrs);

    // Generate handler calls (on_click, on_change, etc.)
    let handler_calls = handler::generate_handler_calls(&elem.handlers);

    // Widget must be in scope - users import rafter widgets or define their own
    quote! {
        {
            #name::new()
                #(#widget_attr_calls)*
                #(#handler_calls)*
                .element()
                #(#layout_calls)*
                #style_call
                #transition_call
        }
    }
}

/// Check if an attribute is a layout attribute
fn is_layout_attr(name: &str) -> bool {
    matches!(
        name,
        "padding"
            | "margin"
            | "gap"
            | "width"
            | "height"
            | "min_width"
            | "max_width"
            | "min_height"
            | "max_height"
            | "direction"
            | "justify"
            | "align"
            | "wrap"
            | "flex_grow"
            | "flex_shrink"
            | "align_self"
            | "overflow"
            | "position"
            | "top"
            | "left"
            | "right"
            | "bottom"
            | "z_index"
            | "id"
            | "focusable"
            | "clickable"
            | "draggable"
    )
}

/// Generate layout method calls for attributes
fn generate_layout_calls(attrs: &[&Attr]) -> Vec<TokenStream> {
    let mut calls = Vec::new();

    for attr in attrs {
        let name_str = attr.name.to_string();
        let value = generate_attr_value(&attr.value);

        match name_str.as_str() {
            "padding" => calls.push(generate_edges_call("padding", &attr.value)),
            "margin" => calls.push(generate_edges_call("margin", &attr.value)),
            "gap" => calls.push(quote! { .gap(#value as u16) }),
            "width" => calls.push(generate_size_call("width", &attr.value)),
            "height" => calls.push(generate_size_call("height", &attr.value)),
            "min_width" => calls.push(quote! { .min_width(#value as u16) }),
            "max_width" => calls.push(quote! { .max_width(#value as u16) }),
            "min_height" => calls.push(quote! { .min_height(#value as u16) }),
            "max_height" => calls.push(quote! { .max_height(#value as u16) }),
            "direction" => calls.push(generate_direction_call(&attr.value)),
            "justify" => calls.push(generate_justify_call(&attr.value)),
            "align" => calls.push(generate_align_call(&attr.value)),
            "wrap" => calls.push(generate_wrap_call(&attr.value)),
            "flex_grow" => calls.push(quote! { .flex_grow(#value as u16) }),
            "flex_shrink" => calls.push(quote! { .flex_shrink(#value as u16) }),
            "align_self" => calls.push(generate_align_self_call(&attr.value)),
            "overflow" => calls.push(generate_overflow_call(&attr.value)),
            "position" => calls.push(generate_position_call(&attr.value)),
            "top" => calls.push(quote! { .top(#value as i16) }),
            "left" => calls.push(quote! { .left(#value as i16) }),
            "right" => calls.push(quote! { .right(#value as i16) }),
            "bottom" => calls.push(quote! { .bottom(#value as i16) }),
            "z_index" => calls.push(quote! { .z_index(#value as i16) }),
            "id" => calls.push(quote! { .id(#value) }),
            "focusable" => calls.push(quote! { .focusable(#value) }),
            "clickable" => calls.push(quote! { .clickable(#value) }),
            "draggable" => calls.push(quote! { .draggable(#value) }),
            _ => {}
        }
    }

    calls
}

/// Generate widget attribute method calls
fn generate_widget_attr_calls(attrs: &[&Attr]) -> Vec<TokenStream> {
    attrs
        .iter()
        .map(|attr| {
            let name = &attr.name;
            let value = generate_attr_value(&attr.value);
            quote! { .#name(#value) }
        })
        .collect()
}

/// Generate token stream for an attribute value
fn generate_attr_value(value: &AttrValue) -> TokenStream {
    match value {
        AttrValue::Ident(ident) => quote! { #ident },
        AttrValue::Lit(lit) => quote! { #lit },
        AttrValue::Expr(expr) => quote! { #expr },
    }
}

/// Generate edges call (padding/margin)
fn generate_edges_call(method: &str, value: &AttrValue) -> TokenStream {
    let method_ident = syn::Ident::new(method, proc_macro2::Span::call_site());

    match value {
        AttrValue::Lit(syn::Lit::Int(i)) => {
            let val = i.base10_parse::<u16>().unwrap_or(0);
            quote! { .#method_ident(tuidom::Edges::all(#val)) }
        }
        AttrValue::Expr(expr) => {
            quote! { .#method_ident(tuidom::Edges::all(#expr as u16)) }
        }
        _ => {
            let val = generate_attr_value(value);
            quote! { .#method_ident(#val) }
        }
    }
}

/// Generate size call (width/height)
fn generate_size_call(method: &str, value: &AttrValue) -> TokenStream {
    let method_ident = syn::Ident::new(method, proc_macro2::Span::call_site());

    match value {
        AttrValue::Ident(ident) => {
            let ident_str = ident.to_string();
            match ident_str.as_str() {
                "auto" => quote! { .#method_ident(tuidom::Size::Auto) },
                "fill" => quote! { .#method_ident(tuidom::Size::Fill) },
                _ => quote! { .#method_ident(tuidom::Size::Fixed(#ident as u16)) },
            }
        }
        AttrValue::Lit(syn::Lit::Int(i)) => {
            let val = i.base10_parse::<u16>().unwrap_or(0);
            quote! { .#method_ident(tuidom::Size::Fixed(#val)) }
        }
        AttrValue::Lit(syn::Lit::Str(s)) => {
            let s_val = s.value();
            if s_val.ends_with('%') {
                let pct: u16 = s_val.trim_end_matches('%').parse().unwrap_or(100);
                quote! { .#method_ident(tuidom::Size::Percent(#pct)) }
            } else {
                quote! { .#method_ident(tuidom::Size::Fixed(#s_val.parse().unwrap_or(0))) }
            }
        }
        AttrValue::Expr(expr) => {
            quote! { .#method_ident(#expr) }
        }
        _ => quote! {},
    }
}

/// Generate direction call
fn generate_direction_call(value: &AttrValue) -> TokenStream {
    match value {
        AttrValue::Ident(ident) => {
            let ident_str = ident.to_string();
            match ident_str.as_str() {
                "row" | "horizontal" => quote! { .direction(tuidom::Direction::Row) },
                "column" | "col" | "vertical" => quote! { .direction(tuidom::Direction::Column) },
                _ => quote! { .direction(#ident) },
            }
        }
        _ => {
            let val = generate_attr_value(value);
            quote! { .direction(#val) }
        }
    }
}

/// Generate justify call
fn generate_justify_call(value: &AttrValue) -> TokenStream {
    match value {
        AttrValue::Ident(ident) => {
            let ident_str = ident.to_string();
            match ident_str.as_str() {
                "start" => quote! { .justify(tuidom::Justify::Start) },
                "end" => quote! { .justify(tuidom::Justify::End) },
                "center" => quote! { .justify(tuidom::Justify::Center) },
                "between" | "space_between" => {
                    quote! { .justify(tuidom::Justify::SpaceBetween) }
                }
                "around" | "space_around" => quote! { .justify(tuidom::Justify::SpaceAround) },
                "evenly" | "space_evenly" => quote! { .justify(tuidom::Justify::SpaceEvenly) },
                _ => quote! { .justify(#ident) },
            }
        }
        _ => {
            let val = generate_attr_value(value);
            quote! { .justify(#val) }
        }
    }
}

/// Generate align call
fn generate_align_call(value: &AttrValue) -> TokenStream {
    match value {
        AttrValue::Ident(ident) => {
            let ident_str = ident.to_string();
            match ident_str.as_str() {
                "start" => quote! { .align(tuidom::Align::Start) },
                "end" => quote! { .align(tuidom::Align::End) },
                "center" => quote! { .align(tuidom::Align::Center) },
                "stretch" => quote! { .align(tuidom::Align::Stretch) },
                _ => quote! { .align(#ident) },
            }
        }
        _ => {
            let val = generate_attr_value(value);
            quote! { .align(#val) }
        }
    }
}

/// Generate align_self call
fn generate_align_self_call(value: &AttrValue) -> TokenStream {
    match value {
        AttrValue::Ident(ident) => {
            let ident_str = ident.to_string();
            match ident_str.as_str() {
                "start" => quote! { .align_self(tuidom::Align::Start) },
                "end" => quote! { .align_self(tuidom::Align::End) },
                "center" => quote! { .align_self(tuidom::Align::Center) },
                "stretch" => quote! { .align_self(tuidom::Align::Stretch) },
                _ => quote! { .align_self(#ident) },
            }
        }
        _ => {
            let val = generate_attr_value(value);
            quote! { .align_self(#val) }
        }
    }
}

/// Generate wrap call
fn generate_wrap_call(value: &AttrValue) -> TokenStream {
    match value {
        AttrValue::Ident(ident) => {
            let ident_str = ident.to_string();
            match ident_str.as_str() {
                "no_wrap" | "nowrap" => quote! { .wrap(tuidom::Wrap::NoWrap) },
                "wrap" => quote! { .wrap(tuidom::Wrap::Wrap) },
                "reverse" | "wrap_reverse" => quote! { .wrap(tuidom::Wrap::WrapReverse) },
                _ => quote! { .wrap(#ident) },
            }
        }
        _ => {
            let val = generate_attr_value(value);
            quote! { .wrap(#val) }
        }
    }
}

/// Generate overflow call
fn generate_overflow_call(value: &AttrValue) -> TokenStream {
    match value {
        AttrValue::Ident(ident) => {
            let ident_str = ident.to_string();
            match ident_str.as_str() {
                "visible" => quote! { .overflow(tuidom::Overflow::Visible) },
                "hidden" => quote! { .overflow(tuidom::Overflow::Hidden) },
                "scroll" => quote! { .overflow(tuidom::Overflow::Scroll) },
                _ => quote! { .overflow(#ident) },
            }
        }
        _ => {
            let val = generate_attr_value(value);
            quote! { .overflow(#val) }
        }
    }
}

/// Generate position call
fn generate_position_call(value: &AttrValue) -> TokenStream {
    match value {
        AttrValue::Ident(ident) => {
            let ident_str = ident.to_string();
            match ident_str.as_str() {
                "relative" => quote! { .position(tuidom::Position::Relative) },
                "absolute" => quote! { .position(tuidom::Position::Absolute) },
                _ => quote! { .position(#ident) },
            }
        }
        _ => {
            let val = generate_attr_value(value);
            quote! { .position(#val) }
        }
    }
}
