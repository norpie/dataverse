//! AST types for the page! macro.

use syn::{Expr, Ident};

/// Root node of the page! macro - contains a single element or expression.
#[derive(Debug)]
pub struct Page {
    pub root: ViewNode,
}

/// A node in the view tree.
#[derive(Debug)]
pub enum ViewNode {
    /// An element (column, row, button, etc.)
    Element(ElementNode),
    /// A for loop
    For(ForNode),
    /// An if/else conditional
    If(IfNode),
    /// A match expression
    Match(MatchNode),
    /// A raw Rust expression (e.g., `{ some_element }`)
    Expr(Expr),
}

/// An element node (e.g., `column (padding: 1) style (bg: primary) { ... }`)
#[derive(Debug)]
pub struct ElementNode {
    /// Element name (column, row, button, etc.)
    pub name: Ident,
    /// Layout attributes in parentheses (padding: 1, gap: 2)
    pub layout_attrs: Vec<Attr>,
    /// Style attributes after `style` keyword (bg: primary, bold: true)
    pub style_attrs: Vec<Attr>,
    /// Transition attributes after `transition` keyword (bg: 200ms ease_out)
    pub transition_attrs: Vec<TransitionAttr>,
    /// Inline handlers (on_click: handler(args))
    pub handlers: Vec<HandlerAttr>,
    /// Child nodes in braces
    pub children: Vec<ViewNode>,
}

/// An attribute (key: value pair)
#[derive(Debug)]
pub struct Attr {
    pub name: Ident,
    pub value: AttrValue,
}

/// A transition attribute (e.g., `bg: 200ms ease_out`)
#[derive(Debug)]
pub struct TransitionAttr {
    /// Property name (bg, fg, all, width, height, etc.)
    pub property: Ident,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Easing function (linear, ease_in, ease_out, ease_in_out)
    pub easing: Option<Ident>,
}

/// Attribute value types
#[derive(Debug)]
pub enum AttrValue {
    /// Identifier (e.g., `primary`, `center`)
    Ident(Ident),
    /// Literal (e.g., `1`, `"hello"`)
    Lit(syn::Lit),
    /// Expression in braces (e.g., `{some_var}`)
    Expr(Expr),
}

/// An inline handler attribute (e.g., `on_click: handler(arg1, cx)`)
#[derive(Debug)]
pub struct HandlerAttr {
    /// Event name (on_click, on_change, etc.)
    pub event: Ident,
    /// Handler method name
    pub handler: Ident,
    /// Arguments to capture/pass
    pub args: Vec<HandlerArg>,
}

/// An argument to a handler
#[derive(Debug)]
pub enum HandlerArg {
    /// A regular expression to capture (cloned at render time)
    Expr(Expr),
    /// A context reference (cx or gx) - passed at event time, not cloned
    Context(Ident),
}

/// A for loop node (e.g., `for item in items { ... }`)
#[derive(Debug)]
pub struct ForNode {
    /// Loop variable pattern
    pub pat: syn::Pat,
    /// Iterator expression
    pub iter: Expr,
    /// Loop body
    pub body: Vec<ViewNode>,
}

/// An if/else node
#[derive(Debug)]
pub struct IfNode {
    /// Condition expression
    pub cond: Expr,
    /// Then branch
    pub then_branch: Vec<ViewNode>,
    /// Else branch (if present)
    pub else_branch: Option<ElseBranch>,
}

/// Else branch of an if node
#[derive(Debug)]
pub enum ElseBranch {
    /// else { ... }
    Else(Vec<ViewNode>),
    /// else if ... { ... }
    ElseIf(Box<IfNode>),
}

/// A match expression node
#[derive(Debug)]
pub struct MatchNode {
    /// Expression being matched
    pub expr: Expr,
    /// Match arms
    pub arms: Vec<MatchArm>,
}

/// A match arm
#[derive(Debug)]
pub struct MatchArm {
    /// Pattern to match
    pub pat: syn::Pat,
    /// Optional guard (if condition)
    pub guard: Option<Expr>,
    /// Body of the arm
    pub body: Vec<ViewNode>,
}
