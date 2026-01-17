//! Code generation for the page! macro.

mod element;
pub mod handler;
pub mod style;
pub mod transition;

use proc_macro2::TokenStream;
use quote::quote;

use super::ast::{AttrValue, AttrValueElse, AttrValueIf, ElseBranch, ForNode, IfNode, MatchNode, Page, ViewNode};

/// Generate code for a conditional attribute value.
///
/// This handles `AttrValue::If` variants by generating if-else expressions,
/// recursively calling the converter function for leaf values.
///
/// # Arguments
/// * `value` - The attribute value to generate code for
/// * `convert_leaf` - A function to convert leaf values (Ident, Lit, Expr, BareFlag) to TokenStream
///
/// # Example
/// For `if active { primary } else { muted }` with a color converter:
/// ```ignore
/// if active {
///     tuidom::Color::var("primary")
/// } else {
///     tuidom::Color::var("muted")
/// }
/// ```
pub fn generate_conditional_attr_value<F>(value: &AttrValue, convert_leaf: F) -> TokenStream
where
    F: Fn(&AttrValue) -> TokenStream + Copy,
{
    match value {
        AttrValue::If {
            cond,
            then_value,
            else_branch,
        } => {
            let then_code = generate_conditional_attr_value(then_value, convert_leaf);
            let else_code = generate_conditional_else_branch(else_branch, convert_leaf);
            quote! {
                if #cond {
                    #then_code
                } else {
                    #else_code
                }
            }
        }
        _ => convert_leaf(value),
    }
}

/// Generate code for the else branch of a conditional attribute value.
fn generate_conditional_else_branch<F>(else_branch: &AttrValueElse, convert_leaf: F) -> TokenStream
where
    F: Fn(&AttrValue) -> TokenStream + Copy,
{
    match else_branch {
        AttrValueElse::Else(value) => generate_conditional_attr_value(value, convert_leaf),
        AttrValueElse::ElseIf(if_node) => generate_conditional_attr_value_if(if_node, convert_leaf),
    }
}

/// Generate code for an AttrValueIf (used in else-if chains).
fn generate_conditional_attr_value_if<F>(if_node: &AttrValueIf, convert_leaf: F) -> TokenStream
where
    F: Fn(&AttrValue) -> TokenStream + Copy,
{
    let cond = &if_node.cond;
    let then_code = generate_conditional_attr_value(&if_node.then_value, convert_leaf);
    let else_code = generate_conditional_else_branch(&if_node.else_branch, convert_leaf);
    quote! {
        if #cond {
            #then_code
        } else {
            #else_code
        }
    }
}

/// Check if an AttrValue contains a conditional (If variant).
pub fn is_conditional(value: &AttrValue) -> bool {
    matches!(value, AttrValue::If { .. })
}

/// Generate code for the entire page
pub fn generate(page: &Page) -> TokenStream {
    generate_view_node(&page.root)
}

/// Generate code for a view node
pub fn generate_view_node(node: &ViewNode) -> TokenStream {
    match node {
        ViewNode::Element(elem) => element::generate(elem),
        ViewNode::For(for_node) => generate_for(for_node),
        ViewNode::If(if_node) => generate_if(if_node),
        ViewNode::Match(match_node) => generate_match(match_node),
        ViewNode::Expr(expr) => quote! { #expr },
    }
}

/// Generate code for a for loop
fn generate_for(node: &ForNode) -> TokenStream {
    let pat = &node.pat;
    let iter = &node.iter;
    let body: Vec<_> = node.body.iter().map(generate_view_node).collect();

    // Use __page_spread marker - will be detected and flattened by parent
    quote! {{
        let __page_spread: Vec<tuidom::Element> = (#iter).into_iter().flat_map(|#pat| {
            std::vec![#(#body),*]
        }).collect();
        __page_spread
    }}
}

/// Generate code for an if statement
fn generate_if(node: &IfNode) -> TokenStream {
    let cond = &node.cond;
    let then_body: Vec<_> = node.then_branch.iter().map(generate_view_node).collect();

    let then_branch = if then_body.len() == 1 {
        let first = &then_body[0];
        quote! { #first }
    } else {
        quote! { tuidom::Element::col().children(vec![#(#then_body),*]) }
    };

    match &node.else_branch {
        Some(ElseBranch::Else(else_body)) => {
            let else_children: Vec<_> = else_body.iter().map(generate_view_node).collect();
            let else_branch = if else_children.len() == 1 {
                let first = &else_children[0];
                quote! { #first }
            } else {
                quote! { tuidom::Element::col().children(vec![#(#else_children),*]) }
            };
            quote! {
                if #cond {
                    #then_branch
                } else {
                    #else_branch
                }
            }
        }
        Some(ElseBranch::ElseIf(else_if)) => {
            let else_if_code = generate_if(else_if);
            quote! {
                if #cond {
                    #then_branch
                } else {
                    #else_if_code
                }
            }
        }
        None => {
            // No else branch - return empty element if condition is false
            quote! {
                if #cond {
                    #then_branch
                } else {
                    tuidom::Element::col()
                }
            }
        }
    }
}

/// Generate code for a match expression
fn generate_match(node: &MatchNode) -> TokenStream {
    let expr = &node.expr;
    let arms: Vec<_> = node
        .arms
        .iter()
        .map(|arm| {
            let pat = &arm.pat;
            let body: Vec<_> = arm.body.iter().map(generate_view_node).collect();

            let body_code = if body.len() == 1 {
                let first = &body[0];
                quote! { #first }
            } else {
                quote! { tuidom::Element::col().children(vec![#(#body),*]) }
            };

            if let Some(guard) = &arm.guard {
                quote! { #pat if #guard => #body_code }
            } else {
                quote! { #pat => #body_code }
            }
        })
        .collect();

    quote! {
        match #expr {
            #(#arms),*
        }
    }
}
