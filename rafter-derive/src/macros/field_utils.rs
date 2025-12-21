//! Shared utilities for field transformation in app/modal macros.

use syn::Type;

/// Component types that manage their own state and shouldn't be wrapped in State<T>
const COMPONENT_TYPES: &[&str] = &[
    "Checkbox",
    "Input",
    "List",
    "RadioGroup",
    "ScrollArea",
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

/// Check if a type is a component type (Input, List<T>, Tree<T>, etc.)
pub fn is_component_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        return COMPONENT_TYPES.contains(&segment.ident.to_string().as_str());
    }
    false
}
