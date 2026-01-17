//! The `#[modal]` attribute macro for defining modal structs.
//!
//! Supports attributes:
//! - `#[modal]` - default centered position, auto size
//! - `#[modal(pages)]` - enable page routing (expects `Page` enum in scope)
//! - `#[modal(size = Sm)]` - small size preset
//! - `#[modal(size = Md)]` - medium size preset
//! - `#[modal(size = Lg)]` - large size preset
//! - `#[modal(size = Fixed { width: 40, height: 10 })]` - fixed size
//! - `#[modal(size = Proportional { width: 0.5, height: 0.3 })]` - proportional size
//! - `#[modal(position = At { x: 5, y: 3 })]` - absolute position
//! - `#[modal(position = Centered)]` - centered (default)

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Field, Fields, FieldsNamed, Ident, parse2};

use super::fields::{has_state_skip, has_widget_attribute, is_resource_type};

/// Parsed modal size from attributes.
#[derive(Debug, Clone)]
enum ModalSize {
    Auto,
    Sm,
    Md,
    Lg,
    Fixed { width: u16, height: u16 },
    Proportional { width: f32, height: f32 },
}

/// Parsed modal position from attributes.
#[derive(Debug, Clone)]
enum ModalPosition {
    Centered,
    At { x: u16, y: u16 },
}

/// Parsed attributes for #[modal].
struct ModalAttrs {
    size: Option<ModalSize>,
    position: Option<ModalPosition>,
    /// Whether page routing is enabled (expects `Page` enum in scope)
    pages: bool,
}

impl ModalAttrs {
    fn parse(attr: TokenStream) -> syn::Result<Self> {
        let mut size: Option<ModalSize> = None;
        let mut position: Option<ModalPosition> = None;
        let mut pages = false;

        if attr.is_empty() {
            return Ok(Self { size, position, pages });
        }

        let parser = syn::meta::parser(|meta| {
            if meta.path.is_ident("pages") {
                pages = true;
            } else if meta.path.is_ident("size") {
                // Parse: size = Sm or size = Fixed { width: 40, height: 10 }
                let _eq: syn::Token![=] = meta.input.parse()?;
                let ident: syn::Ident = meta.input.parse()?;

                match ident.to_string().as_str() {
                    "Auto" => size = Some(ModalSize::Auto),
                    "Sm" => size = Some(ModalSize::Sm),
                    "Md" => size = Some(ModalSize::Md),
                    "Lg" => size = Some(ModalSize::Lg),
                    "Fixed" => {
                        // Parse: Fixed { width: N, height: N }
                        let content;
                        syn::braced!(content in meta.input);
                        let mut width: Option<u16> = None;
                        let mut height: Option<u16> = None;

                        while !content.is_empty() {
                            let field: syn::Ident = content.parse()?;
                            let _colon: syn::Token![:] = content.parse()?;
                            let value: syn::LitInt = content.parse()?;

                            if field == "width" {
                                width = Some(value.base10_parse()?);
                            } else if field == "height" {
                                height = Some(value.base10_parse()?);
                            }

                            if content.peek(syn::Token![,]) {
                                let _comma: syn::Token![,] = content.parse()?;
                            }
                        }

                        size = Some(ModalSize::Fixed {
                            width: width.unwrap_or(40),
                            height: height.unwrap_or(10),
                        });
                    }
                    "Proportional" => {
                        // Parse: Proportional { width: 0.5, height: 0.3 }
                        let content;
                        syn::braced!(content in meta.input);
                        let mut width: Option<f32> = None;
                        let mut height: Option<f32> = None;

                        while !content.is_empty() {
                            let field: syn::Ident = content.parse()?;
                            let _colon: syn::Token![:] = content.parse()?;
                            let value: syn::LitFloat = content.parse()?;

                            if field == "width" {
                                width = Some(value.base10_parse()?);
                            } else if field == "height" {
                                height = Some(value.base10_parse()?);
                            }

                            if content.peek(syn::Token![,]) {
                                let _comma: syn::Token![,] = content.parse()?;
                            }
                        }

                        size = Some(ModalSize::Proportional {
                            width: width.unwrap_or(0.5),
                            height: height.unwrap_or(0.5),
                        });
                    }
                    _ => {}
                }
            } else if meta.path.is_ident("position") {
                // Parse: position = Centered or position = At { x: 5, y: 3 }
                let _eq: syn::Token![=] = meta.input.parse()?;
                let ident: syn::Ident = meta.input.parse()?;

                match ident.to_string().as_str() {
                    "Centered" => position = Some(ModalPosition::Centered),
                    "At" => {
                        // Parse: At { x: N, y: N }
                        let content;
                        syn::braced!(content in meta.input);
                        let mut x: Option<u16> = None;
                        let mut y: Option<u16> = None;

                        while !content.is_empty() {
                            let field: syn::Ident = content.parse()?;
                            let _colon: syn::Token![:] = content.parse()?;
                            let value: syn::LitInt = content.parse()?;

                            if field == "x" {
                                x = Some(value.base10_parse()?);
                            } else if field == "y" {
                                y = Some(value.base10_parse()?);
                            }

                            if content.peek(syn::Token![,]) {
                                let _comma: syn::Token![,] = content.parse()?;
                            }
                        }

                        position = Some(ModalPosition::At {
                            x: x.unwrap_or(0),
                            y: y.unwrap_or(0),
                        });
                    }
                    _ => {}
                }
            }
            Ok(())
        });

        syn::parse::Parser::parse2(parser, attr)?;
        Ok(Self { size, position, pages })
    }
}

