//! The `#[modal]` attribute macro for defining modal structs.
//!
//! This macro transforms a struct into a modal by:
//! - Wrapping non-Resource fields in `State<T>`
//! - Generating `Clone` and `Default` impls
//! - Creating metadata for use by `#[modal_impl]`

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Attribute, DeriveInput, Field, Fields, FieldsNamed, Ident, Meta, Type, parse2};

/// Field attributes
struct FieldAttrs {
    /// Skip wrapping in State<T>
    skip: bool,
}

impl FieldAttrs {
    fn parse(attrs: &[Attribute]) -> Self {
        let mut skip = false;

        for attr in attrs {
            if attr.path().is_ident("state") {
                // #[state(skip)]
                if let Meta::List(list) = attr.meta.clone() {
                    let _ = list.parse_nested_meta(|meta| {
                        if meta.path.is_ident("skip") {
                            skip = true;
                        }
                        Ok(())
                    });
                }
            }
        }

        Self { skip }
    }
}

/// Check if a type is Resource<T>
fn is_resource_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        return segment.ident == "Resource";
    }
    false
}

/// Transform a field, wrapping in State<T> or keeping Resource<T> as-is
fn transform_field(field: &Field) -> TokenStream {
    let attrs = FieldAttrs::parse(&field.attrs);
    let vis = &field.vis;
    let ident = &field.ident;
    let ty = &field.ty;

    // Filter out our custom attributes from the output
    let other_attrs: Vec<_> = field
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("state"))
        .collect();

    if attrs.skip {
        // #[state(skip)] - no wrapping
        quote! {
            #(#other_attrs)*
            #vis #ident: #ty
        }
    } else if is_resource_type(ty) {
        // Resource<T> stays as Resource<T>
        quote! {
            #(#other_attrs)*
            #vis #ident: #ty
        }
    } else {
        // Regular field -> State<T>
        quote! {
            #(#other_attrs)*
            #vis #ident: rafter::state::State<#ty>
        }
    }
}

/// Generate the Default impl for the modal
fn generate_default_impl(name: &Ident, fields: Option<&FieldsNamed>) -> TokenStream {
    let Some(fields) = fields else {
        // Unit struct
        return quote! {
            impl Default for #name {
                fn default() -> Self {
                    Self
                }
            }
        };
    };

    let field_defaults: Vec<_> = fields
        .named
        .iter()
        .map(|f| {
            let attrs = FieldAttrs::parse(&f.attrs);
            let ident = &f.ident;
            let ty = &f.ty;

            if attrs.skip {
                // #[state(skip)] - use type's Default
                quote! { #ident: Default::default() }
            } else if is_resource_type(ty) {
                // Resource<T> -> Resource::new()
                quote! { #ident: rafter::resource::Resource::new() }
            } else {
                // Regular -> State<T>::new(Default::default())
                quote! { #ident: rafter::state::State::new(Default::default()) }
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

/// Generate Clone impl for the modal
fn generate_clone_impl(name: &Ident, fields: Option<&FieldsNamed>) -> TokenStream {
    let Some(fields) = fields else {
        // Unit struct
        return quote! {
            impl Clone for #name {
                fn clone(&self) -> Self {
                    Self
                }
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

/// Generate metadata struct for use by #[modal_impl]
fn generate_metadata(name: &Ident, fields: Option<&FieldsNamed>) -> TokenStream {
    let metadata_name = format_ident!(
        "__rafter_modal_metadata_{}",
        name.to_string().to_lowercase()
    );

    let Some(fields) = fields else {
        // Unit struct - no dirty checking needed
        return quote! {
            #[doc(hidden)]
            #[allow(non_snake_case)]
            pub mod #metadata_name {
                use super::*;

                pub fn is_dirty(_modal: &#name) -> bool {
                    false
                }

                pub fn clear_dirty(_modal: &#name) {}
            }
        };
    };

    // Collect field names for dirty checking (excluding skipped fields)
    let dirty_fields: Vec<_> = fields
        .named
        .iter()
        .filter_map(|f| {
            let attrs = FieldAttrs::parse(&f.attrs);
            if attrs.skip { None } else { f.ident.as_ref() }
        })
        .collect();

    let is_dirty_checks = dirty_fields.iter().map(|f| {
        quote! { modal.#f.is_dirty() }
    });

    let clear_dirty_calls = dirty_fields.iter().map(|f| {
        quote! { modal.#f.clear_dirty(); }
    });

    quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub mod #metadata_name {
            use super::*;

            pub fn is_dirty(modal: &#name) -> bool {
                false #(|| #is_dirty_checks)*
            }

            pub fn clear_dirty(modal: &#name) {
                #(#clear_dirty_calls)*
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
    let generics = &input.generics;

    // Filter out doc attributes to preserve, but not our custom ones
    let other_attrs: Vec<_> = input
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("modal"))
        .collect();

    let fields = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            Fields::Named(f) => Some(f),
            Fields::Unit => None,
            Fields::Unnamed(_) => {
                return syn::Error::new_spanned(
                    &input,
                    "#[modal] does not support tuple structs",
                )
                .to_compile_error();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "#[modal] can only be applied to structs")
                .to_compile_error();
        }
    };

    // Generate implementations
    let default_impl = generate_default_impl(name, fields);
    let clone_impl = generate_clone_impl(name, fields);
    let metadata = generate_metadata(name, fields);

    if let Some(fields) = fields {
        // Struct with named fields
        let transformed_fields: Vec<_> = fields.named.iter().map(transform_field).collect();

        quote! {
            #(#other_attrs)*
            #vis struct #name #generics {
                #(#transformed_fields),*
            }

            #default_impl
            #clone_impl
            #metadata
        }
    } else {
        // Unit struct
        quote! {
            #(#other_attrs)*
            #vis struct #name #generics;

            #default_impl
            #clone_impl
            #metadata
        }
    }
}
