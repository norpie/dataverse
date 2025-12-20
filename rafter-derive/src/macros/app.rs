use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Attribute, DeriveInput, Expr, ExprPath, Field, Fields, FieldsNamed, Ident, Meta, parse2,
};

use super::field_utils::{is_resource_type, is_widget_type};

/// Attributes that can be applied to the #[app] macro
struct AppAttrs {
    /// Panic behavior for this app
    on_panic: Option<Ident>,
}

impl AppAttrs {
    fn parse(attr: TokenStream) -> syn::Result<Self> {
        let mut on_panic = None;

        if !attr.is_empty() {
            // Parse: on_panic = ShowError
            let meta: Meta = parse2(attr)?;
            if let Meta::NameValue(nv) = meta
                && nv.path.is_ident("on_panic")
                && let Expr::Path(ExprPath { path, .. }) = &nv.value
                && let Some(ident) = path.get_ident()
            {
                on_panic = Some(ident.clone());
            }
        }

        Ok(Self { on_panic })
    }
}

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
        // Resource<T> stays as Resource<T> (it's already the unified type)
        quote! {
            #(#other_attrs)*
            #vis #ident: #ty
        }
    } else if is_widget_type(ty) {
        // Widget types (Input, List<T>, etc.) manage their own state
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

/// Generate the Default impl for the app
fn generate_default_impl(name: &Ident, fields: &FieldsNamed) -> TokenStream {
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
            } else if is_widget_type(ty) {
                // Widget types use Default
                quote! { #ident: Default::default() }
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

/// Generate Clone impl for the app
fn generate_clone_impl(name: &Ident, fields: &FieldsNamed) -> TokenStream {
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

/// Generate inventory registration
fn generate_registration(name: &Ident) -> TokenStream {
    let name_str = name.to_string();

    quote! {
        inventory::submit! {
            rafter::app::AppRegistration::new(
                #name_str,
                || Box::new(#name::default()) as Box<dyn rafter::app::CloneableApp>
            )
        }
    }
}

/// Generate metadata struct for use by #[app_impl]
fn generate_metadata(name: &Ident, attrs: &AppAttrs, fields: &FieldsNamed) -> TokenStream {
    let panic_behavior = match &attrs.on_panic {
        Some(ident) => quote! { rafter::app::PanicBehavior::#ident },
        None => quote! { rafter::app::PanicBehavior::ShowError },
    };

    // Collect field names for dirty checking (excluding skipped fields)
    // State<T> and Resource<T> both have is_dirty/clear_dirty
    let dirty_fields: Vec<_> = fields
        .named
        .iter()
        .filter_map(|f| {
            let attrs = FieldAttrs::parse(&f.attrs);
            if attrs.skip { None } else { f.ident.as_ref() }
        })
        .collect();

    let is_dirty_checks = dirty_fields.iter().map(|f| {
        quote! { app.#f.is_dirty() }
    });

    let clear_dirty_calls = dirty_fields.iter().map(|f| {
        quote! { app.#f.clear_dirty(); }
    });

    let metadata_name = format_ident!("__rafter_app_metadata_{}", name.to_string().to_lowercase());

    quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub mod #metadata_name {
            use super::*;

            pub const PANIC_BEHAVIOR: rafter::app::PanicBehavior = #panic_behavior;

            pub fn is_dirty(app: &#name) -> bool {
                false #(|| #is_dirty_checks)*
            }

            pub fn clear_dirty(app: &#name) {
                #(#clear_dirty_calls)*
            }
        }
    }
}

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attrs = match AppAttrs::parse(attr) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error(),
    };

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
        .filter(|a| !a.path().is_ident("app"))
        .collect();

    let fields = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            Fields::Named(f) => f,
            _ => {
                return syn::Error::new_spanned(
                    &input,
                    "#[app] only supports structs with named fields",
                )
                .to_compile_error();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "#[app] can only be applied to structs")
                .to_compile_error();
        }
    };

    // Transform fields
    let transformed_fields: Vec<_> = fields.named.iter().map(transform_field).collect();

    // Generate implementations
    let default_impl = generate_default_impl(name, fields);
    let clone_impl = generate_clone_impl(name, fields);
    let registration = generate_registration(name);
    let metadata = generate_metadata(name, &attrs, fields);

    quote! {
        #(#other_attrs)*
        #vis struct #name #generics {
            #(#transformed_fields),*
        }

        #default_impl
        #clone_impl
        #registration
        #metadata
    }
}
