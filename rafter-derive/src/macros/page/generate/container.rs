//! Container (column/row/stack) code generation.

use proc_macro2::TokenStream;
use quote::quote;

use crate::macros::page::ast::{AttrValue, ElementNode};

use super::style::generate_style_and_layout;

/// Generate code for a container element (column, row, or stack)
pub fn generate_container(elem: &ElementNode, variant: TokenStream) -> TokenStream {
    let children: Vec<_> = elem.children.iter().map(super::generate_node).collect();
    let (style, layout) = generate_style_and_layout(&elem.attrs);

    // Check if container has an explicit ID for transition tracking
    // Note: For transitions to work on containers, an explicit id is required
    // to track style changes across renders.
    let mut explicit_id: Option<TokenStream> = None;

    for attr in &elem.attrs {
        let name_str = attr.name.to_string();
        if name_str == "id" {
            if let Some(AttrValue::Str(s)) = &attr.value {
                explicit_id = Some(quote! { Some(#s.to_string()) });
            } else if let Some(AttrValue::Expr(e)) = &attr.value {
                explicit_id = Some(quote! { Some((#e).to_string()) });
            }
        }
    }

    // Generate id field
    // Note: For transitions to work on containers, an explicit id is required
    // to track style changes across renders. Without an id, transitions are ignored.
    let id = if let Some(id_expr) = explicit_id {
        id_expr
    } else {
        quote! { None }
    };

    quote! {
        #variant {
            children: vec![#(#children),*],
            style: #style,
            layout: #layout,
            id: #id,
        }
    }
}
