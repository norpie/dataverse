//! Container (column/row/stack) code generation.

use proc_macro2::TokenStream;
use quote::quote;

use crate::macros::page::ast::ElementNode;

use super::style::generate_style_and_layout;

/// Generate code for a container element (column, row, or stack)
pub fn generate_container(elem: &ElementNode, variant: TokenStream) -> TokenStream {
    let children: Vec<_> = elem.children.iter().map(super::generate_node).collect();
    let (style, layout) = generate_style_and_layout(&elem.attrs);

    quote! {
        #variant {
            children: vec![#(#children),*],
            style: #style,
            layout: #layout,
        }
    }
}
