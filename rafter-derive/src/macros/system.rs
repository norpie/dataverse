//! The `#[system]` attribute macro for defining system structs.
//!
//! Systems have global keybinds and optionally a visual overlay.
//!
//! # Examples
//!
//! ```ignore
//! // Keybinds only (no overlay)
//! #[system]
//! struct GlobalKeys;
//!
//! // With overlay
//! #[system(position = Bottom, height = 1)]
//! struct Taskbar {
//!     apps: Vec<AppInfo>,
//! }
//! ```

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Field, Fields, FieldsNamed, Ident, Token, parse2};

use super::fields::{has_state_skip, has_widget_attribute, is_resource_type};

/// Overlay position from attributes.
#[derive(Clone)]
enum OverlayPosition {
    Top { height: u16 },
    Bottom { height: u16 },
    Left { width: u16 },
    Right { width: u16 },
    Absolute { x: u16, y: u16, width: u16, height: u16 },
}

/// Parsed attributes for #[system].
struct SystemAttrs {
    position: Option<OverlayPosition>,
}

impl SystemAttrs {
    fn parse(attr: TokenStream) -> syn::Result<Self> {
        let mut position_type: Option<String> = None;
        let mut height: Option<u16> = None;
        let mut width: Option<u16> = None;
        let mut x: Option<u16> = None;
        let mut y: Option<u16> = None;

        if !attr.is_empty() {
            let parser = syn::meta::parser(|meta| {
                if meta.path.is_ident("position") {
                    meta.input.parse::<Token![=]>()?;
                    let ident: Ident = meta.input.parse()?;
                    position_type = Some(ident.to_string());
                } else if meta.path.is_ident("height") {
                    meta.input.parse::<Token![=]>()?;
                    let lit: syn::LitInt = meta.input.parse()?;
                    height = Some(lit.base10_parse()?);
                } else if meta.path.is_ident("width") {
                    meta.input.parse::<Token![=]>()?;
                    let lit: syn::LitInt = meta.input.parse()?;
                    width = Some(lit.base10_parse()?);
                } else if meta.path.is_ident("x") {
                    meta.input.parse::<Token![=]>()?;
                    let lit: syn::LitInt = meta.input.parse()?;
                    x = Some(lit.base10_parse()?);
                } else if meta.path.is_ident("y") {
                    meta.input.parse::<Token![=]>()?;
                    let lit: syn::LitInt = meta.input.parse()?;
                    y = Some(lit.base10_parse()?);
                } else {
                    return Err(meta.error("unknown system attribute"));
                }
                Ok(())
            });
            syn::parse::Parser::parse2(parser, attr)?;
        }

        let position = if let Some(pos) = position_type {
            Some(match pos.as_str() {
                "Top" => {
                    let height = height.ok_or_else(|| {
                        syn::Error::new(proc_macro2::Span::call_site(), "Top requires height")
                    })?;
                    OverlayPosition::Top { height }
                }
                "Bottom" => {
                    let height = height.ok_or_else(|| {
                        syn::Error::new(proc_macro2::Span::call_site(), "Bottom requires height")
                    })?;
                    OverlayPosition::Bottom { height }
                }
                "Left" => {
                    let width = width.ok_or_else(|| {
                        syn::Error::new(proc_macro2::Span::call_site(), "Left requires width")
                    })?;
                    OverlayPosition::Left { width }
                }
                "Right" => {
                    let width = width.ok_or_else(|| {
                        syn::Error::new(proc_macro2::Span::call_site(), "Right requires width")
                    })?;
                    OverlayPosition::Right { width }
                }
                "Absolute" => {
                    let x = x.ok_or_else(|| {
                        syn::Error::new(proc_macro2::Span::call_site(), "Absolute requires x")
                    })?;
                    let y = y.ok_or_else(|| {
                        syn::Error::new(proc_macro2::Span::call_site(), "Absolute requires y")
                    })?;
                    let width = width.ok_or_else(|| {
                        syn::Error::new(proc_macro2::Span::call_site(), "Absolute requires width")
                    })?;
                    let height = height.ok_or_else(|| {
                        syn::Error::new(proc_macro2::Span::call_site(), "Absolute requires height")
                    })?;
                    OverlayPosition::Absolute { x, y, width, height }
                }
                other => {
                    return Err(syn::Error::new(
                        proc_macro2::Span::call_site(),
                        format!("invalid position: {other}. Expected Top, Bottom, Left, Right, or Absolute"),
                    ));
                }
            })
        } else {
            None
        };

        Ok(Self { position })
    }
}

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

