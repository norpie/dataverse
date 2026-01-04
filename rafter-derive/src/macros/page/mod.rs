//! The page! macro for declarative UI definitions.
//!
//! Outputs `tuidom::Element` using a builder pattern.

pub mod ast;
mod generate;
pub mod parse;

use proc_macro2::TokenStream;
use syn::parse2;

use ast::Page;

/// Expand the page! macro
pub fn expand(input: TokenStream) -> TokenStream {
    let page: Page = match parse2(input) {
        Ok(p) => p,
        Err(e) => return e.to_compile_error(),
    };

    generate::generate(&page)
}
