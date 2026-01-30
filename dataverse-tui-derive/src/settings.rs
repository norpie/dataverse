//! Implementation of the `#[settings]` macro.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Field, Fields, FieldsNamed, Meta};

/// Expands the `#[settings]` macro.
pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    let name = &input.ident;
    let vis = &input.vis;

    // Extract fields
    let fields = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => fields,
            _ => {
                return syn::Error::new_spanned(
                    &input,
                    "#[settings] only supports structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "#[settings] only supports structs")
                .to_compile_error()
                .into();
        }
    };

    let expanded = generate_settings_struct(name, vis, fields);

    TokenStream::from(expanded)
}

/// Generate the transformed struct with Setting<T> fields and load method.
fn generate_settings_struct(
    name: &syn::Ident,
    vis: &syn::Visibility,
    fields: &FieldsNamed,
) -> TokenStream2 {
    let mut setting_fields = Vec::new();
    let mut load_stmts = Vec::new();

    for field in &fields.named {
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;
        let field_vis = &field.vis;

        // Extract default value from #[default = ...] attribute
        let default_expr = extract_default_attr(field);

        // Generate key name: "StructName.FieldName" in PascalCase
        let key = format!("{}.{}", name, to_pascal_case(&field_name.to_string()));

        // Filter out #[default = ...] and other non-standard attributes
        let filtered_attrs: Vec<_> = field
            .attrs
            .iter()
            .filter(|attr| !attr.path().is_ident("default") && !attr.path().is_ident("doc"))
            .collect();

        // Transform field to Setting<T>
        setting_fields.push(quote! {
            #(#filtered_attrs)*
            #field_vis #field_name: crate::settings::Setting<#field_type>
        });

        // Generate load statement for this field
        load_stmts.push(quote! {
            #field_name: crate::settings::Setting::load(
                backend.clone(),
                #key,
                #default_expr,
            ).await?
        });
    }

    quote! {
        #vis struct #name {
            #(#setting_fields),*
        }

        impl #name {
            /// Load all settings from the backend.
            pub async fn load(
                backend: std::sync::Arc<dyn crate::settings::SettingsBackend>,
            ) -> Result<Self, crate::settings::SettingsError> {
                Ok(Self {
                    #(#load_stmts),*
                })
            }
        }
    }
}

/// Extract the default value from #[default = ...] attribute.
fn extract_default_attr(field: &Field) -> TokenStream2 {
    for attr in &field.attrs {
        if attr.path().is_ident("default") {
            if let Meta::NameValue(nv) = &attr.meta {
                if let syn::Expr::Lit(expr_lit) = &nv.value {
                    // Return the literal as-is
                    let lit = &expr_lit.lit;
                    return quote! { #lit };
                } else {
                    // Return the expression as-is (e.g., Vec::new(), SomeType::default())
                    let expr = &nv.value;
                    return quote! { #expr };
                }
            }
        }
    }

    // If no default attribute found, use Default::default()
    quote! { Default::default() }
}

/// Convert snake_case to PascalCase.
fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("max_concurrency"), "MaxConcurrency");
        assert_eq!(to_pascal_case("is_paused"), "IsPaused");
        assert_eq!(to_pascal_case("check_interval_secs"), "CheckIntervalSecs");
    }
}
