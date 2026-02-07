//! Shared dependency detection for `#[derived]` and `#[watch]`.
//!
//! Analyzes function bodies to find `State<T>` field accesses by looking for
//! patterns like `self.{field}.get()`, `self.{field}.with_ref()`, etc.

use proc_macro2::TokenStream;
use syn::{Expr, ExprField, ExprMethodCall, ImplItemFn};

/// Extract dependencies from function body by finding State access patterns.
///
/// Returns a sorted, deduplicated list of field names that are accessed
/// through State methods (get, with_ref, is_dirty, generation).
pub fn find_dependencies(func: &ImplItemFn) -> Vec<String> {
    let mut deps = Vec::new();
    visit_block(&func.block, &mut deps);
    deps.sort();
    deps.dedup();
    deps
}

/// Recursively visit expressions in a block to find dependencies.
fn visit_block(block: &syn::Block, deps: &mut Vec<String>) {
    for stmt in &block.stmts {
        match stmt {
            syn::Stmt::Expr(expr, _) => visit_expr(expr, deps),
            syn::Stmt::Local(local) => {
                if let Some(init) = &local.init {
                    visit_expr(&init.expr, deps);
                }
            }
            _ => {}
        }
    }
}

/// Visit an expression to find State field accesses.
fn visit_expr(expr: &Expr, deps: &mut Vec<String>) {
    match expr {
        // Method call: self.field.get()
        Expr::MethodCall(ExprMethodCall {
            receiver, method, ..
        }) => {
            // Check if this is a State method (get, with_ref, etc.)
            let method_name = method.to_string();
            if matches!(
                method_name.as_str(),
                "get" | "with_ref" | "is_dirty" | "generation"
            ) {
                // Check if receiver is self.field
                if let Expr::Field(ExprField { base, member, .. }) = &**receiver
                    && is_self_expr(base)
                        && let syn::Member::Named(field_name) = member {
                            deps.push(field_name.to_string());
                        }
            }
            // Recurse into receiver and args
            visit_expr(receiver, deps);
        }
        // Field access: self.field (less common for State)
        Expr::Field(ExprField { base, .. }) => {
            visit_expr(base, deps);
        }
        // Binary operations: a + b
        Expr::Binary(binary) => {
            visit_expr(&binary.left, deps);
            visit_expr(&binary.right, deps);
        }
        // Unary operations: -a
        Expr::Unary(unary) => {
            visit_expr(&unary.expr, deps);
        }
        // Function calls
        Expr::Call(call) => {
            visit_expr(&call.func, deps);
            for arg in &call.args {
                visit_expr(arg, deps);
            }
        }
        // If expressions
        Expr::If(if_expr) => {
            visit_expr(&if_expr.cond, deps);
            visit_block(&if_expr.then_branch, deps);
            if let Some((_, else_branch)) = &if_expr.else_branch {
                visit_expr(else_branch, deps);
            }
        }
        // Match expressions
        Expr::Match(match_expr) => {
            visit_expr(&match_expr.expr, deps);
            for arm in &match_expr.arms {
                visit_expr(&arm.body, deps);
            }
        }
        // Block expressions
        Expr::Block(block_expr) => {
            visit_block(&block_expr.block, deps);
        }
        // Array/tuple literals
        Expr::Array(array) => {
            for elem in &array.elems {
                visit_expr(elem, deps);
            }
        }
        Expr::Tuple(tuple) => {
            for elem in &tuple.elems {
                visit_expr(elem, deps);
            }
        }
        // Closures
        Expr::Closure(closure) => {
            visit_expr(&closure.body, deps);
        }
        // Reference/dereference
        Expr::Reference(reference) => {
            visit_expr(&reference.expr, deps);
        }
        // Macro calls - try to parse their content
        Expr::Macro(mac) => {
            // Can't easily parse macro content, but format! and similar
            // might contain our patterns
            if let Ok(tokens) = mac.mac.parse_body::<TokenStream>() {
                // Try to find patterns in the token stream
                let token_str = tokens.to_string();
                // This is a simple heuristic - look for "self . field"
                // Not perfect but catches common cases like format!
                let parts: Vec<&str> = token_str.split_whitespace().collect();
                for window in parts.windows(3) {
                    if window[0] == "self" && window[1] == "." {
                        let field =
                            window[2].trim_end_matches(&['.', '(', ')', ',', ';'] as &[char]);
                        if !field.is_empty() {
                            deps.push(field.to_string());
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// Check if an expression is `self`.
fn is_self_expr(expr: &Expr) -> bool {
    matches!(expr, Expr::Path(path) if path.path.is_ident("self"))
}
