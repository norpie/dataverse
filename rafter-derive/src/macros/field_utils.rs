//! Shared utilities for field transformation in app/modal macros.

use syn::Type;

/// Widget types that manage their own state and shouldn't be wrapped in State<T>
const WIDGET_TYPES: &[&str] = &["Input", "List", "Tree", "Table", "Scrollable"];

/// Check if a type is Resource<T>
pub fn is_resource_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        return segment.ident == "Resource";
    }
    false
}

/// Check if a type is a widget type (Input, List<T>, Tree<T>, etc.)
pub fn is_widget_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        return WIDGET_TYPES.contains(&segment.ident.to_string().as_str());
    }
    false
}


