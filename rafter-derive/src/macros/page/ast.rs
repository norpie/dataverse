//! AST types for the page macro.

use syn::{Expr, Ident};

/// A node in the page tree
pub enum ViewNode {
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
pub struct ElementNode {
    /// Element name (column, row, text, etc.)
    pub name: Ident,
    /// Attributes in parentheses
    pub attrs: Vec<Attr>,
    /// Children in braces
    pub children: Vec<ViewNode>,
}

/// An attribute like padding: 1 or bold
pub struct Attr {
    pub name: Ident,
    pub value: Option<AttrValue>,
}

/// Attribute value
pub enum AttrValue {
    /// Literal integer
    Int(i64),
    /// Literal float
    Float(f64),
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
pub enum TextNode {
    /// String literal
    Literal(String),
    /// Expression that produces a string
    Expr(Expr),
}

/// Control flow nodes
pub enum ControlFlowNode {
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
pub struct MatchArm {
    pub pattern: syn::Pat,
    pub guard: Option<Expr>,
    pub body: Vec<ViewNode>,
}
