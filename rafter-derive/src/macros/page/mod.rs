//! The page! macro for declarative UI definitions.
//!
//! Outputs `tuidom::Element` using a builder pattern.

pub mod ast;
mod generate;
pub mod parse;

use proc_macro2::TokenStream;
use syn::parse2;

use ast::{Page, ViewNode};

/// Expand the page! macro (with handler support)
pub fn expand(input: TokenStream) -> TokenStream {
    let page: Page = match parse2(input) {
        Ok(p) => p,
        Err(e) => return e.to_compile_error(),
    };

    generate::generate(&page)
}

/// Expand the element! macro (without handler support)
pub fn expand_element(input: TokenStream) -> TokenStream {
    let page: Page = match parse2(input) {
        Ok(p) => p,
        Err(e) => return e.to_compile_error(),
    };

    // Validate that no handlers are present
    if let Err(e) = validate_no_handlers(&page.root) {
        return e.to_compile_error();
    }

    generate::generate_element(&page)
}

/// Recursively check if any handlers are present in the view tree
fn validate_no_handlers(node: &ViewNode) -> syn::Result<()> {
    match node {
        ViewNode::Element(elem) => {
            // Check if this element has handlers
            if !elem.handlers.is_empty() {
                return Err(syn::Error::new_spanned(
                    &elem.name,
                    "element! macro does not support handlers. Use page! macro instead, or remove the handler.",
                ));
            }

            // Recursively check children
            for child in &elem.children {
                validate_no_handlers(child)?;
            }
            Ok(())
        }
        ViewNode::For(for_node) => {
            for child in &for_node.body {
                validate_no_handlers(child)?;
            }
            Ok(())
        }
        ViewNode::If(if_node) => {
            for child in &if_node.then_branch {
                validate_no_handlers(child)?;
            }
            if let Some(else_branch) = &if_node.else_branch {
                match else_branch {
                    ast::ElseBranch::Else(children) => {
                        for child in children {
                            validate_no_handlers(child)?;
                        }
                    }
                    ast::ElseBranch::ElseIf(else_if) => {
                        validate_no_handlers_if(else_if)?;
                    }
                }
            }
            Ok(())
        }
        ViewNode::Match(match_node) => {
            for arm in &match_node.arms {
                for child in &arm.body {
                    validate_no_handlers(child)?;
                }
            }
            Ok(())
        }
        ViewNode::Expr(_) | ViewNode::Spread(_) => Ok(()),
    }
}

/// Helper to validate an IfNode without converting to ViewNode
fn validate_no_handlers_if(if_node: &ast::IfNode) -> syn::Result<()> {
    for child in &if_node.then_branch {
        validate_no_handlers(child)?;
    }
    if let Some(else_branch) = &if_node.else_branch {
        match else_branch {
            ast::ElseBranch::Else(children) => {
                for child in children {
                    validate_no_handlers(child)?;
                }
            }
            ast::ElseBranch::ElseIf(else_if) => {
                validate_no_handlers_if(else_if)?;
            }
        }
    }
    Ok(())
}
