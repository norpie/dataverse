//! Code generation for view nodes.

pub mod button;
pub mod checkbox;
pub mod container;
pub mod input;
pub mod layout;
pub mod list;
pub mod radio_group;
pub mod scroll_area;
pub mod style;
pub mod table;
pub mod text;
pub mod tree;

use proc_macro2::TokenStream;
use quote::quote;

use crate::macros::view::ast::{AttrValue, ControlFlowNode, ElementNode, ViewNode};

pub use button::generate_button_element;
pub use checkbox::generate_checkbox_element;
pub use container::generate_container;
pub use input::generate_input_element;
pub use list::generate_list_element;
pub use radio_group::generate_radio_group_element;
pub use scroll_area::generate_scroll_area_element;
pub use table::generate_table_element;
pub use text::{generate_text, generate_text_element};
pub use tree::generate_tree_element;

/// Generate code for a view node
pub fn generate_node(node: &ViewNode) -> TokenStream {
    match node {
        ViewNode::Element(elem) => generate_element(elem),
        ViewNode::Text(text) => generate_text(text),
        ViewNode::ControlFlow(cf) => generate_control_flow(cf),
        ViewNode::Expr(expr) => {
            // Expression should produce a Node or something that can convert to Node
            quote! { #expr }
        }
    }
}

/// Generate code for an element node
fn generate_element(elem: &ElementNode) -> TokenStream {
    let name_str = elem.name.to_string();

    match name_str.as_str() {
        "column" => generate_container(elem, quote! { rafter::node::Node::Column }),
        "row" => generate_container(elem, quote! { rafter::node::Node::Row }),
        "stack" => generate_container(elem, quote! { rafter::node::Node::Stack }),
        "text" => generate_text_element(elem),
        "input" => generate_input_element(elem),
        "button" => generate_button_element(elem),
        "checkbox" => generate_checkbox_element(elem),
        "radio_group" => generate_radio_group_element(elem),
        "scroll_area" => generate_scroll_area_element(elem),
        "list" => generate_list_element(elem),
        "tree" => generate_tree_element(elem),
        "table" => generate_table_element(elem),
        _ => {
            // Unknown element - treat as a component function call
            let name = &elem.name;
            let args = generate_component_args(elem);
            quote! { #name(#args) }
        }
    }
}

/// Generate arguments for a component function call
fn generate_component_args(elem: &ElementNode) -> TokenStream {
    let args: Vec<_> = elem
        .attrs
        .iter()
        .map(|attr| {
            let name = &attr.name;
            match &attr.value {
                Some(AttrValue::Int(v)) => quote! { #name: #v },
                Some(AttrValue::Str(s)) => quote! { #name: #s },
                Some(AttrValue::Bool(b)) => quote! { #name: #b },
                Some(AttrValue::Ident(i)) => quote! { #name: #i },
                Some(AttrValue::Expr(e)) => quote! { #name: #e },
                None => quote! { #name: true },
            }
        })
        .collect();

    if args.is_empty() {
        quote! {}
    } else {
        quote! { #(#args),* }
    }
}

/// Generate code for control flow nodes (if, for, match)
fn generate_control_flow(cf: &ControlFlowNode) -> TokenStream {
    match cf {
        ControlFlowNode::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let then_nodes: Vec<_> = then_branch.iter().map(generate_node).collect();
            let then_code = if then_nodes.len() == 1 {
                then_nodes[0].clone()
            } else {
                quote! {
                    rafter::node::Node::Column {
                        children: vec![#(#then_nodes),*],
                        style: rafter::style::Style::new(),
                        layout: rafter::node::Layout::default(),
                    }
                }
            };

            let else_code = if let Some(else_branch) = else_branch {
                let else_nodes: Vec<_> = else_branch.iter().map(generate_node).collect();
                if else_nodes.len() == 1 {
                    else_nodes[0].clone()
                } else {
                    quote! {
                        rafter::node::Node::Column {
                            children: vec![#(#else_nodes),*],
                            style: rafter::style::Style::new(),
                            layout: rafter::node::Layout::default(),
                        }
                    }
                }
            } else {
                quote! { rafter::node::Node::Empty }
            };

            quote! {
                if #condition {
                    #then_code
                } else {
                    #else_code
                }
            }
        }
        ControlFlowNode::IfLet {
            pattern,
            expr,
            then_branch,
            else_branch,
        } => {
            let then_nodes: Vec<_> = then_branch.iter().map(generate_node).collect();
            let then_code = if then_nodes.len() == 1 {
                then_nodes[0].clone()
            } else {
                quote! {
                    rafter::node::Node::Column {
                        children: vec![#(#then_nodes),*],
                        style: rafter::style::Style::new(),
                        layout: rafter::node::Layout::default(),
                    }
                }
            };

            let else_code = if let Some(else_branch) = else_branch {
                let else_nodes: Vec<_> = else_branch.iter().map(generate_node).collect();
                if else_nodes.len() == 1 {
                    else_nodes[0].clone()
                } else {
                    quote! {
                        rafter::node::Node::Column {
                            children: vec![#(#else_nodes),*],
                            style: rafter::style::Style::new(),
                            layout: rafter::node::Layout::default(),
                        }
                    }
                }
            } else {
                quote! { rafter::node::Node::Empty }
            };

            quote! {
                if let #pattern = #expr {
                    #then_code
                } else {
                    #else_code
                }
            }
        }
        ControlFlowNode::For {
            pattern,
            iter,
            body,
        } => {
            let body_nodes: Vec<_> = body.iter().map(generate_node).collect();
            let body_code = if body_nodes.len() == 1 {
                body_nodes[0].clone()
            } else {
                quote! {
                    rafter::node::Node::Column {
                        children: vec![#(#body_nodes),*],
                        style: rafter::style::Style::new(),
                        layout: rafter::node::Layout::default(),
                    }
                }
            };

            quote! {
                rafter::node::Node::Column {
                    children: (#iter).into_iter().map(|#pattern| {
                        #body_code
                    }).collect(),
                    style: rafter::style::Style::new(),
                    layout: rafter::node::Layout::default(),
                }
            }
        }
        ControlFlowNode::Match { expr, arms } => {
            let arm_code: Vec<_> = arms
                .iter()
                .map(|arm| {
                    let pattern = &arm.pattern;
                    let guard = arm.guard.as_ref().map(|g| quote! { if #g });
                    let body_nodes: Vec<_> = arm.body.iter().map(generate_node).collect();
                    let body_code = if body_nodes.len() == 1 {
                        body_nodes[0].clone()
                    } else {
                        quote! {
                            rafter::node::Node::Column {
                                children: vec![#(#body_nodes),*],
                                style: rafter::style::Style::new(),
                                layout: rafter::node::Layout::default(),
                            }
                        }
                    };

                    quote! {
                        #pattern #guard => #body_code
                    }
                })
                .collect();

            quote! {
                match #expr {
                    #(#arm_code),*
                }
            }
        }
    }
}
