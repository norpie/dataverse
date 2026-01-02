use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse2};

/// Derive macro for the Event trait.
///
/// Generates a simple implementation of `rafter::Event` for the type.
/// The type must also derive `Clone` separately.
pub fn expand(input: TokenStream) -> TokenStream {
    let input: DeriveInput = match parse2(input) {
        Ok(i) => i,
        Err(e) => return e.to_compile_error(),
    };

    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    quote! {
        impl #impl_generics rafter::Event for #name #ty_generics #where_clause {}
    }
}
