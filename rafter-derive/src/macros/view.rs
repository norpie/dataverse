use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    Expr, Ident, LitStr, Token, braced, parenthesized,
    parse::{Parse, ParseStream},
    parse2,
    token::Brace,
};

/// A node in the view tree
enum ViewNode {
    /// Element: column (attrs) { children }
    Element(ElementNode),
    /// Text content: "string" or { expr }
    Text(TextNode),
    /// Control flow: if/for/match
    ControlFlow(Box<ControlFlowNode>),
    /// Expression in braces: { some_expr }
    Expr(Expr),
}

/// An element node like column, row, text, etc.
struct ElementNode {
    /// Element name (column, row, text, etc.)
    name: Ident,
    /// Attributes in parentheses
    attrs: Vec<Attr>,
    /// Children in braces
    children: Vec<ViewNode>,
}

/// An attribute like padding: 1 or bold
struct Attr {
    name: Ident,
    value: Option<AttrValue>,
}

/// Attribute value
enum AttrValue {
    /// Literal integer
    Int(i64),
    /// Literal string
    Str(String),
    /// Boolean
    Bool(bool),
    /// Identifier (like a color name)
    Ident(Ident),
    /// Expression
    Expr(Expr),
}

/// Text content
#[allow(dead_code)]
enum TextNode {
    /// String literal
    Literal(String),
    /// Expression that produces a string
    Expr(Expr),
}

/// Control flow nodes
enum ControlFlowNode {
    /// if condition { ... } else { ... }
    If {
        condition: Expr,
        then_branch: Vec<ViewNode>,
        else_branch: Option<Vec<ViewNode>>,
    },
    /// if let pattern = expr { ... }
    IfLet {
        pattern: syn::Pat,
        expr: Expr,
        then_branch: Vec<ViewNode>,
        else_branch: Option<Vec<ViewNode>>,
    },
    /// for item in iter { ... }
    For {
        pattern: syn::Pat,
        iter: Expr,
        body: Vec<ViewNode>,
    },
    /// match expr { ... }
    Match { expr: Expr, arms: Vec<MatchArm> },
}

/// A match arm
struct MatchArm {
    pattern: syn::Pat,
    guard: Option<Expr>,
    body: Vec<ViewNode>,
}

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

