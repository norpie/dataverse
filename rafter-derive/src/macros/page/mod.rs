//! The `page!` macro for building UI trees.

pub mod ast;
mod generate;
mod parse;

use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::parse2;

use ast::ViewNode;
use generate::generate_node;

/// The main entry point for the page macro
struct ViewInput {
    nodes: Vec<ViewNode>,
}

impl Parse for ViewInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut nodes = Vec::new();
        while !input.is_empty() {
            nodes.push(input.parse()?);
        }
        Ok(Self { nodes })
    }
}

pub fn expand(input: TokenStream) -> TokenStream {
    let view_input: ViewInput = match parse2(input) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error(),
    };

    if view_input.nodes.is_empty() {
        return quote! { rafter::node::Node::Empty };
    }

    if view_input.nodes.len() == 1 {
        return generate_node(&view_input.nodes[0]);
    }

    // Multiple top-level nodes - wrap in a column
    let nodes: Vec<_> = view_input.nodes.iter().map(generate_node).collect();
    quote! {
        rafter::node::Node::Column {
            children: vec![#(#nodes),*],
            style: rafter::style::Style::new(),
            layout: rafter::node::Layout::default(),
            id: None,
        }
    }
}