/// Generate inventory registration.
fn generate_registration(name: &Ident) -> TokenStream {
    let name_str = name.to_string();

    quote! {
        inventory::submit! {
            rafter::SystemRegistration::new(
                #name_str,
                || Box::new(#name::default()) as Box<dyn rafter::AnySystem>
            )
        }
    }
}

/// Generate position constant.
fn generate_position_const(position: &OverlayPosition) -> TokenStream {
    match position {
        OverlayPosition::Top { height } => {
            quote! { rafter::OverlayPosition::Top { height: #height } }
        }
        OverlayPosition::Bottom { height } => {
            quote! { rafter::OverlayPosition::Bottom { height: #height } }
        }
        OverlayPosition::Left { width } => {
            quote! { rafter::OverlayPosition::Left { width: #width } }
        }
        OverlayPosition::Right { width } => {
            quote! { rafter::OverlayPosition::Right { width: #width } }
        }
        OverlayPosition::Absolute { x, y, width, height } => {
            quote! { rafter::OverlayPosition::Absolute { x: #x, y: #y, width: #width, height: #height } }
        }
    }
}

/// Generate metadata for #[system_impl].
fn generate_metadata(name: &Ident, attrs: &SystemAttrs, fields: Option<&FieldsNamed>) -> TokenStream {
    let name_str = name.to_string();

    let position_const = attrs.position.as_ref().map(|p| {
        let pos = generate_position_const(p);
        quote! { pub const OVERLAY_POSITION: rafter::OverlayPosition = #pos; }
    });

    let has_overlay = attrs.position.is_some();

    let dirty_fields: Vec<_> = fields
        .map(|f| {
            f.named
                .iter()
                .filter(|f| !has_state_skip(&f.attrs))
                .filter_map(|f| f.ident.as_ref())
                .collect()
        })
        .unwrap_or_default();

    let wakeup_fields: Vec<_> = fields
        .map(|f| {
            f.named
                .iter()
                .filter(|f| !has_state_skip(&f.attrs) && !has_widget_attribute(&f.attrs))
                .filter_map(|f| f.ident.as_ref())
                .collect()
        })
        .unwrap_or_default();

    let is_dirty_checks = dirty_fields.iter().map(|f| quote! { self.#f.is_dirty() });
    let clear_dirty_calls = dirty_fields.iter().map(|f| quote! { self.#f.clear_dirty(); });
    let install_wakeup_calls = wakeup_fields.iter().map(|f| quote! { self.#f.install_wakeup(sender.clone()); });

    let metadata_name = format_ident!("__rafter_system_metadata_{}", name.to_string().to_lowercase());

    quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub mod #metadata_name {
            use super::*;

            pub const NAME: &str = #name_str;
            pub const HAS_OVERLAY: bool = #has_overlay;
            #position_const
        }

        impl #name {
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
    let attrs = match SystemAttrs::parse(attr) {
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
        .filter(|a| !a.path().is_ident("system"))
        .collect();

    let fields = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            Fields::Named(f) => Some(f),
            Fields::Unit => None,
            _ => {
                return syn::Error::new_spanned(&input, "#[system] doesn't support tuple structs")
                    .to_compile_error();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "#[system] only works on structs")
                .to_compile_error();
        }
    };

    let default_impl = generate_default_impl(name, fields);
    let clone_impl = generate_clone_impl(name, fields);
    let registration = generate_registration(name);
    let metadata = generate_metadata(name, &attrs, fields);

    if let Some(fields) = fields {
        let transformed_fields: Vec<_> = fields.named.iter().map(transform_field).collect();

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
    } else {
        quote! {
            #(#other_attrs)*
            #vis struct #name;

            #default_impl
            #clone_impl
            #registration
            #metadata
        }
    }
}