fn parse_element(input: ParseStream) -> syn::Result<ElementNode> {
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

fn parse_attrs(input: ParseStream) -> syn::Result<Vec<Attr>> {
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
                let ident: Ident = input.parse()?;
                // Check for true/false identifiers
                if ident == "true" {
                    Some(AttrValue::Bool(true))
                } else if ident == "false" {
                    Some(AttrValue::Bool(false))
                } else {
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

fn parse_children(input: ParseStream) -> syn::Result<Vec<ViewNode>> {
    let mut children = Vec::new();

    while !input.is_empty() {
        children.push(input.parse()?);
    }

    Ok(children)
}

fn parse_if(input: ParseStream) -> syn::Result<ControlFlowNode> {
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

fn parse_for(input: ParseStream) -> syn::Result<ControlFlowNode> {
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

fn parse_match(input: ParseStream) -> syn::Result<ControlFlowNode> {
    input.parse::<Token![match]>()?;
    let expr: Expr = Expr::parse_without_eager_brace(input)?;

    let content;
    braced!(content in input);

    let mut arms = Vec::new();
    while !content.is_empty() {
        let pattern: syn::Pat = syn::Pat::parse_multi(input)?;

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

/// Generate code for a view node
fn generate_node(node: &ViewNode) -> TokenStream {
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

fn generate_element(elem: &ElementNode) -> TokenStream {
    let name_str = elem.name.to_string();

    match name_str.as_str() {
        "column" => generate_container(elem, quote! { rafter::node::Node::Column }),
        "row" => generate_container(elem, quote! { rafter::node::Node::Row }),
        "stack" => generate_container(elem, quote! { rafter::node::Node::Stack }),
        "text" => generate_text_element(elem),
        "input" => generate_input_element(elem),
        "button" => generate_button_element(elem),
        _ => {
            // Unknown element - treat as a component function call
            let name = &elem.name;
            let args = generate_component_args(elem);
            quote! { #name(#args) }
        }
    }
}

fn generate_container(elem: &ElementNode, variant: TokenStream) -> TokenStream {
    let children: Vec<_> = elem.children.iter().map(generate_node).collect();
    let (style, layout) = generate_style_and_layout(&elem.attrs);

    quote! {
        #variant {
            children: vec![#(#children),*],
            style: #style,
            layout: #layout,
        }
    }
}

fn generate_text_element(elem: &ElementNode) -> TokenStream {
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

fn generate_input_element(elem: &ElementNode) -> TokenStream {
    let style = generate_style(&elem.attrs);
    let mut value = quote! { String::new() };
    let mut placeholder = quote! { String::new() };
    let mut id = quote! { None };
    let mut focused = quote! { false };
    let mut on_change = quote! { None };
    let mut on_submit = quote! { None };

    for attr in &elem.attrs {
        let name_str = attr.name.to_string();
        match name_str.as_str() {
            "value" => {
                if let Some(AttrValue::Str(s)) = &attr.value {
                    value = quote! { #s.to_string() };
                } else if let Some(AttrValue::Expr(e)) = &attr.value {
                    value = quote! { (#e).to_string() };
                } else if let Some(AttrValue::Ident(i)) = &attr.value {
                    value = quote! { #i.to_string() };
                }
            }
            "placeholder" => {
                if let Some(AttrValue::Str(s)) = &attr.value {
                    placeholder = quote! { #s.to_string() };
                } else if let Some(AttrValue::Expr(e)) = &attr.value {
                    placeholder = quote! { #e.to_string() };
                }
            }
            "id" => {
                if let Some(AttrValue::Str(s)) = &attr.value {
                    id = quote! { Some(#s.to_string()) };
                } else if let Some(AttrValue::Expr(e)) = &attr.value {
                    id = quote! { Some(#e.to_string()) };
                }
            }
            "focused" => {
                if let Some(AttrValue::Bool(b)) = &attr.value {
                    focused = quote! { #b };
                } else if let Some(AttrValue::Expr(e)) = &attr.value {
                    focused = quote! { #e };
                } else if let Some(AttrValue::Ident(i)) = &attr.value {
                    // Variable reference like step_focused
                    focused = quote! { #i };
                } else {
                    focused = quote! { true };
                }
            }
            "on_change" => {
                if let Some(AttrValue::Ident(i)) = &attr.value {
                    let handler_name = i.to_string();
                    on_change =
                        quote! { Some(rafter::keybinds::HandlerId(#handler_name.to_string())) };
                }
            }
            "on_submit" => {
                if let Some(AttrValue::Ident(i)) = &attr.value {
                    let handler_name = i.to_string();
                    on_submit =
                        quote! { Some(rafter::keybinds::HandlerId(#handler_name.to_string())) };
                }
            }
            _ => {}
        }
    }

    quote! {
        rafter::node::Node::Input {
            value: #value,
            placeholder: #placeholder,
            on_change: #on_change,
            on_submit: #on_submit,
            id: #id,
            style: #style,
            focused: #focused,
        }
    }
}

fn generate_button_element(elem: &ElementNode) -> TokenStream {
    let style = generate_style(&elem.attrs);
    let mut label = quote! { String::new() };
    let mut id = quote! { None };
    let mut focused = quote! { false };
    let mut on_click = quote! { None };

    for attr in &elem.attrs {
        let name_str = attr.name.to_string();
        match name_str.as_str() {
            "label" => {
                if let Some(AttrValue::Str(s)) = &attr.value {
                    label = quote! { #s.to_string() };
                } else if let Some(AttrValue::Expr(e)) = &attr.value {
                    label = quote! { #e.to_string() };
                }
            }
            "id" => {
                if let Some(AttrValue::Str(s)) = &attr.value {
                    id = quote! { Some(#s.to_string()) };
                } else if let Some(AttrValue::Expr(e)) = &attr.value {
                    id = quote! { Some(#e.to_string()) };
                }
            }
            "focused" => {
                if let Some(AttrValue::Bool(b)) = &attr.value {
                    focused = quote! { #b };
                } else if let Some(AttrValue::Expr(e)) = &attr.value {
                    focused = quote! { #e };
                } else if let Some(AttrValue::Ident(i)) = &attr.value {
                    // Variable reference like inc_focused
                    focused = quote! { #i };
                } else {
                    focused = quote! { true };
                }
            }
            "on_click" => {
                if let Some(AttrValue::Ident(i)) = &attr.value {
                    let handler_name = i.to_string();
                    on_click =
                        quote! { Some(rafter::keybinds::HandlerId(#handler_name.to_string())) };
                }
            }
            _ => {}
        }
    }

    quote! {
        rafter::node::Node::Button {
            label: #label,
            on_click: #on_click,
            id: #id,
            style: #style,
            focused: #focused,
        }
    }
}

fn generate_style(attrs: &[Attr]) -> TokenStream {
    let mut bold = quote! { false };
    let mut italic = quote! { false };
    let mut underline = quote! { false };
    let mut dim = quote! { false };
    let mut fg = quote! { None };
    let mut bg = quote! { None };

    for attr in attrs {
        let name_str = attr.name.to_string();
        match name_str.as_str() {
            "bold" => {
                if let Some(AttrValue::Bool(v)) = &attr.value {
                    bold = quote! { #v };
                } else {
                    bold = quote! { true };
                }
            }
            "italic" => {
                if let Some(AttrValue::Bool(v)) = &attr.value {
                    italic = quote! { #v };
                } else {
                    italic = quote! { true };
                }
            }
            "underline" => {
                if let Some(AttrValue::Bool(v)) = &attr.value {
                    underline = quote! { #v };
                } else {
                    underline = quote! { true };
                }
            }
            "dim" => {
                if let Some(AttrValue::Bool(v)) = &attr.value {
                    dim = quote! { #v };
                } else {
                    dim = quote! { true };
                }
            }
            "color" | "fg" => {
                fg = generate_color_value(&attr.value);
            }
            "bg" | "background" => {
                bg = generate_color_value(&attr.value);
            }
            _ => {}
        }
    }

    quote! {
        rafter::style::Style {
            fg: #fg,
            bg: #bg,
            bold: #bold,
            italic: #italic,
            underline: #underline,
            dim: #dim,
        }
    }
}

fn generate_color_value(value: &Option<AttrValue>) -> TokenStream {
    match value {
        Some(AttrValue::Ident(ident)) => {
            // Color name like "primary", "error", etc.
            // For now, just use the identifier as-is (theme lookup)
            let name_str = ident.to_string();
            quote! { Some(rafter::color::Color::Named(#name_str.to_string())) }
        }
        Some(AttrValue::Str(s)) => {
            // Hex color or color name
            quote! { Some(rafter::color::Color::from_hex(#s).unwrap_or_default()) }
        }
        Some(AttrValue::Expr(e)) => {
            quote! { Some(#e) }
        }
        _ => quote! { None },
    }
}

fn generate_style_and_layout(attrs: &[Attr]) -> (TokenStream, TokenStream) {
    let style = generate_style(attrs);
    let layout = generate_layout(attrs);
    (style, layout)
}

fn generate_layout(attrs: &[Attr]) -> TokenStream {
    let mut padding = quote! { 0 };
    let mut gap = quote! { 0 };
    let mut justify = quote! { rafter::node::Justify::Start };
    let mut align = quote! { rafter::node::Align::Stretch };
    let mut border = quote! { rafter::node::Border::None };

    for attr in attrs {
        let name_str = attr.name.to_string();
        match name_str.as_str() {
            "padding" => {
                if let Some(AttrValue::Int(v)) = &attr.value {
                    let v = *v as u16;
                    padding = quote! { #v };
                }
            }
            "gap" => {
                if let Some(AttrValue::Int(v)) = &attr.value {
                    let v = *v as u16;
                    gap = quote! { #v };
                }
            }
            "justify" | "align_items" => {
                if let Some(AttrValue::Ident(ident)) = &attr.value {
                    justify = match ident.to_string().as_str() {
                        "start" => quote! { rafter::node::Justify::Start },
                        "center" => quote! { rafter::node::Justify::Center },
                        "end" => quote! { rafter::node::Justify::End },
                        "space_between" => quote! { rafter::node::Justify::SpaceBetween },
                        "space_around" => quote! { rafter::node::Justify::SpaceAround },
                        _ => quote! { rafter::node::Justify::Start },
                    };
                }
            }
            "align" | "align_content" => {
                if let Some(AttrValue::Ident(ident)) = &attr.value {
                    align = match ident.to_string().as_str() {
                        "start" => quote! { rafter::node::Align::Start },
                        "center" => quote! { rafter::node::Align::Center },
                        "end" => quote! { rafter::node::Align::End },
                        "stretch" => quote! { rafter::node::Align::Stretch },
                        _ => quote! { rafter::node::Align::Stretch },
                    };
                }
            }
            "border" => {
                if let Some(AttrValue::Ident(ident)) = &attr.value {
                    border = match ident.to_string().as_str() {
                        "none" => quote! { rafter::node::Border::None },
                        "single" => quote! { rafter::node::Border::Single },
                        "double" => quote! { rafter::node::Border::Double },
                        "rounded" => quote! { rafter::node::Border::Rounded },
                        "thick" => quote! { rafter::node::Border::Thick },
                        _ => quote! { rafter::node::Border::None },
                    };
                }
            }
            _ => {}
        }
    }

    quote! {
        rafter::node::Layout {
            padding: #padding,
            gap: #gap,
            justify: #justify,
            align: #align,
            border: #border,
            ..Default::default()
        }
    }
}

fn generate_text(text: &TextNode) -> TokenStream {
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

fn generate_component_args(elem: &ElementNode) -> TokenStream {
    // For component calls, convert attrs to function arguments
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

/// The main entry point for the view macro
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
        }
    }
}
