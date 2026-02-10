//! The `#[derived]` attribute macro for defining derived state.
//!
//! Transforms a method into a memoized computed value that automatically
//! tracks dependencies and recomputes when they change.
//!
//! Dependencies are detected by analyzing the function body for calls to
//! `self.{field}.get()`, `self.{field}.with_ref()`, etc.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{ImplItemFn, parse2};

use super::dep_detection::find_dependencies;

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
