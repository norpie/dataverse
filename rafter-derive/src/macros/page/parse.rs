//! Parsing logic for the page macro.

use syn::{
    Expr, Ident, LitStr, Token, braced, parenthesized,
    parse::{Parse, ParseStream},
    token::Brace,
};

use super::ast::{Attr, AttrValue, ControlFlowNode, ElementNode, MatchArm, TextNode, ViewNode};

impl Parse for ViewNode {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Check for control flow keywords
        if input.peek(Token![if]) {
            return Ok(ViewNode::ControlFlow(Box::new(parse_if(input)?)));
        }
        if input.peek(Token![for]) {
            return Ok(ViewNode::ControlFlow(Box::new(parse_for(input)?)));
        }
        if input.peek(Token![match]) {
            return Ok(ViewNode::ControlFlow(Box::new(parse_match(input)?)));
        }

        // Check for string literal
        if input.peek(LitStr) {
            let lit: LitStr = input.parse()?;
            return Ok(ViewNode::Text(TextNode::Literal(lit.value())));
        }

        // Check for braced expression
        if input.peek(Brace) {
            let content;
            braced!(content in input);
            let expr: Expr = content.parse()?;
            return Ok(ViewNode::Expr(expr));
        }

        // Check for `self` keyword - parse as expression
        if input.peek(Token![self]) {
            let expr: Expr = input.parse()?;
            return Ok(ViewNode::Expr(expr));
        }

        // Check if it looks like an element (identifier followed by parens or braces)
        // or a bare identifier/expression (variable reference)
        if input.peek(Ident) {
            // Look ahead to see if this is an element (has parens or braces after)
            // or just a variable reference
            let fork = input.fork();
            let _ident: Ident = fork.parse()?;

            if fork.peek(syn::token::Paren) || fork.peek(Brace) {
                // This is an element with attrs or children
                let element = parse_element(input)?;
                return Ok(ViewNode::Element(element));
            } else {
                // Bare identifier - treat as expression (variable reference)
                let expr: Expr = input.parse()?;
                return Ok(ViewNode::Expr(expr));
            }
        }

        // Fallback: try to parse as element
        let element = parse_element(input)?;
        Ok(ViewNode::Element(element))
    }
}

pub fn parse_element(input: ParseStream) -> syn::Result<ElementNode> {
    let name: Ident = input.parse()?;

    // Optional attributes in parentheses
    let attrs = if input.peek(syn::token::Paren) {
        let content;
        parenthesized!(content in input);
        parse_attrs(&content)?
    } else {
        Vec::new()
    };

    // Optional children in braces
    let children = if input.peek(Brace) {
        let content;
        braced!(content in input);
        parse_children(&content)?
    } else {
        Vec::new()
    };

    Ok(ElementNode {
        name,
        attrs,
        children,
    })
}

pub fn parse_attrs(input: ParseStream) -> syn::Result<Vec<Attr>> {
    let mut attrs = Vec::new();

    while !input.is_empty() {
        let name: Ident = input.parse()?;

        let value = if input.peek(Token![:]) {
            input.parse::<Token![:]>()?;

            // Parse the value
            if input.peek(LitStr) {
                let lit: LitStr = input.parse()?;
                Some(AttrValue::Str(lit.value()))
            } else if input.peek(syn::LitInt) {
                let lit: syn::LitInt = input.parse()?;
                Some(AttrValue::Int(lit.base10_parse()?))
            } else if input.peek(syn::LitBool) {
                let lit: syn::LitBool = input.parse()?;
                Some(AttrValue::Bool(lit.value))
            } else if input.peek(Ident) {
                // Look ahead to see if this is a simple identifier or start of an expression
                let fork = input.fork();
                let ident: Ident = fork.parse()?;

                // Check for true/false
                if ident == "true" {
                    input.parse::<Ident>()?;
                    Some(AttrValue::Bool(true))
                } else if ident == "false" {
                    input.parse::<Ident>()?;
                    Some(AttrValue::Bool(false))
                } else if fork.peek(Token![.]) || fork.peek(syn::token::Paren) {
                    // This is an expression (method call, function call, etc.)
                    let expr: Expr = input.parse()?;
                    Some(AttrValue::Expr(expr))
                } else {
                    // Simple identifier (color name, variable)
                    input.parse::<Ident>()?;
                    Some(AttrValue::Ident(ident))
                }
            } else {
                // Parse as expression
                let expr: Expr = input.parse()?;
                Some(AttrValue::Expr(expr))
            }
        } else {
            // Boolean shorthand: (bold) means (bold: true)
            Some(AttrValue::Bool(true))
        };

        attrs.push(Attr { name, value });

        // Optional comma
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }
    }

    Ok(attrs)
}

