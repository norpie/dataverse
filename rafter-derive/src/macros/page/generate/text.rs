//! Text element code generation.

use proc_macro2::TokenStream;
use quote::quote;

use crate::macros::page::ast::{ElementNode, TextNode, ViewNode};

use super::style::generate_style;

/// Generate code for a text element
pub fn generate_text_element(elem: &ElementNode) -> TokenStream {
    let style = generate_style(&elem.attrs);

    // Get text content from children
    let content = if elem.children.is_empty() {
        quote! { String::new() }
    } else if elem.children.len() == 1 {
        match &elem.children[0] {
            ViewNode::Text(TextNode::Literal(s)) => quote! { #s.to_string() },
            ViewNode::Text(TextNode::Expr(e)) => quote! { #e.to_string() },
            ViewNode::Expr(e) => quote! { #e.to_string() },
            _ => quote! { String::new() },
        }
    } else {
        // Multiple children - concatenate
        let parts: Vec<_> = elem
            .children
            .iter()
            .map(|c| match c {
                ViewNode::Text(TextNode::Literal(s)) => quote! { #s },
                ViewNode::Text(TextNode::Expr(e)) => quote! { &#e.to_string() },
                ViewNode::Expr(e) => quote! { &#e.to_string() },
                _ => quote! { "" },
            })
            .collect();
        quote! { format!("{}", [#(#parts),*].concat()) }
    };

    quote! {
        rafter::node::Node::Text {
            content: #content,
            style: #style,
        }
    }
}

/// Generate code for a standalone text node
pub fn generate_text(text: &TextNode) -> TokenStream {
    match text {
        TextNode::Literal(s) => {
            quote! {
                rafter::node::Node::text(#s)
            }
        }
        TextNode::Expr(e) => {
            quote! {
                rafter::node::Node::text(#e.to_string())
            }
        }
    }
}
