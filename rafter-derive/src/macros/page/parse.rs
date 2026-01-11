//! Parsing logic for the page! macro.

use syn::parse::{Parse, ParseStream};
use syn::token::{Brace, Paren};
use syn::{braced, parenthesized, Expr, Ident, Pat, Token};

use super::ast::{
    Attr, AttrValue, ElseBranch, ElementNode, ForNode, HandlerArg, HandlerAttr, IfNode, MatchArm,
    MatchNode, Page, TransitionAttr, ViewNode,
};

impl Parse for Page {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let root = parse_view_node(input)?;
        Ok(Page { root })
    }
}

/// Parse a single view node
fn parse_view_node(input: ParseStream) -> syn::Result<ViewNode> {
    // Check for control flow keywords
    if input.peek(Token![for]) {
        return Ok(ViewNode::For(parse_for(input)?));
    }
    if input.peek(Token![if]) {
        return Ok(ViewNode::If(parse_if(input)?));
    }
    if input.peek(Token![match]) {
        return Ok(ViewNode::Match(parse_match(input)?));
    }

    // Check for braced expression
    if input.peek(Brace) {
        let content;
        braced!(content in input);
        let expr: Expr = content.parse()?;
        return Ok(ViewNode::Expr(expr));
    }

    // Otherwise, it's an element
    Ok(ViewNode::Element(parse_element(input)?))
}

/// Parse an element: `name (layout_attrs) style (style_attrs) transition (trans_attrs) handlers { children }`
fn parse_element(input: ParseStream) -> syn::Result<ElementNode> {
    let name: Ident = input.parse()?;

    // Parse layout attributes in parentheses (optional)
    let layout_attrs = if input.peek(syn::token::Paren) {
        let content;
        parenthesized!(content in input);
        parse_attrs(&content)?
    } else {
        Vec::new()
    };

    // Parse style attributes: `style (bg: primary, bold: true)`
    let style_attrs = parse_keyword_attrs(input, "style")?;

    // Parse transition attributes: `transition (bg: 200ms ease_out)`
    let transition_attrs = parse_transition_attrs(input)?;

    // Parse handlers (on_click: handler(args))
    let handlers = parse_handlers(input)?;

    // Parse children in braces (optional)
    let children = if input.peek(Brace) {
        let content;
        braced!(content in input);
        parse_children(&content)?
    } else {
        Vec::new()
    };

    Ok(ElementNode {
        name,
        layout_attrs,
        style_attrs,
        transition_attrs,
        handlers,
        children,
    })
}

/// Parse attributes after a keyword: `keyword (attrs)`
fn parse_keyword_attrs(input: ParseStream, keyword: &str) -> syn::Result<Vec<Attr>> {
    // Check if next token is the keyword followed by parens
    if input.peek(Ident) && input.peek2(syn::token::Paren) {
        let fork = input.fork();
        let ident: Ident = fork.parse()?;

        if ident == keyword {
            // Consume the keyword
            let _: Ident = input.parse()?;
            // Parse attrs in parens
            let content;
            parenthesized!(content in input);
            return parse_attrs(&content);
        }
    }

    Ok(Vec::new())
}

/// Parse transition attributes: `transition (bg: 200ms ease_out, fg: 100ms)`
fn parse_transition_attrs(input: ParseStream) -> syn::Result<Vec<TransitionAttr>> {
    // Check if next token is "transition" followed by parens
    if input.peek(Ident) && input.peek2(syn::token::Paren) {
        let fork = input.fork();
        let ident: Ident = fork.parse()?;

        if ident == "transition" {
            // Consume the keyword
            let _: Ident = input.parse()?;
            // Parse transition attrs in parens
            let content;
            parenthesized!(content in input);
            return parse_transition_attr_list(&content);
        }
    }

    Ok(Vec::new())
}

/// Parse transition attribute list: `bg: 200 ease_out, fg: 100`
///
/// Duration is in milliseconds (integer literal).
fn parse_transition_attr_list(input: ParseStream) -> syn::Result<Vec<TransitionAttr>> {
    let mut attrs = Vec::new();

    while !input.is_empty() {
        let property: Ident = input.parse()?;
        input.parse::<Token![:]>()?;

        // Parse duration as integer literal (milliseconds)
        let duration_lit: syn::LitInt = input.parse()?;
        let duration_ms: u64 = duration_lit.base10_parse()?;

        // Parse optional easing (e.g., ease_out, linear)
        let easing = if input.peek(Ident) && !input.peek2(Token![:]) {
            // Next ident is easing, not a new property
            Some(input.parse()?)
        } else {
            None
        };

        attrs.push(TransitionAttr {
            property,
            duration_ms,
            easing,
        });

        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        } else {
            break;
        }
    }

    Ok(attrs)
}

