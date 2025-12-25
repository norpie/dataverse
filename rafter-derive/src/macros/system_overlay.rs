//! The `#[system_overlay]` attribute macro for system overlays.
//!
//! System overlays are systems with a persistent visual presence (e.g., taskbar).
//! They combine System functionality (keybinds, handlers, events) with a rendered view.
//!
//! # Attributes
//!
//! - `position = Top | Bottom | Left | Right` - Edge position
//! - `height = N` - Height for Top/Bottom overlays
//! - `width = N` - Width for Left/Right overlays
//! - `x = N, y = N, width = N, height = N` - Absolute position
//!
//! # Examples
//!
//! ```ignore
//! #[system_overlay(position = Bottom, height = 1)]
//! struct Taskbar {
//!     apps: Vec<AppInfo>,
//! }
//!
//! #[system_overlay(position = Absolute, x = 0, y = 0, width = 20, height = 5)]
//! struct FloatingPanel {
//!     visible: bool,
//! }
//! ```

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Attribute, DeriveInput, Field, Fields, FieldsNamed, Ident, Meta, Token, parse2};

use super::field_utils::{is_resource_type, is_widget_type};

/// Parsed overlay position from attributes.
#[derive(Debug, Clone)]
enum OverlayPositionAttr {
    Top { height: u16 },
    Bottom { height: u16 },
    Left { width: u16 },
    Right { width: u16 },
    Absolute { x: u16, y: u16, width: u16, height: u16 },
}

/// Parsed attributes for #[system_overlay].
struct SystemOverlayAttrs {
    position: OverlayPositionAttr,
}

impl SystemOverlayAttrs {
    fn parse(attr: TokenStream) -> syn::Result<Self> {
        let mut position_type: Option<String> = None;
        let mut height: Option<u16> = None;
        let mut width: Option<u16> = None;
        let mut x: Option<u16> = None;
        let mut y: Option<u16> = None;

        if attr.is_empty() {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "#[system_overlay] requires position attributes, e.g., #[system_overlay(position = Bottom, height = 1)]",
            ));
        }

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
                return Err(meta.error(format!(
                    "unknown system_overlay attribute: `{}`",
                    meta.path.get_ident().map(|i| i.to_string()).unwrap_or_default()
                )));
            }
            Ok(())
        });

        syn::parse::Parser::parse2(parser, attr)?;

        let position_type = position_type.ok_or_else(|| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "#[system_overlay] requires `position` attribute",
            )
        })?;

        let position = match position_type.as_str() {
            "Top" => {
                let height = height.ok_or_else(|| {
                    syn::Error::new(
                        proc_macro2::Span::call_site(),
                        "Top position requires `height` attribute",
                    )
                })?;
                OverlayPositionAttr::Top { height }
            }
            "Bottom" => {
                let height = height.ok_or_else(|| {
                    syn::Error::new(
                        proc_macro2::Span::call_site(),
                        "Bottom position requires `height` attribute",
                    )
                })?;
                OverlayPositionAttr::Bottom { height }
            }
            "Left" => {
                let width = width.ok_or_else(|| {
                    syn::Error::new(
                        proc_macro2::Span::call_site(),
                        "Left position requires `width` attribute",
                    )
                })?;
                OverlayPositionAttr::Left { width }
            }
            "Right" => {
                let width = width.ok_or_else(|| {
                    syn::Error::new(
                        proc_macro2::Span::call_site(),
                        "Right position requires `width` attribute",
                    )
                })?;
                OverlayPositionAttr::Right { width }
            }
            "Absolute" => {
                let x = x.ok_or_else(|| {
                    syn::Error::new(
                        proc_macro2::Span::call_site(),
                        "Absolute position requires `x` attribute",
                    )
                })?;
                let y = y.ok_or_else(|| {
                    syn::Error::new(
                        proc_macro2::Span::call_site(),
                        "Absolute position requires `y` attribute",
                    )
                })?;
                let width = width.ok_or_else(|| {
                    syn::Error::new(
                        proc_macro2::Span::call_site(),
                        "Absolute position requires `width` attribute",
                    )
                })?;
                let height = height.ok_or_else(|| {
                    syn::Error::new(
                        proc_macro2::Span::call_site(),
                        "Absolute position requires `height` attribute",
                    )
                })?;
                OverlayPositionAttr::Absolute { x, y, width, height }
            }
            other => {
                return Err(syn::Error::new(
                    proc_macro2::Span::call_site(),
                    format!(
                        "invalid position: `{}`. Expected one of: Top, Bottom, Left, Right, Absolute",
                        other
                    ),
                ));
            }
        };

        Ok(Self { position })
    }
}

/// Field attributes for system overlay fields.
struct FieldAttrs {
    /// Skip wrapping in State<T>.
    skip: bool,
}

