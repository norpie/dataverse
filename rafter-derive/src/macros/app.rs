use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Attribute, DeriveInput, Expr, ExprPath, Field, Fields, FieldsNamed, Ident, Meta, Type, parse2,
};

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
    /// This is async state (can be mutated from async handlers)
    async_state: bool,
}

impl FieldAttrs {
    fn parse(attrs: &[Attribute]) -> Self {
        let mut skip = false;
        let mut async_state = false;

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
            } else if attr.path().is_ident("async_state") {
                async_state = true;
            }
        }

        Self { skip, async_state }
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

/// Transform a field, optionally wrapping in State<T>
fn transform_field(field: &Field) -> TokenStream {
    let attrs = FieldAttrs::parse(&field.attrs);
    let vis = &field.vis;
    let ident = &field.ident;
    let ty = &field.ty;

    // Filter out our custom attributes from the output
    let other_attrs: Vec<_> = field
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("state") && !a.path().is_ident("async_state"))
        .collect();

    // Skip wrapping if:
    // - #[state(skip)] is present
    // - Type is already Resource<T> (implicitly async state)
    // - #[async_state] is present (uses AsyncResource internally)
    if attrs.skip {
        quote! {
            #(#other_attrs)*
            #vis #ident: #ty
        }
    } else if is_resource_type(ty) || attrs.async_state {
        // Resource types and async_state don't get wrapped in State
        // They have their own interior mutability
        quote! {
            #(#other_attrs)*
            #vis #ident: #ty
        }
    } else {
        // Wrap in State<T>
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

            if attrs.skip || is_resource_type(ty) || attrs.async_state {
                quote! { #ident: Default::default() }
            } else {
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

/// Generate inventory registration
fn generate_registration(name: &Ident) -> TokenStream {
    let name_str = name.to_string();

    quote! {
        inventory::submit! {
            rafter::app::AppRegistration::new(
                #name_str,
                || Box::new(#name::default()) as Box<dyn rafter::app::App>
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

    // Collect field names for dirty checking (excluding skipped and Resource fields)
    let dirty_fields: Vec<_> = fields
        .named
        .iter()
        .filter_map(|f| {
            let attrs = FieldAttrs::parse(&f.attrs);
            if attrs.skip || is_resource_type(&f.ty) || attrs.async_state {
                None
            } else {
                f.ident.as_ref()
            }
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

            pub fn clear_dirty(app: &mut #name) {
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
    let registration = generate_registration(name);
    let metadata = generate_metadata(name, &attrs, fields);

    quote! {
        #(#other_attrs)*
        #vis struct #name #generics {
            #(#transformed_fields),*
        }

        #default_impl
        #registration
        #metadata
    }
}
