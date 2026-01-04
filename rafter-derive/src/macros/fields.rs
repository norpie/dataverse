//! Shared utilities for field transformation in app/modal/system macros.

use syn::{Attribute, Type};

/// Check if a type is Resource<T>.
pub fn is_resource_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return segment.ident == "Resource";
        }
    }
    false
}

/// Check if a field has the #[widget] attribute.
pub fn has_widget_attribute(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident("widget"))
}

/// Check if a field has #[state(skip)] attribute.
pub fn has_state_skip(attrs: &[Attribute]) -> bool {
    for attr in attrs {
        if attr.path().is_ident("state") {
            if let syn::Meta::List(list) = &attr.meta {
                let mut skip = false;
                let _ = list.parse_nested_meta(|meta| {
                    if meta.path.is_ident("skip") {
                        skip = true;
                    }
                    Ok(())
                });
                if skip {
                    return true;
                }
            }
        }
    }
    false
}