impl FieldAttrs {
    fn parse(attrs: &[Attribute]) -> Self {
        let mut skip = false;

        for attr in attrs {
            if attr.path().is_ident("state") {
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

/// Transform a field, wrapping in State<T> like apps do.
fn transform_field(field: &Field) -> TokenStream {
    let attrs = FieldAttrs::parse(&field.attrs);
    let vis = &field.vis;
    let ident = &field.ident;
    let ty = &field.ty;

    let other_attrs: Vec<_> = field
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("state") && !a.path().is_ident("widget"))
        .collect();

    if attrs.skip {
        quote! {
            #(#other_attrs)*
            #vis #ident: #ty
        }
    } else if is_resource_type(ty) {
        quote! {
            #(#other_attrs)*
            #vis #ident: #ty
        }
    } else if is_widget_type(ty, &field.attrs) {
        quote! {
            #(#other_attrs)*
            #vis #ident: #ty
        }
    } else {
        quote! {
            #(#other_attrs)*
            #vis #ident: rafter::state::State<#ty>
        }
    }
}

/// Generate Default impl.
fn generate_default_impl(name: &Ident, fields: &FieldsNamed) -> TokenStream {
    let field_defaults: Vec<_> = fields
        .named
        .iter()
        .map(|f| {
            let attrs = FieldAttrs::parse(&f.attrs);
            let ident = &f.ident;
            let ty = &f.ty;

            if attrs.skip {
                quote! { #ident: Default::default() }
            } else if is_resource_type(ty) {
                quote! { #ident: rafter::resource::Resource::new() }
            } else if is_widget_type(ty, &f.attrs) {
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
            rafter::layers::SystemOverlayRegistration::new(
                #name_str,
                || Box::new(rafter::layers::SystemOverlayInstance::new(#name::default())) as Box<dyn rafter::layers::AnySystemOverlay>
            )
        }
    }
}

/// Generate position constant.
fn generate_position_const(position: &OverlayPositionAttr) -> TokenStream {
    match position {
        OverlayPositionAttr::Top { height } => {
            quote! { rafter::layers::SystemOverlayPosition::Top { height: #height } }
        }
        OverlayPositionAttr::Bottom { height } => {
            quote! { rafter::layers::SystemOverlayPosition::Bottom { height: #height } }
        }
        OverlayPositionAttr::Left { width } => {
            quote! { rafter::layers::SystemOverlayPosition::Left { width: #width } }
        }
        OverlayPositionAttr::Right { width } => {
            quote! { rafter::layers::SystemOverlayPosition::Right { width: #width } }
        }
        OverlayPositionAttr::Absolute { x, y, width, height } => {
            quote! { rafter::layers::SystemOverlayPosition::Absolute { x: #x, y: #y, width: #width, height: #height } }
        }
    }
}

/// Generate metadata module.
fn generate_metadata(name: &Ident, attrs: &SystemOverlayAttrs, fields: &FieldsNamed) -> TokenStream {
    let name_str = name.to_string();
    let position = generate_position_const(&attrs.position);

    // Collect field names for dirty checking
    let dirty_fields: Vec<_> = fields
        .named
        .iter()
        .filter_map(|f| {
            let attrs = FieldAttrs::parse(&f.attrs);
            if attrs.skip { None } else { f.ident.as_ref() }
        })
        .collect();

    // Collect field names that need wakeup
    let wakeup_fields: Vec<_> = fields
        .named
        .iter()
        .filter_map(|f| {
            let attrs = FieldAttrs::parse(&f.attrs);
            if attrs.skip || is_widget_type(&f.ty, &f.attrs) {
                None
            } else {
                f.ident.as_ref()
            }
        })
        .collect();

    let is_dirty_checks = dirty_fields.iter().map(|f| {
        quote! { overlay.#f.is_dirty() }
    });

    let clear_dirty_calls = dirty_fields.iter().map(|f| {
        quote! { overlay.#f.clear_dirty(); }
    });

    let install_wakeup_calls = wakeup_fields.iter().map(|f| {
        quote! { overlay.#f.install_wakeup(sender.clone()); }
    });

    let metadata_name = format_ident!("__rafter_system_overlay_metadata_{}", name.to_string().to_lowercase());

    quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub mod #metadata_name {
            use super::*;

            pub const NAME: &str = #name_str;
            pub const POSITION: rafter::layers::SystemOverlayPosition = #position;

            pub fn is_dirty(overlay: &#name) -> bool {
                false #(|| #is_dirty_checks)*
            }

            pub fn clear_dirty(overlay: &#name) {
                #(#clear_dirty_calls)*
            }

            pub fn install_wakeup(overlay: &#name, sender: rafter::runtime::wakeup::WakeupSender) {
                #(#install_wakeup_calls)*
            }
        }
    }
}

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attrs = match SystemOverlayAttrs::parse(attr) {
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

    let other_attrs: Vec<_> = input
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("system_overlay"))
        .collect();

    let fields = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            Fields::Named(f) => f,
            Fields::Unit => {
                // Unit struct - no fields
                let registration = generate_registration(name);
                let position = generate_position_const(&attrs.position);
                let name_str = name.to_string();
                let metadata_name = format_ident!("__rafter_system_overlay_metadata_{}", name.to_string().to_lowercase());

                return quote! {
                    #(#other_attrs)*
                    #vis struct #name #generics;

                    impl Clone for #name {
                        fn clone(&self) -> Self { Self }
                    }

                    impl Default for #name {
                        fn default() -> Self { Self }
                    }

                    #registration

                    #[doc(hidden)]
                    #[allow(non_snake_case)]
                    pub mod #metadata_name {
                        use super::*;

                        pub const NAME: &str = #name_str;
                        pub const POSITION: rafter::layers::SystemOverlayPosition = #position;

                        pub fn is_dirty(_overlay: &#name) -> bool { false }
                        pub fn clear_dirty(_overlay: &#name) {}
                        pub fn install_wakeup(_overlay: &#name, _sender: rafter::runtime::wakeup::WakeupSender) {}
                    }
                };
            }
            _ => {
                return syn::Error::new_spanned(
                    &input,
                    "#[system_overlay] only supports structs with named fields or unit structs",
                )
                .to_compile_error();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "#[system_overlay] can only be applied to structs")
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
