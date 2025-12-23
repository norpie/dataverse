use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Attribute, DeriveInput, Field, Fields, FieldsNamed, Ident, Meta, Token, parse2};

use super::field_utils::{is_resource_type, is_widget_type};

/// Attributes that can be applied to the #[app] macro
///
/// Supported attributes:
/// - `#[app]` - basic app with default config
/// - `#[app(name = "My App")]` - custom display name
/// - `#[app(singleton)]` - max 1 instance
/// - `#[app(on_blur = Sleep)]` - blur policy (Continue, Sleep, Close)
/// - `#[app(persistent)]` - cannot be force-closed
/// - `#[app(on_panic = RestartApp)]` - panic behavior
///
/// Can be combined:
/// - `#[app(name = "Queue", singleton, persistent, on_blur = Continue)]`
struct AppAttrs {
    /// Custom display name (defaults to type name)
    name: Option<String>,
    /// Singleton app (max 1 instance)
    singleton: bool,
    /// Persistent app (cannot be force-closed)
    persistent: bool,
    /// Blur policy
    on_blur: Option<Ident>,
    /// Panic behavior for this app
    on_panic: Option<Ident>,
}

impl AppAttrs {
    fn parse(attr: TokenStream) -> syn::Result<Self> {
        let mut name = None;
        let mut singleton = false;
        let mut persistent = false;
        let mut on_blur = None;
        let mut on_panic = None;

        if !attr.is_empty() {
            // Parse comma-separated attributes
            let parser = syn::meta::parser(|meta| {
                if meta.path.is_ident("name") {
                    // name = "Display Name"
                    let value: syn::LitStr = meta.value()?.parse()?;
                    name = Some(value.value());
                } else if meta.path.is_ident("singleton") {
                    singleton = true;
                } else if meta.path.is_ident("persistent") {
                    persistent = true;
                } else if meta.path.is_ident("on_blur") {
                    // on_blur = Continue | Sleep | Close
                    meta.input.parse::<Token![=]>()?;
                    let ident: Ident = meta.input.parse()?;
                    on_blur = Some(ident);
                } else if meta.path.is_ident("on_panic") {
                    // on_panic = ShowError | RestartApp | CrashRuntime
                    meta.input.parse::<Token![=]>()?;
                    let ident: Ident = meta.input.parse()?;
                    on_panic = Some(ident);
                } else {
                    return Err(meta.error(format!(
                        "unknown app attribute: `{}`",
                        meta.path
                            .get_ident()
                            .map(|i| i.to_string())
                            .unwrap_or_default()
                    )));
                }
                Ok(())
            });
            syn::parse::Parser::parse2(parser, attr)?;
        }

        Ok(Self {
            name,
            singleton,
            persistent,
            on_blur,
            on_panic,
        })
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

/// Transform a field, wrapping in State<T> or keeping Resource<T>/widgets as-is
fn transform_field(field: &Field) -> TokenStream {
    let attrs = FieldAttrs::parse(&field.attrs);
    let vis = &field.vis;
    let ident = &field.ident;
    let ty = &field.ty;

    // Filter out our custom attributes from the output
    let other_attrs: Vec<_> = field
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("state") && !a.path().is_ident("widget"))
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
    } else if is_widget_type(ty, &field.attrs) {
        // Widget types (built-in or #[widget]) manage their own state
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
            } else if is_widget_type(ty, &f.attrs) {
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

    // Generate AppConfig
    let config_name = match &attrs.name {
        Some(n) => quote! { #n },
        None => {
            let name_str = name.to_string();
            quote! { #name_str }
        }
    };

    let on_blur = match &attrs.on_blur {
        Some(ident) => quote! { rafter::app::BlurPolicy::#ident },
        None => quote! { rafter::app::BlurPolicy::Continue },
    };

    let persistent = attrs.persistent;

    let max_instances = if attrs.singleton {
        quote! { Some(1) }
    } else {
        quote! { None }
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

            pub fn config() -> rafter::app::AppConfig {
                rafter::app::AppConfig {
                    name: #config_name,
                    on_blur: #on_blur,
                    persistent: #persistent,
                    max_instances: #max_instances,
                }
            }

            pub fn is_dirty(app: &#name) -> bool {
                false #(|| #is_dirty_checks)*
            }

            pub fn clear_dirty(app: &#name) {
                #(#clear_dirty_calls)*
            }
        }
    }
}

/// Generate singleton helper methods for apps with `singleton` attribute
fn generate_singleton_methods(name: &Ident, attrs: &AppAttrs) -> TokenStream {
    if !attrs.singleton {
        return quote! {};
    }

    quote! {
        impl #name {
            /// Get the existing singleton instance, or spawn a new one.
            ///
            /// This method ensures only one instance of this app exists.
            pub fn get_or_spawn(cx: &rafter::context::AppContext) -> Result<rafter::app::InstanceId, rafter::app::SpawnError> {
                if let Some(id) = cx.instance_of::<Self>() {
                    Ok(id)
                } else {
                    cx.spawn::<Self>(Self::default())
                }
            }

            /// Get the existing singleton instance, or spawn and focus a new one.
            ///
            /// This method ensures only one instance of this app exists and is focused.
            pub fn get_or_spawn_and_focus(cx: &rafter::context::AppContext) -> Result<rafter::app::InstanceId, rafter::app::SpawnError> {
                let id = Self::get_or_spawn(cx)?;
                cx.focus(id);
                Ok(id)
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
    let singleton_methods = generate_singleton_methods(name, &attrs);

    quote! {
        #(#other_attrs)*
        #vis struct #name #generics {
            #(#transformed_fields),*
        }

        #default_impl
        #clone_impl
        #registration
        #metadata
        #singleton_methods
    }
}
