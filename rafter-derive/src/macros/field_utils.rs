//! Shared utilities for field transformation in app/modal macros.

use syn::{Attribute, Type};

/// Built-in widget types that manage their own state and shouldn't be wrapped in State<T>.
/// These are auto-detected by type name.
const BUILTIN_WIDGET_TYPES: &[&str] = &[
    "Button",
    "Checkbox",
    "Collapsible",
    "Input",
    "List",
    "RadioGroup",
    "ScrollArea",
    "Select",
    "Table",
    "Tree",
];

/// Check if a type is Resource<T>
pub fn is_resource_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        return segment.ident == "Resource";
    }
    false
}

/// Check if a type is a built-in widget type (Input, List<T>, Tree<T>, Button, etc.)
pub fn is_builtin_widget_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        return BUILTIN_WIDGET_TYPES.contains(&segment.ident.to_string().as_str());
    }
    false
}

/// Check if a field has the #[widget] attribute (for custom widgets)
pub fn has_widget_attribute(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident("widget"))
}

/// Check if a type is a widget type (built-in or marked with #[widget])
pub fn is_widget_type(ty: &Type, attrs: &[Attribute]) -> bool {
    is_builtin_widget_type(ty) || has_widget_attribute(attrs)
}
