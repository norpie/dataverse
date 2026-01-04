//! The `#[app]` attribute macro for defining app structs.
//!
//! Transforms a struct into an app by:
//! - Wrapping fields in `State<T>` (unless Resource, widget, or skipped)
//! - Generating `Clone` and `Default` impls
//! - Registering with inventory for auto-discovery
//! - Creating metadata for use by `#[app_impl]`

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Field, Fields, FieldsNamed, Ident, Token, parse2};

use super::fields::{has_state_skip, has_widget_attribute, is_resource_type};

/// Attributes for #[app].
struct AppAttrs {
    name: Option<String>,
    singleton: bool,
    on_panic: Option<Ident>,
}

impl AppAttrs {
    fn parse(attr: TokenStream) -> syn::Result<Self> {
        let mut attrs = Self {
            name: None,
            singleton: false,
            on_panic: None,
        };

        if !attr.is_empty() {
            let parser = syn::meta::parser(|meta| {
                if meta.path.is_ident("name") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    attrs.name = Some(value.value());
                } else if meta.path.is_ident("singleton") {
                    attrs.singleton = true;
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
fn generate_default_impl(name: &Ident, fields: &FieldsNamed) -> TokenStream {
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

    let is_dirty_checks = dirty_fields.iter().map(|f| quote! { self.#f.is_dirty() });
    let clear_dirty_calls = dirty_fields.iter().map(|f| quote! { self.#f.clear_dirty(); });
    let install_wakeup_calls = wakeup_fields.iter().map(|f| quote! { self.#f.install_wakeup(sender.clone()); });

    let widget_ids: Vec<_> = widget_fields.iter().map(|f| f.to_string()).collect();

    quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub mod #metadata_name {
            pub const WIDGET_FIELDS: &[&str] = &[#(#widget_ids),*];
        }

        impl #name {
            #[doc(hidden)]
            pub fn __app_config() -> rafter::AppConfig {
                rafter::AppConfig {
                    name: #config_name,
                    blur_on_background: false,
                    max_instances: #max_instances,
                    panic_behavior: #panic_behavior,
                }
            }

            #[doc(hidden)]
            pub fn __is_dirty(&self) -> bool {
                false #(|| #is_dirty_checks)*
            }

            #[doc(hidden)]
            pub fn __clear_dirty(&self) {
                #(#clear_dirty_calls)*
            }

            #[doc(hidden)]
            pub fn __install_wakeup(&self, sender: rafter::WakeupSender) {
                #(#install_wakeup_calls)*
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
    let default_impl = generate_default_impl(name, fields);
    let clone_impl = generate_clone_impl(name, fields);
    let registration = generate_registration(name);
    let metadata = generate_metadata(name, &attrs, fields);

    quote! {
        #(#other_attrs)*
        #vis struct #name {
            #(#transformed_fields),*
        }

        #default_impl
        #clone_impl
        #registration
        #metadata
    }
}
