//! Theme derive macro.
//!
//! Generates a `Theme` implementation for structs with Color fields.
//! Supports nested groups via the `#[group]` attribute.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse2, DeriveInput, Fields};

/// Check if a field has the `#[group]` attribute.
fn has_group_attr(field: &syn::Field) -> bool {
    field.attrs.iter().any(|attr| attr.path().is_ident("group"))
}

/// Expand the `#[theme]` attribute macro.
///
/// Transforms a struct with `Color` fields into a theme that implements
/// the `tuidom::Theme` trait. Supports nested groups via `#[group]`.
///
/// # Example
///
/// ```rust,ignore
/// #[theme]
/// struct ButtonColors {
///     normal: Color,
///     hover: Color,
/// }
///
/// #[theme]
/// struct MyTheme {
///     primary: Color,
///     #[group]
///     button: ButtonColors,
/// }
/// ```
///
/// Resolves: "primary", "button.normal", "button.hover"
pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input: DeriveInput = match parse2(item.clone()) {
        Ok(input) => input,
        Err(e) => return e.to_compile_error(),
    };

    let name = &input.ident;

    // Extract fields from struct
    let fields = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return syn::Error::new_spanned(
                    &input,
                    "#[theme] only supports structs with named fields",
                )
                .to_compile_error();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "#[theme] only supports structs")
                .to_compile_error();
        }
    };

    // Separate regular fields from group fields
    let mut color_fields = Vec::new();
    let mut group_fields = Vec::new();

    for field in fields.iter() {
        if let Some(ident) = &field.ident {
            if has_group_attr(field) {
                group_fields.push(ident);
            } else {
                color_fields.push(ident);
            }
        }
    }

    // Generate match arms for direct color fields
    let color_arms: Vec<_> = color_fields
        .iter()
        .map(|field| {
            let name_str = field.to_string();
            quote! {
                #name_str => return Some(&self.#field)
            }
        })
        .collect();

    // Generate group resolution blocks
    let group_blocks: Vec<_> = group_fields
        .iter()
        .map(|field| {
            let prefix = format!("{}.", field);
            quote! {
                if let Some(rest) = name.strip_prefix(#prefix) {
                    if let Some(color) = self.#field.__theme_resolve(rest) {
                        return Some(color);
                    }
                }
            }
        })
        .collect();

    // Strip #[group] attributes from the output
    let mut output_item = input.clone();
    if let syn::Data::Struct(ref mut data) = output_item.data {
        if let Fields::Named(ref mut fields) = data.fields {
            for field in fields.named.iter_mut() {
                field.attrs.retain(|attr| !attr.path().is_ident("group"));
            }
        }
    }

    quote! {
        #output_item

        impl #name {
            /// Resolve a color by name within this theme/group.
            /// Used internally for nested group resolution.
            #[doc(hidden)]
            pub fn __theme_resolve(&self, name: &str) -> Option<&tuidom::Color> {
                // Try direct color fields
                match name {
                    #(#color_arms,)*
                    _ => {}
                }
                // Try group fields
                #(#group_blocks)*
                None
            }
        }

        impl tuidom::Theme for #name {
            fn resolve(&self, name: &str) -> Option<&tuidom::Color> {
                self.__theme_resolve(name)
            }
        }
    }
}
