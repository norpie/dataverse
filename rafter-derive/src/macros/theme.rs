use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Fields, parse2};

/// Expand the `#[theme]` attribute macro.
///
/// This macro transforms a struct with `Color` fields into a theme that implements
/// the `Theme` trait. It generates:
/// - A `resolve(&self, name: &str) -> Option<Color>` implementation
/// - A `color_names() -> Vec<&'static str>` implementation
/// - A `clone_box()` implementation for trait object cloning
///
/// # Example
///
/// ```rust,ignore
/// #[theme]
/// struct MyTheme {
///     primary: Color,
///     secondary: Color,
///     error: Color,
/// }
/// ```
///
/// Generates an impl of `Theme` that resolves "primary", "secondary", "error".
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

    // Collect field names for the resolve match arms
    let field_names: Vec<_> = fields.iter().filter_map(|f| f.ident.as_ref()).collect();

    let field_name_strs: Vec<String> = field_names.iter().map(|id| id.to_string()).collect();

    // Generate match arms for resolve()
    let resolve_arms: Vec<_> = field_names
        .iter()
        .zip(field_name_strs.iter())
        .map(|(field, name_str)| {
            quote! {
                #name_str => Some(self.#field.clone())
            }
        })
        .collect();

    // Generate the color_names list
    let color_names_list: Vec<_> = field_name_strs.iter().map(|s| quote! { #s }).collect();

    // Generate the Theme impl
    let expanded = quote! {
        #item

        impl rafter::theme::Theme for #name {
            fn resolve(&self, name: &str) -> Option<rafter::color::Color> {
                match name {
                    #(#resolve_arms,)*
                    _ => None,
                }
            }

            fn color_names(&self) -> Vec<&'static str> {
                vec![#(#color_names_list),*]
            }

            fn clone_box(&self) -> Box<dyn rafter::theme::Theme> {
                Box::new(self.clone())
            }
        }
    };

    expanded
}
