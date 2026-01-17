//! The `#[app]` attribute macro for defining app structs.
//!
//! Transforms a struct into an app by:
//! - Wrapping fields in `State<T>` (unless Resource, widget, or skipped)
//! - Generating `Clone` and `Default` impls
//! - Registering with inventory for auto-discovery
//! - Creating metadata for use by `#[app_impl]`
//!
//! Supports the `pages` flag for page routing:
//! - `#[app(pages)]` - enables page routing (expects `Page` enum in scope)

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Field, Fields, FieldsNamed, Ident, Token, parse2};

use super::fields::{has_state_skip, has_widget_attribute, is_resource_type};

/// Attributes for #[app].
struct AppAttrs {
    name: Option<String>,
    singleton: bool,
    on_panic: Option<Ident>,
    on_blur: Option<Ident>,
    /// Whether page routing is enabled (expects `Page` enum in scope)
    pages: bool,
}

impl AppAttrs {
    fn parse(attr: TokenStream) -> syn::Result<Self> {
        let mut attrs = Self {
            name: None,
            singleton: false,
            on_panic: None,
            on_blur: None,
            pages: false,
        };

        if !attr.is_empty() {
            let parser = syn::meta::parser(|meta| {
                if meta.path.is_ident("name") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    attrs.name = Some(value.value());
                } else if meta.path.is_ident("singleton") {
                    attrs.singleton = true;
                } else if meta.path.is_ident("pages") {
                    attrs.pages = true;
                } else if meta.path.is_ident("on_panic") {
                    meta.input.parse::<Token![=]>()?;
                    let ident: Ident = meta.input.parse()?;
                    let valid = ["Close", "Restart", "Ignore"];
                    if !valid.contains(&ident.to_string().as_str()) {
                        return Err(syn::Error::new(
                            ident.span(),
                            format!("expected one of: {}", valid.join(", ")),
                        ));
                    }
                    attrs.on_panic = Some(ident);
                } else if meta.path.is_ident("on_blur") {
                    meta.input.parse::<Token![=]>()?;
                    let ident: Ident = meta.input.parse()?;
                    let valid = ["Continue", "Sleep", "Close"];
                    if !valid.contains(&ident.to_string().as_str()) {
                        return Err(syn::Error::new(
                            ident.span(),
                            format!("expected one of: {}", valid.join(", ")),
                        ));
                    }
                    attrs.on_blur = Some(ident);
                } else {
                    return Err(meta.error("unknown app attribute"));
                }
                Ok(())
            });
            syn::parse::Parser::parse2(parser, attr)?;
        }

        Ok(attrs)
    }
}

/// Transform a field, wrapping in State<T> if needed.
fn transform_field(field: &Field) -> TokenStream {
    let vis = &field.vis;
    let ident = &field.ident;
    let ty = &field.ty;

    // Filter out our custom attributes
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
fn generate_default_impl(name: &Ident, fields: &FieldsNamed, attrs: &AppAttrs) -> TokenStream {
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

    let page_field = if attrs.pages {
        quote! { __page: rafter::State::new(Page::default()), }
    } else {
        quote! {}
    };

    let fields_init = if field_defaults.is_empty() {
        quote! {
            #page_field
            __handler_registry: rafter::HandlerRegistry::new()
        }
    } else {
        quote! {
            #(#field_defaults),*,
            #page_field
            __handler_registry: rafter::HandlerRegistry::new()
        }
    };

    quote! {
        impl Default for #name {
            fn default() -> Self {
                Self {
                    #fields_init
                }
            }
        }
    }
}

/// Generate Clone impl.
fn generate_clone_impl(name: &Ident, fields: &FieldsNamed, attrs: &AppAttrs) -> TokenStream {
    let field_clones: Vec<_> = fields
        .named
        .iter()
        .map(|f| {
            let ident = &f.ident;
            quote! { #ident: self.#ident.clone() }
        })
        .collect();

    let page_field = if attrs.pages {
        quote! { __page: self.__page.clone(), }
    } else {
        quote! {}
    };

    let fields_clone = if field_clones.is_empty() {
        quote! {
            #page_field
            __handler_registry: self.__handler_registry.clone()
        }
    } else {
        quote! {
            #(#field_clones),*,
            #page_field
            __handler_registry: self.__handler_registry.clone()
        }
    };

    quote! {
        impl Clone for #name {
            fn clone(&self) -> Self {
                Self {
                    #fields_clone
                }
            }
        }
    }
}

