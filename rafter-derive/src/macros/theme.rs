//! Theme derive macro.
//!
//! Generates a `Theme` implementation for structs with Color fields.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse2, DeriveInput, Fields};

/// Expand the `#[theme]` attribute macro.
///
/// Transforms a struct with `Color` fields into a theme that implements
/// the `tuidom::Theme` trait.
///
/// # Example
///
/// ```rust,ignore
/// #[theme]
/// struct MyTheme {
///     primary: Color,
///     secondary: Color,
///     background: Color,
/// }
/// ```
///
/// Generates:
/// ```rust,ignore
/// impl tuidom::Theme for MyTheme {
///     fn resolve(&self, name: &str) -> Option<&tuidom::Color> {
///         match name {
///             "primary" => Some(&self.primary),
///             "secondary" => Some(&self.secondary),
///             "background" => Some(&self.background),
///             _ => None,
///         }
///     }
/// }
/// ```
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

    // Collect field names
    let field_names: Vec<_> = fields.iter().filter_map(|f| f.ident.as_ref()).collect();
    let field_name_strs: Vec<String> = field_names.iter().map(|id| id.to_string()).collect();

    // Generate match arms for resolve()
    let resolve_arms: Vec<_> = field_names
        .iter()
        .zip(field_name_strs.iter())
        .map(|(field, name_str)| {
            quote! {
                #name_str => Some(&self.#field)
            }
        })
        .collect();

    quote! {
        #item

        impl tuidom::Theme for #name {
            fn resolve(&self, name: &str) -> Option<&tuidom::Color> {
                match name {
                    #(#resolve_arms,)*
                    _ => None,
                }
            }
        }
    }
}