/// Parse attributes: `key: value, key2: value2, bare_flag`
///
/// Supports both key-value pairs and bare flags (identifiers without values).
/// Bare flags generate `.flag()` method calls with no arguments.
fn parse_attrs(input: ParseStream) -> syn::Result<Vec<Attr>> {
    let mut attrs = Vec::new();

    while !input.is_empty() {
        let name: Ident = input.parse()?;

        // Check if this is a key: value pair or a bare flag
        let value = if input.peek(Token![:]) {
            input.parse::<Token![:]>()?;
            parse_attr_value(input)?
        } else {
            // Bare flag (e.g., `disabled`, `small`)
            AttrValue::BareFlag
        };

        attrs.push(Attr { name, value });

        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        } else {
            break;
        }
    }

    Ok(attrs)
}

/// Parse an attribute value
fn parse_attr_value(input: ParseStream) -> syn::Result<AttrValue> {
    // Check for braced expression
    if input.peek(Brace) {
        let content;
        braced!(content in input);
        let expr: Expr = content.parse()?;
        return Ok(AttrValue::Expr(expr));
    }

    // Check for tuple expression (e.g., (1, 2) for symmetric padding)
    if input.peek(Paren) {
        let content;
        parenthesized!(content in input);
        let exprs: syn::punctuated::Punctuated<Expr, Token![,]> =
            content.parse_terminated(Expr::parse, Token![,])?;
        let tuple = syn::ExprTuple {
            attrs: vec![],
            paren_token: syn::token::Paren::default(),
            elems: exprs,
        };
        return Ok(AttrValue::Expr(Expr::Tuple(tuple)));
    }

    // Check for literal
    if input.peek(syn::Lit) {
        let lit: syn::Lit = input.parse()?;
        return Ok(AttrValue::Lit(lit));
    }

    // Check for `self` keyword (start of path like `self.foo`)
    if input.peek(Token![self]) {
        let expr: Expr = input.parse()?;
        return Ok(AttrValue::Expr(expr));
    }

    // Try to parse as identifier, then check if it's a path (foo.bar) or pipe (foo | op())
    let ident: Ident = input.parse()?;

    // Check for pipe operator (color operations like `primary | darken(0.2)`)
    if input.peek(Token![|]) {
        return parse_color_with_ops(ident, input);
    }

    // Check if this is a path expression (ident.something)
    if input.peek(Token![.]) {
        // Build a path expression: start with the ident, then parse the rest
        let mut expr: Expr = syn::parse_quote!(#ident);
        while input.peek(Token![.]) {
            input.parse::<Token![.]>()?;
            let field: Ident = input.parse()?;
            expr = syn::parse_quote!(#expr.#field);
        }
        return Ok(AttrValue::Expr(expr));
    }

    Ok(AttrValue::Ident(ident))
}

/// Parse color operations: `| op(arg) | op(arg)`
///
/// Supports: lighten, darken, saturate, desaturate, hue_shift, alpha
fn parse_color_with_ops(base: Ident, input: ParseStream) -> syn::Result<AttrValue> {
    let base_str = base.to_string();

    // Start building: Color::var("base")
    let mut expr: Expr = syn::parse_quote!(tuidom::Color::var(#base_str));

    // Parse operations
    while input.peek(Token![|]) {
        input.parse::<Token![|]>()?;

        let op: Ident = input.parse()?;
        let op_str = op.to_string();

        // Parse argument in parentheses
        let content;
        parenthesized!(content in input);
        let arg: syn::Lit = content.parse()?;

        // Build method call
        expr = match op_str.as_str() {
            "lighten" | "darken" | "saturate" | "desaturate" | "alpha" | "hue_shift" => {
                syn::parse_quote!(#expr.#op(#arg))
            }
            _ => {
                return Err(syn::Error::new(
                    op.span(),
                    format!(
                        "Unknown color operation: '{}'. Valid: lighten, darken, saturate, desaturate, hue_shift, alpha",
                        op_str
                    ),
                ));
            }
        };
    }

    Ok(AttrValue::Expr(expr))
}

/// Parse handlers after attributes: `on_click: handler(args) on_change: other()`
fn parse_handlers(input: ParseStream) -> syn::Result<Vec<HandlerAttr>> {
    let mut handlers = Vec::new();

    // Handlers are: ident `:` ident `(` args `)`
    // They appear before children (braces) and are not in parentheses
    while input.peek(Ident) {
        // Peek ahead to see if this looks like a handler (ident: ident)
        // Not a handler if next is `(` (that would be attrs) or `{` (children)
        let fork = input.fork();
        let _first: Ident = fork.parse()?;

        if !fork.peek(Token![:]) {
            // Not a handler pattern, stop
            break;
        }

        // Check if this is on_* pattern (handler names start with on_)
        let event: Ident = input.parse()?;
        if !event.to_string().starts_with("on_") {
            return Err(syn::Error::new(
                event.span(),
                "Handler events must start with 'on_' (e.g., on_click, on_change)",
            ));
        }

        input.parse::<Token![:]>()?;
        let handler: Ident = input.parse()?;

        // Parse args in parentheses (optional)
        let args = if input.peek(syn::token::Paren) {
            let content;
            parenthesized!(content in input);
            parse_handler_args(&content)?
        } else {
            Vec::new()
        };

        handlers.push(HandlerAttr {
            event,
            handler,
            args,
        });
    }

    Ok(handlers)
}

/// Parse handler arguments: `arg1, arg2, cx`
fn parse_handler_args(input: ParseStream) -> syn::Result<Vec<HandlerArg>> {
    let mut args = Vec::new();

    while !input.is_empty() {
        let expr: Expr = input.parse()?;

        // Check if it's cx or gx (context reference)
        let arg = if let Expr::Path(ref p) = expr {
            if let Some(ident) = p.path.get_ident() {
                if ident == "cx" || ident == "gx" {
                    HandlerArg::Context(ident.clone())
                } else {
                    HandlerArg::Expr(expr)
                }
            } else {
                HandlerArg::Expr(expr)
            }
        } else {
            HandlerArg::Expr(expr)
        };

        args.push(arg);

        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        } else {
            break;
        }
    }

    Ok(args)
}

/// Parse children nodes
fn parse_children(input: ParseStream) -> syn::Result<Vec<ViewNode>> {
    let mut children = Vec::new();

    while !input.is_empty() {
        children.push(parse_view_node(input)?);
    }

    Ok(children)
}

/// Parse a for loop: `for pat in expr { children }`
fn parse_for(input: ParseStream) -> syn::Result<ForNode> {
    input.parse::<Token![for]>()?;
    let pat: Pat = Pat::parse_single(input)?;
    input.parse::<Token![in]>()?;

    // Parse the iterator expression (everything up to the brace)
    let iter = parse_expr_before_brace(input)?;

    let content;
    braced!(content in input);
    let body = parse_children(&content)?;

    Ok(ForNode { pat, iter, body })
}

/// Parse an if statement: `if cond { then } else { else }`
fn parse_if(input: ParseStream) -> syn::Result<IfNode> {
    input.parse::<Token![if]>()?;

    // Parse condition (everything up to the brace)
    let cond = parse_expr_before_brace(input)?;

    let content;
    braced!(content in input);
    let then_branch = parse_children(&content)?;

    // Check for else
    let else_branch = if input.peek(Token![else]) {
        input.parse::<Token![else]>()?;

        if input.peek(Token![if]) {
            // else if
            Some(ElseBranch::ElseIf(Box::new(parse_if(input)?)))
        } else {
            // else
            let content;
            braced!(content in input);
            Some(ElseBranch::Else(parse_children(&content)?))
        }
    } else {
        None
    };

    Ok(IfNode {
        cond,
        then_branch,
        else_branch,
    })
}

/// Parse a match expression: `match expr { arms }`
fn parse_match(input: ParseStream) -> syn::Result<MatchNode> {
    input.parse::<Token![match]>()?;

    // Parse the expression being matched
    let expr = parse_expr_before_brace(input)?;

    let content;
    braced!(content in input);
    let arms = parse_match_arms(&content)?;

    Ok(MatchNode { expr, arms })
}

/// Parse match arms
fn parse_match_arms(input: ParseStream) -> syn::Result<Vec<MatchArm>> {
    let mut arms = Vec::new();

    while !input.is_empty() {
        let pat: Pat = Pat::parse_multi(input)?;

        // Optional guard
        let guard = if input.peek(Token![if]) {
            input.parse::<Token![if]>()?;
            Some(parse_expr_before_arrow(input)?)
        } else {
            None
        };

        input.parse::<Token![=>]>()?;

        // Body can be either a single element or braced children
        let body = if input.peek(Brace) {
            let content;
            braced!(content in input);
            parse_children(&content)?
        } else {
            vec![parse_view_node(input)?]
        };

        arms.push(MatchArm { pat, guard, body });

        // Optional comma
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }
    }

    Ok(arms)
}

/// Parse an expression that ends before a brace
fn parse_expr_before_brace(input: ParseStream) -> syn::Result<Expr> {
    // Collect tokens until we hit a brace
    let mut tokens = Vec::new();

    while !input.is_empty() && !input.peek(Brace) {
        tokens.push(input.parse::<proc_macro2::TokenTree>()?);
    }

    if tokens.is_empty() {
        return Err(input.error("expected expression"));
    }

    let token_stream: proc_macro2::TokenStream = tokens.into_iter().collect();
    syn::parse2(token_stream)
}

/// Parse an expression that ends before a `=>`
fn parse_expr_before_arrow(input: ParseStream) -> syn::Result<Expr> {
    let mut tokens = Vec::new();

    while !input.is_empty() && !input.peek(Token![=>]) {
        tokens.push(input.parse::<proc_macro2::TokenTree>()?);
    }

    if tokens.is_empty() {
        return Err(input.error("expected expression"));
    }

    let token_stream: proc_macro2::TokenStream = tokens.into_iter().collect();
    syn::parse2(token_stream)
}
