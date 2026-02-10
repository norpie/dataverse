//! Implementation of the `parallel_load!` macro.
//!
//! This macro provides ergonomic syntax for parallel loading with typed results.

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{parse_macro_input, Expr, Ident, LitStr, Token};

/// A single task in the parallel_load! macro.
///
/// Format: `"Label" => future_expr`
struct TaskDef {
    label: LitStr,
    future: Expr,
}

impl Parse for TaskDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let label: LitStr = input.parse()?;
        input.parse::<Token![=>]>()?;
        let future: Expr = input.parse()?;
        Ok(TaskDef { label, future })
    }
}

/// Options for the parallel_load! macro.
#[derive(Default)]
struct Options {
    fail_fast: Option<Expr>,
}


/// Full parallel_load! macro input.
///
/// Format:
/// ```ignore
/// parallel_load!(gx, {
///     "Label 1" => future1,
///     "Label 2" => future2,
/// })
/// ```
///
/// Or with options:
/// ```ignore
/// parallel_load!(gx, fail_fast: false, {
///     "Label 1" => future1,
///     "Label 2" => future2,
/// })
/// ```
struct ParallelLoadInput {
    gx: Expr,
    options: Options,
    tasks: Vec<TaskDef>,
}

impl Parse for ParallelLoadInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Parse the global context expression
        let gx: Expr = input.parse()?;
        input.parse::<Token![,]>()?;

        // Parse optional options before the task block
        let mut options = Options::default();

        // Check if we have options (keyword followed by colon)
        while !input.peek(syn::token::Brace) {
            let ident: Ident = input.parse()?;
            input.parse::<Token![:]>()?;

            match ident.to_string().as_str() {
                "fail_fast" => {
                    options.fail_fast = Some(input.parse()?);
                }
                other => {
                    return Err(syn::Error::new(
                        ident.span(),
                        format!("unknown option: {}", other),
                    ));
                }
            }

            input.parse::<Token![,]>()?;
        }

        // Parse the task block: { ... }
        let content;
        syn::braced!(content in input);

        // Parse comma-separated tasks
        let tasks: Punctuated<TaskDef, Token![,]> =
            content.parse_terminated(TaskDef::parse, Token![,])?;

        Ok(ParallelLoadInput {
            gx,
            options,
            tasks: tasks.into_iter().collect(),
        })
    }
}

/// Expand the parallel_load! macro.
pub fn expand(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ParallelLoadInput);
    let expanded = generate_parallel_load(input);
    TokenStream::from(expanded)
}

fn generate_parallel_load(input: ParallelLoadInput) -> TokenStream2 {
    let gx = &input.gx;
    let task_count = input.tasks.len();

    if task_count == 0 {
        return quote! {
            compile_error!("parallel_load! requires at least one task")
        };
    }

    // Generate channel pairs for each task
    let channel_defs: Vec<TokenStream2> = (0..task_count)
        .map(|i| {
            let tx = Ident::new(&format!("__tx_{}", i), Span::call_site());
            let rx = Ident::new(&format!("__rx_{}", i), Span::call_site());
            quote! {
                let (#tx, mut #rx) = tokio::sync::oneshot::channel();
            }
        })
        .collect();

    // Generate future bindings
    let future_bindings: Vec<TokenStream2> = input
        .tasks
        .iter()
        .enumerate()
        .map(|(i, task)| {
            let fut = Ident::new(&format!("__fut_{}", i), Span::call_site());
            let future = &task.future;
            quote! {
                let #fut = #future;
            }
        })
        .collect();

    // Generate task wrappers
    let task_wrappers: Vec<TokenStream2> = input
        .tasks
        .iter()
        .enumerate()
        .map(|(i, task)| {
            let label = &task.label;
            let tx = Ident::new(&format!("__tx_{}", i), Span::call_site());
            let fut = Ident::new(&format!("__fut_{}", i), Span::call_site());
            quote! {
                crate::modals::ParallelTask::new(#label, {
                    let tx = #tx;
                    async move {
                        let result = #fut.await;
                        let success = crate::modals::Checkable::is_success(&result);
                        let _ = tx.send(result);
                        success
                    }
                })
            }
        })
        .collect();

    // Generate result collection
    let result_exprs: Vec<TokenStream2> = (0..task_count)
        .map(|i| {
            let rx = Ident::new(&format!("__rx_{}", i), Span::call_site());
            quote! {
                match #rx.try_recv() {
                    Ok(value) => Ok(value),
                    Err(_) => {
                        let label = __failed_task_label.lock().unwrap();
                        Err(match label.as_deref() {
                            Some(failed) => crate::modals::ParallelLoadError::Cancelled {
                                failed_task: failed.to_string(),
                            },
                            None => crate::modals::ParallelLoadError::Dropped,
                        })
                    }
                }
            }
        })
        .collect();

    // Generate the fail_fast value
    let fail_fast_value = match &input.options.fail_fast {
        Some(expr) => quote! { #expr },
        None => quote! { true },
    };

    // Generate task info initializers
    let task_info_inits: Vec<TokenStream2> = input
        .tasks
        .iter()
        .map(|task| {
            let label = &task.label;
            quote! {
                crate::modals::TaskInfo {
                    label: #label.to_string(),
                    status: crate::modals::TaskStatus::Pending,
                }
            }
        })
        .collect();

    quote! {
        {
            // Create channels for each task
            #(#channel_defs)*

            // Bind futures (to evaluate them before moving into closures)
            #(#future_bindings)*

            // Create task wrappers
            let __tasks = vec![
                #(#task_wrappers),*
            ];

            // Build task infos for display
            let __task_infos = vec![
                #(#task_info_inits),*
            ];

            // Shared handle for tracking which task caused a fail-fast cancellation
            let __failed_task_label = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));

            // Create and run the modal
            let __modal = crate::modals::ParallelLoadingModal::new(
                std::sync::Arc::new(std::sync::Mutex::new(__tasks)),
                #fail_fast_value,
                std::sync::Arc::clone(&__failed_task_label),
                __task_infos,
            );

            #gx.modal(__modal).await;

            // Collect results
            (#(#result_exprs),*)
        }
    }
}

#[cfg(test)]
mod tests {
    // Macro expansion tests would go here, but they're difficult to test
    // without the full compilation context. Integration tests are better.
}
