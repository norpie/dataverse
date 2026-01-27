//! The `#[derived]` attribute macro for defining derived state.
//!
//! Transforms a method into a memoized computed value that automatically
//! tracks dependencies and recomputes when they change.
//!
//! Dependencies are detected by analyzing the function body for calls to
//! `self.{field}.get()`, `self.{field}.with_ref()`, etc.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Expr, ExprField, ExprMethodCall, ImplItemFn, parse2};

/// Extract dependencies from function body by finding State access patterns.
fn find_dependencies(func: &ImplItemFn) -> Vec<String> {
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
                if let Expr::Field(ExprField { base, member, .. }) = &**receiver {
                    if is_self_expr(base) {
                        if let syn::Member::Named(field_name) = member {
                            deps.push(field_name.to_string());
                        }
                    }
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

pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let func: ImplItemFn = match parse2(item) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error(),
    };

    // Extract function metadata
    let fn_name = &func.sig.ident;
    let fn_vis = &func.vis;
    let fn_return = &func.sig.output;
    let fn_body = &func.block;

    // Cache key is the function name as a string literal
    let cache_key = fn_name.to_string();

    // Find dependencies
    let deps = find_dependencies(&func);

    if deps.is_empty() {
        return syn::Error::new_spanned(
            func.sig.fn_token,
            "#[derived] function has no dependencies - it won't recompute",
        )
        .to_compile_error();
    }

    // Generate code to read generation counters
    let dep_idents: Vec<_> = deps.iter().map(|d| quote::format_ident!("{}", d)).collect();
    let gen_reads: Vec<_> = dep_idents
        .iter()
        .map(|d| quote! { let #d = self.#d.generation(); })
        .collect();

    // Generate tuple type for cache entry: (gen1, gen2, ..., result_type)
    let return_type = match fn_return {
        syn::ReturnType::Type(_, ty) => ty,
        syn::ReturnType::Default => {
            return syn::Error::new_spanned(
                &func.sig,
                "#[derived] function must have an explicit return type",
            )
            .to_compile_error();
        }
    };

    let gen_types = vec![quote! { u64 }; deps.len()];
    let cache_tuple_types = quote! { (#(#gen_types),*, #return_type) };

    // Generate cache check: if all generations match, return cached value
    let cache_check_conditions: Vec<_> = dep_idents
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let idx = syn::Index::from(i);
            quote! { entry.#idx == #d }
        })
        .collect();

    let result_idx = syn::Index::from(deps.len());

    // Generate expanded function
    let expanded = quote! {
        #fn_vis fn #fn_name(&self) #fn_return {
            const CACHE_KEY: &'static str = #cache_key;

            // Read current generations of all dependencies
            #(#gen_reads)*

            // Try to read from cache
            {
                let cache = self.__derived_cache.read().unwrap();
                if let Some(boxed) = cache.get(CACHE_KEY) {
                    if let Some(entry) = boxed.downcast_ref::<#cache_tuple_types>() {
                        // Check if all generations match
                        if #(#cache_check_conditions)&&* {
                            return entry.#result_idx.clone();
                        }
                    }
                }
            }

            // Cache miss or stale - recompute
            let result = (|| #fn_body)();

            // Update cache
            {
                let mut cache = self.__derived_cache.write().unwrap();
                cache.insert(
                    CACHE_KEY,
                    Box::new((#(#dep_idents),*, result.clone()))
                );
            }

            result
        }
    };

    expanded
}
