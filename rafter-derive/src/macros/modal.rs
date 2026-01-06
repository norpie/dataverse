//! The `#[modal]` attribute macro for defining modal structs.
//!
//! Similar to #[app] but simpler - no registration, no wakeup.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Field, Fields, FieldsNamed, Ident, parse2};

use super::fields::{has_state_skip, has_widget_attribute, is_resource_type};

/// Transform a field, wrapping in State<T> if needed.
fn transform_field(field: &Field) -> TokenStream {
    let vis = &field.vis;
    let ident = &field.ident;
    let ty = &field.ty;

    let other_attrs: Vec<_> = field
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("state") && !a.path().is_ident("widget"))
        .collect();

    let should_wrap = !has_state_skip(&field.attrs)
        && !is_resource_type(ty)
        && !has_widget_attribute(&field.attrs);

    if should_wrap {
        quote! {
            #(#other_attrs)*
            #vis #ident: rafter::State<#ty>
        }
    } else {
        quote! {
            #(#other_attrs)*
            #vis #ident: #ty
        }
    }
}

/// Generate Default impl.
fn generate_default_impl(name: &Ident, fields: Option<&FieldsNamed>) -> TokenStream {
    let Some(fields) = fields else {
        return quote! {
            impl Default for #name {
                fn default() -> Self { Self }
            }
        };
    };

    let field_defaults: Vec<_> = fields
        .named
        .iter()
        .map(|f| {
            let ident = &f.ident;
            let ty = &f.ty;

            let should_wrap = !has_state_skip(&f.attrs)
                && !is_resource_type(ty)
                && !has_widget_attribute(&f.attrs);

            if is_resource_type(ty) {
                quote! { #ident: rafter::Resource::new() }
            } else if should_wrap {
                quote! { #ident: rafter::State::new(Default::default()) }
            } else {
                quote! { #ident: Default::default() }
            }
        })
        .collect();

    quote! {
        impl Default for #name {
            fn default() -> Self {
                Self {
                    #(#field_defaults),*
                }
            }
        }
    }
}

/// Generate Clone impl.
fn generate_clone_impl(name: &Ident, fields: Option<&FieldsNamed>) -> TokenStream {
    let Some(fields) = fields else {
        return quote! {
            impl Clone for #name {
                fn clone(&self) -> Self { Self }
            }
        };
    };

    let field_clones: Vec<_> = fields
        .named
        .iter()
        .map(|f| {
            let ident = &f.ident;
            quote! { #ident: self.#ident.clone() }
        })
        .collect();

    quote! {
        impl Clone for #name {
            fn clone(&self) -> Self {
                Self {
                    #(#field_clones),*
                }
            }
        }
    }
}

/// Generate metadata module for #[modal_impl].
fn generate_metadata(name: &Ident, fields: Option<&FieldsNamed>) -> TokenStream {
    let metadata_name = format_ident!(
        "__rafter_modal_metadata_{}",
        name.to_string().to_lowercase()
    );

    let dirty_fields: Vec<_> = fields
        .map(|f| {
            f.named
                .iter()
                .filter(|f| !has_state_skip(&f.attrs))
                .filter_map(|f| f.ident.as_ref())
                .collect()
        })
        .unwrap_or_default();

    quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub mod #metadata_name {
            use super::*;

            pub fn is_dirty(modal: &#name) -> bool {
                false #(|| modal.#dirty_fields.is_dirty())*
            }

            pub fn clear_dirty(modal: &#name) {
                #(modal.#dirty_fields.clear_dirty();)*
            }
        }
    }
}

pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input: DeriveInput = match parse2(item) {
        Ok(i) => i,
        Err(e) => return e.to_compile_error(),
    };

    let name = &input.ident;
    let vis = &input.vis;

    let other_attrs: Vec<_> = input
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("modal"))
        .collect();

    let fields = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            Fields::Named(f) => Some(f),
            Fields::Unit => None,
            _ => {
                return syn::Error::new_spanned(&input, "#[modal] doesn't support tuple structs")
                    .to_compile_error();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "#[modal] only works on structs")
                .to_compile_error();
        }
    };

    let default_impl = generate_default_impl(name, fields);
    let clone_impl = generate_clone_impl(name, fields);
    let metadata = generate_metadata(name, fields);

    if let Some(fields) = fields {
        let transformed_fields: Vec<_> = fields.named.iter().map(transform_field).collect();

        quote! {
            #(#other_attrs)*
            #vis struct #name {
                #(#transformed_fields),*
            }

            #default_impl
            #clone_impl
            #metadata
        }
    } else {
        quote! {
            #(#other_attrs)*
            #vis struct #name;

            #default_impl
            #clone_impl
            #metadata
        }
    }
}