pub fn parse_children(input: ParseStream) -> syn::Result<Vec<ViewNode>> {
    let mut children = Vec::new();

    while !input.is_empty() {
        children.push(input.parse()?);
    }

    Ok(children)
}

pub fn parse_if(input: ParseStream) -> syn::Result<ControlFlowNode> {
    input.parse::<Token![if]>()?;

    // Check for if let
    if input.peek(Token![let]) {
        input.parse::<Token![let]>()?;
        let pattern: syn::Pat = syn::Pat::parse_single(input)?;
        input.parse::<Token![=]>()?;
        let expr: Expr = Expr::parse_without_eager_brace(input)?;

        let content;
        braced!(content in input);
        let then_branch = parse_children(&content)?;

        let else_branch = if input.peek(Token![else]) {
            input.parse::<Token![else]>()?;
            let content;
            braced!(content in input);
            Some(parse_children(&content)?)
        } else {
            None
        };

        return Ok(ControlFlowNode::IfLet {
            pattern,
            expr,
            then_branch,
            else_branch,
        });
    }

    // Regular if
    let condition: Expr = Expr::parse_without_eager_brace(input)?;

    let content;
    braced!(content in input);
    let then_branch = parse_children(&content)?;

    let else_branch = if input.peek(Token![else]) {
        input.parse::<Token![else]>()?;
        if input.peek(Token![if]) {
            // else if - parse as single node in else branch
            let else_if = parse_if(input)?;
            Some(vec![ViewNode::ControlFlow(Box::new(else_if))])
        } else {
            let content;
            braced!(content in input);
            Some(parse_children(&content)?)
        }
    } else {
        None
    };

    Ok(ControlFlowNode::If {
        condition,
        then_branch,
        else_branch,
    })
}

pub fn parse_for(input: ParseStream) -> syn::Result<ControlFlowNode> {
    input.parse::<Token![for]>()?;
    let pattern: syn::Pat = syn::Pat::parse_single(input)?;
    input.parse::<Token![in]>()?;
    let iter: Expr = Expr::parse_without_eager_brace(input)?;

    let content;
    braced!(content in input);
    let body = parse_children(&content)?;

    Ok(ControlFlowNode::For {
        pattern,
        iter,
        body,
    })
}

pub fn parse_match(input: ParseStream) -> syn::Result<ControlFlowNode> {
    input.parse::<Token![match]>()?;
    let expr: Expr = Expr::parse_without_eager_brace(input)?;

    let content;
    braced!(content in input);

    let mut arms = Vec::new();
    while !content.is_empty() {
        let pattern: syn::Pat = syn::Pat::parse_multi(&content)?;

        let guard = if content.peek(Token![if]) {
            content.parse::<Token![if]>()?;
            Some(content.parse()?)
        } else {
            None
        };

        content.parse::<Token![=>]>()?;

        let body = if content.peek(Brace) {
            let arm_content;
            braced!(arm_content in content);
            parse_children(&arm_content)?
        } else {
            vec![content.parse()?]
        };

        // Optional comma
        if content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
        }

        arms.push(MatchArm {
            pattern,
            guard,
            body,
        });
    }

    Ok(ControlFlowNode::Match { expr, arms })
}