/// Generate inventory registration.
fn generate_registration(name: &Ident) -> TokenStream {
    let name_str = name.to_string();

    quote! {
        inventory::submit! {
            rafter::AppRegistration::new(
                #name_str,
                || Box::new(#name::default()) as Box<dyn rafter::CloneableApp>
            )
        }
    }
}

/// Generate metadata module for #[app_impl].
fn generate_metadata(name: &Ident, attrs: &AppAttrs, fields: &FieldsNamed) -> TokenStream {
    let metadata_name = format_ident!("__rafter_app_metadata_{}", name.to_string().to_lowercase());

    let config_name = match &attrs.name {
        Some(n) => quote! { #n },
        None => {
            let n = name.to_string();
            quote! { #n }
        }
    };

    let max_instances = if attrs.singleton {
        quote! { Some(1) }
    } else {
        quote! { None }
    };

    let panic_behavior = match &attrs.on_panic {
        Some(ident) => quote! { rafter::PanicBehavior::#ident },
        None => quote! { rafter::PanicBehavior::Close },
    };

    let blur_policy = match &attrs.on_blur {
        Some(ident) => quote! { rafter::BlurPolicy::#ident },
        None => quote! { rafter::BlurPolicy::Continue },
    };

    let has_pages = attrs.pages;

    // Fields for dirty checking (all non-skipped fields)
    let dirty_fields: Vec<_> = fields
        .named
        .iter()
        .filter(|f| !has_state_skip(&f.attrs))
        .filter_map(|f| f.ident.as_ref())
        .collect();

    // Fields for wakeup (State and Resource, not widgets)
    let wakeup_fields: Vec<_> = fields
        .named
        .iter()
        .filter(|f| !has_state_skip(&f.attrs) && !has_widget_attribute(&f.attrs))
        .filter_map(|f| f.ident.as_ref())
        .collect();

    // Fields marked as widgets
    let widget_fields: Vec<_> = fields
        .named
        .iter()
        .filter(|f| has_widget_attribute(&f.attrs))
        .filter_map(|f| f.ident.as_ref())
        .collect();

    let widget_ids: Vec<_> = widget_fields.iter().map(|f| f.to_string()).collect();

    // Include __page in dirty checking and wakeup if pages is enabled
    let page_dirty = if attrs.pages {
        quote! { || app.__page.is_dirty() }
    } else {
        quote! {}
    };

    let page_clear_dirty = if attrs.pages {
        quote! { app.__page.clear_dirty(); }
    } else {
        quote! {}
    };

    let page_wakeup = if attrs.pages {
        quote! { app.__page.install_wakeup(sender.clone()); }
    } else {
        quote! {}
    };

    quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub mod #metadata_name {
            use super::*;

            pub const WIDGET_FIELDS: &[&str] = &[#(#widget_ids),*];
            pub const PANIC_BEHAVIOR: rafter::PanicBehavior = #panic_behavior;
            pub const HAS_PAGES: bool = #has_pages;

            pub fn config() -> rafter::AppConfig {
                rafter::AppConfig {
                    name: #config_name,
                    on_blur: #blur_policy,
                    max_instances: #max_instances,
                    panic_behavior: PANIC_BEHAVIOR,
                }
            }

            pub fn is_dirty(app: &#name) -> bool {
                false #(|| app.#dirty_fields.is_dirty())* #page_dirty
            }

            pub fn clear_dirty(app: &#name) {
                #(app.#dirty_fields.clear_dirty();)*
                #page_clear_dirty
            }

            pub fn install_wakeup(app: &#name, sender: rafter::WakeupSender) {
                #(app.#wakeup_fields.install_wakeup(sender.clone());)*
                #page_wakeup
            }
        }
    }
}

/// Generate singleton helper methods for apps marked with `singleton`.
fn generate_singleton_methods(name: &Ident, attrs: &AppAttrs) -> TokenStream {
    if !attrs.singleton {
        return quote! {};
    }

    quote! {
        impl #name {
            /// Get the existing singleton instance, or spawn a new one.
            ///
            /// This method ensures only one instance of this app exists.
            pub fn get_or_spawn(gx: &rafter::GlobalContext) -> Result<rafter::InstanceId, rafter::SpawnError> {
                if let Some(id) = gx.instance_of::<Self>() {
                    Ok(id)
                } else {
                    gx.spawn(Self::default())
                }
            }

            /// Get the existing singleton instance, or spawn and focus a new one.
            ///
            /// This method ensures only one instance of this app exists and is focused.
            pub fn get_or_spawn_and_focus(gx: &rafter::GlobalContext) -> Result<rafter::InstanceId, rafter::SpawnError> {
                let id = Self::get_or_spawn(gx)?;
                gx.focus_instance(id);
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

    let other_attrs: Vec<_> = input
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("app"))
        .collect();

    let fields = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            Fields::Named(f) => f,
            _ => {
                return syn::Error::new_spanned(&input, "#[app] requires named fields")
                    .to_compile_error();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "#[app] only works on structs")
                .to_compile_error();
        }
    };

    let transformed_fields: Vec<_> = fields.named.iter().map(transform_field).collect();
    let default_impl = generate_default_impl(name, fields, &attrs);
    let clone_impl = generate_clone_impl(name, fields, &attrs);
    let registration = generate_registration(name);
    let metadata = generate_metadata(name, &attrs, fields);
    let singleton_methods = generate_singleton_methods(name, &attrs);

    // Generate the __page field if pages is enabled
    let page_field = if attrs.pages {
        quote! {
            #[doc(hidden)]
            __page: rafter::State<Page>,
        }
    } else {
        quote! {}
    };

    // Handle empty fields case to avoid trailing comma issues
    let fields_tokens = if transformed_fields.is_empty() {
        quote! {
            #page_field
            #[doc(hidden)]
            __handler_registry: rafter::HandlerRegistry,
        }
    } else {
        quote! {
            #(#transformed_fields),*,
            #page_field
            #[doc(hidden)]
            __handler_registry: rafter::HandlerRegistry,
        }
    };

    quote! {
        #(#other_attrs)*
        #vis struct #name {
            #fields_tokens
        }

        #default_impl
        #clone_impl
        #registration
        #metadata
        #singleton_methods
    }
}
