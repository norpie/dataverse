use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Type, parse2};

/// Derive macro for the Request trait.
///
/// Requires a `#[response(Type)]` attribute to specify the response type.
pub fn expand(input: TokenStream) -> TokenStream {
    let input: DeriveInput = match parse2(input) {
        Ok(i) => i,
        Err(e) => return e.to_compile_error(),
    };

    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Find #[response(Type)] attribute
    let response_type = input.attrs.iter().find_map(|attr| {
        if attr.path().is_ident("response") {
            attr.parse_args::<Type>().ok()
        } else {
            None
        }
    });

    let Some(response_type) = response_type else {
        return syn::Error::new_spanned(
            &input,
            "#[derive(Request)] requires #[response(Type)] attribute",
        )
        .to_compile_error();
    };

    quote! {
        impl #impl_generics rafter::Request for #name #ty_generics #where_clause {
            type Response = #response_type;
        }
    }
}