/// Generate size constant.
fn generate_size_const(size: &ModalSize) -> TokenStream {
    match size {
        ModalSize::Auto => quote! { rafter::ModalSize::Auto },
        ModalSize::Sm => quote! { rafter::ModalSize::Sm },
        ModalSize::Md => quote! { rafter::ModalSize::Md },
        ModalSize::Lg => quote! { rafter::ModalSize::Lg },
        ModalSize::Fixed { width, height } => {
            quote! { rafter::ModalSize::Fixed { width: #width, height: #height } }
        }
        ModalSize::Proportional { width, height } => {
            quote! { rafter::ModalSize::Proportional { width: #width, height: #height } }
        }
    }
}

/// Generate position constant.
fn generate_position_const(position: &ModalPosition) -> TokenStream {
    match position {
        ModalPosition::Centered => quote! { rafter::ModalPosition::Centered },
        ModalPosition::At { x, y } => quote! { rafter::ModalPosition::At { x: #x, y: #y } },
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
fn generate_default_impl(name: &Ident, fields: Option<&FieldsNamed>, attrs: &ModalAttrs) -> TokenStream {
    let page_field = if attrs.pages {
        quote! { __page: rafter::State::new(Page::default()), }
    } else {
        quote! {}
    };

    let Some(fields) = fields else {
        return quote! {
            impl Default for #name {
                fn default() -> Self {
                    Self {
                        #page_field
                        __handler_registry: rafter::HandlerRegistry::new(),
                    }
                }
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
                    #(#field_defaults),*,
                    #page_field
                    __handler_registry: rafter::HandlerRegistry::new(),
                }
            }
        }
    }
}

/// Generate Clone impl.
fn generate_clone_impl(name: &Ident, fields: Option<&FieldsNamed>, attrs: &ModalAttrs) -> TokenStream {
    let page_field = if attrs.pages {
        quote! { __page: self.__page.clone(), }
    } else {
        quote! {}
    };

    let Some(fields) = fields else {
        return quote! {
            impl Clone for #name {
                fn clone(&self) -> Self {
                    Self {
                        #page_field
                        __handler_registry: self.__handler_registry.clone(),
                    }
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
                    #(#field_clones),*,
                    #page_field
                    __handler_registry: self.__handler_registry.clone(),
                }
            }
        }
    }
}

/// Generate metadata module for #[modal_impl].
fn generate_metadata(
    name: &Ident,
    fields: Option<&FieldsNamed>,
    attrs: &ModalAttrs,
) -> TokenStream {
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

    // Generate size function - returns the configured size or default
    let size_fn = match &attrs.size {
        Some(s) => {
            let size = generate_size_const(s);
            quote! {
                pub fn size() -> rafter::ModalSize {
                    #size
                }
            }
        }
        None => {
            quote! {
                pub fn size() -> rafter::ModalSize {
                    rafter::ModalSize::Auto
                }
            }
        }
    };

    // Generate position function - returns the configured position or default
    let position_fn = match &attrs.position {
        Some(p) => {
            let pos = generate_position_const(p);
            quote! {
                pub fn position() -> rafter::ModalPosition {
                    #pos
                }
            }
        }
        None => {
            quote! {
                pub fn position() -> rafter::ModalPosition {
                    rafter::ModalPosition::Centered
                }
            }
        }
    };

    let has_pages = attrs.pages;

    // Include __page in dirty checking if pages is enabled
    let page_dirty = if attrs.pages {
        quote! { || modal.__page.is_dirty() }
    } else {
        quote! {}
    };

    let page_clear_dirty = if attrs.pages {
        quote! { modal.__page.clear_dirty(); }
    } else {
        quote! {}
    };

    quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub mod #metadata_name {
            use super::*;

            pub const HAS_PAGES: bool = #has_pages;

            #size_fn
            #position_fn

            pub fn is_dirty(modal: &#name) -> bool {
                false #(|| modal.#dirty_fields.is_dirty())* #page_dirty
            }

            pub fn clear_dirty(modal: &#name) {
                #(modal.#dirty_fields.clear_dirty();)*
                #page_clear_dirty
            }
        }
    }
}

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attrs = match ModalAttrs::parse(attr) {
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

    let default_impl = generate_default_impl(name, fields, &attrs);
    let clone_impl = generate_clone_impl(name, fields, &attrs);
    let metadata = generate_metadata(name, fields, &attrs);

    // Generate the __page field if pages is enabled
    let page_field = if attrs.pages {
        quote! {
            #[doc(hidden)]
            __page: rafter::State<Page>,
        }
    } else {
        quote! {}
    };

    if let Some(fields) = fields {
        let transformed_fields: Vec<_> = fields.named.iter().map(transform_field).collect();

        quote! {
            #(#other_attrs)*
            #vis struct #name {
                #(#transformed_fields),*,
                #page_field
                #[doc(hidden)]
                __handler_registry: rafter::HandlerRegistry,
            }

            #default_impl
            #clone_impl
            #metadata
        }
    } else {
        // Unit struct becomes struct with just __handler_registry (and __page if pages enabled)
        quote! {
            #(#other_attrs)*
            #vis struct #name {
                #page_field
                #[doc(hidden)]
                __handler_registry: rafter::HandlerRegistry,
            }

            #default_impl
            #clone_impl
            #metadata
        }
    }
}
