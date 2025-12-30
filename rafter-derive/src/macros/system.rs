//! The `#[system]` attribute macro for system handlers.
//!
//! Systems are like invisible apps - they have keybinds and handlers but no view.
//! System keybinds are checked before app keybinds (highest priority).

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Fields, parse2};

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    // #[system] takes no attributes for now
    if !attr.is_empty() {
        return syn::Error::new_spanned(attr, "#[system] does not accept attributes")
            .to_compile_error();
    }

    let input: DeriveInput = match parse2(item) {
        Ok(i) => i,
        Err(e) => return e.to_compile_error(),
    };

    let name = &input.ident;
    let vis = &input.vis;
    let generics = &input.generics;

    // Preserve non-system attributes
    let other_attrs: Vec<_> = input
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("system"))
        .collect();

    // Handle different struct types
    let (struct_def, clone_impl, default_impl) = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            Fields::Unit => {
                // Unit struct: struct MySystem;
                let struct_def = quote! {
                    #(#other_attrs)*
                    #vis struct #name #generics;
                };
                let clone_impl = quote! {
                    impl #generics Clone for #name #generics {
                        fn clone(&self) -> Self { Self }
                    }
                };
                let default_impl = quote! {
                    impl #generics Default for #name #generics {
                        fn default() -> Self { Self }
                    }
                };
                (struct_def, clone_impl, default_impl)
            }
            Fields::Named(fields) => {
                // Named fields struct
                let field_defs: Vec<_> = fields
                    .named
                    .iter()
                    .map(|f| {
                        let vis = &f.vis;
                        let ident = &f.ident;
                        let ty = &f.ty;
                        let attrs: Vec<_> = f.attrs.iter().collect();
                        quote! { #(#attrs)* #vis #ident: #ty }
                    })
                    .collect();

                let struct_def = quote! {
                    #(#other_attrs)*
                    #vis struct #name #generics {
                        #(#field_defs),*
                    }
                };

                let field_clones: Vec<_> = fields
                    .named
                    .iter()
                    .map(|f| {
                        let ident = &f.ident;
                        quote! { #ident: self.#ident.clone() }
                    })
                    .collect();

                let clone_impl = quote! {
                    impl #generics Clone for #name #generics {
                        fn clone(&self) -> Self {
                            Self { #(#field_clones),* }
                        }
                    }
                };

                let field_defaults: Vec<_> = fields
                    .named
                    .iter()
                    .map(|f| {
                        let ident = &f.ident;
                        quote! { #ident: Default::default() }
                    })
                    .collect();

                let default_impl = quote! {
                    impl #generics Default for #name #generics {
                        fn default() -> Self {
                            Self { #(#field_defaults),* }
                        }
                    }
                };

                (struct_def, clone_impl, default_impl)
            }
            Fields::Unnamed(_) => {
                return syn::Error::new_spanned(&input, "#[system] does not support tuple structs")
                    .to_compile_error();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "#[system] can only be applied to structs")
                .to_compile_error();
        }
    };

    // Generate inventory registration
    let name_str = name.to_string();
    let registration = quote! {
        inventory::submit! {
            rafter::system::SystemRegistration::new(
                #name_str,
                || Box::new(#name::default()) as Box<dyn rafter::system::AnySystem>
            )
        }
    };

    // Generate metadata module
    let metadata_name = format_ident!(
        "__rafter_system_metadata_{}",
        name.to_string().to_lowercase()
    );
    let metadata = quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub mod #metadata_name {
            use super::*;

            pub const NAME: &str = #name_str;
        }
    };

    quote! {
        #struct_def
        #clone_impl
        #default_impl
        #registration
        #metadata
    }
}
