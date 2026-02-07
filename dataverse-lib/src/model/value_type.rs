//! Design-time value type for type tracking.

use super::metadata::AttributeMetadata;
use super::metadata::AttributeType;
use super::Value;

/// Lightweight option set value + label pair for design-time type tracking.
///
/// Derived from `OptionMetadata` but carries only what's needed for display
/// and mapping in the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionInfo {
    /// The integer value of the option.
    pub value: i32,
    /// The display label.
    pub label: String,
}

/// A concrete data type with enough info for compatibility checking.
///
/// Non-lookup types are `Simple(AttributeType)`. Lookup types carry target entity info
/// so that compatibility can check for overlapping targets. Option set types carry
/// the available options (value + label) for UI display and mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldType {
    /// Non-lookup, non-option-set type (String, Integer, DateTime, etc.)
    Simple(AttributeType),
    /// Lookup type with target entity info.
    /// `targets` may be empty if unknown (e.g., from a constant Value).
    Lookup {
        kind: AttributeType,
        targets: Vec<String>,
    },
    /// Option set type with name for compatibility checking.
    /// `name` identifies the option set (e.g., "statusreason").
    /// Empty name means unknown — treated as wildcard for compatibility.
    OptionSet { kind: AttributeType, name: String },
}

impl FieldType {
    /// Check if two field types are compatible.
    pub fn is_compatible_with(&self, other: &FieldType) -> bool {
        match (self, other) {
            (FieldType::Simple(a), FieldType::Simple(b)) => attr_type_assignable_to(a, b),
            (
                FieldType::Lookup {
                    kind: ka,
                    targets: ta,
                },
                FieldType::Lookup {
                    kind: kb,
                    targets: tb,
                },
            ) => {
                // Kinds must be in the same compatibility group
                if !attr_type_assignable_to(ka, kb) {
                    return false;
                }
                // If both have non-empty targets, they must overlap
                if !ta.is_empty() && !tb.is_empty() {
                    ta.iter().any(|t| tb.contains(t))
                } else {
                    // Unknown targets = assume compatible
                    true
                }
            }
            (
                FieldType::OptionSet {
                    kind: ka, name: na, ..
                },
                FieldType::OptionSet {
                    kind: kb, name: nb, ..
                },
            ) => {
                // Kinds must be in the same compatibility group
                if !attr_type_assignable_to(ka, kb) {
                    return false;
                }
                // If both have names, they must match
                if !na.is_empty() && !nb.is_empty() {
                    na == nb
                } else {
                    // Unknown name = assume compatible
                    true
                }
            }
            // Different categories (Simple vs Lookup vs OptionSet) are incompatible
            _ => false,
        }
    }

    /// Display string for UI.
    pub fn display(&self) -> String {
        match self {
            FieldType::Simple(attr) => format!("{:?}", attr),
            FieldType::Lookup { kind, targets } => {
                if targets.is_empty() {
                    format!("{:?}", kind)
                } else {
                    let target_list = targets.join(" | ");
                    format!("{:?}({})", kind, target_list)
                }
            }
            FieldType::OptionSet { kind, name, .. } => {
                if name.is_empty() {
                    format!("{:?}", kind)
                } else {
                    format!("{:?}({})", kind, name)
                }
            }
        }
    }

    /// Returns the underlying `AttributeType`.
    pub fn attribute_type(&self) -> AttributeType {
        match self {
            FieldType::Simple(attr) => *attr,
            FieldType::Lookup { kind, .. } => *kind,
            FieldType::OptionSet { kind, .. } => *kind,
        }
    }
}

impl From<AttributeType> for FieldType {
    fn from(attr: AttributeType) -> Self {
        if is_lookup_type(attr) {
            FieldType::Lookup {
                kind: attr,
                targets: vec![],
            }
        } else if is_option_set_type(attr) {
            FieldType::OptionSet {
                kind: attr,
                name: String::new(),
            }
        } else {
            FieldType::Simple(attr)
        }
    }
}

impl From<&AttributeMetadata> for FieldType {
    fn from(attr: &AttributeMetadata) -> Self {
        if is_lookup_type(attr.attribute_type) {
            FieldType::Lookup {
                kind: attr.attribute_type,
                targets: attr.targets.clone(),
            }
        } else if is_option_set_type(attr.attribute_type) {
            let name = attr
                .options()
                .and_then(|os| os.name.clone())
                .unwrap_or_default();
            FieldType::OptionSet {
                kind: attr.attribute_type,
                name,
            }
        } else {
            FieldType::Simple(attr.attribute_type)
        }
    }
}

/// Value type for design-time type tracking in transform chains.
///
/// Wraps `FieldType` with additional variants for type inference.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ValueType {
    /// Known field type from metadata.
    Known(FieldType),
    /// Accepts any type (for transform input signatures only).
    Any,
    /// Accepts any option set regardless of kind or name.
    /// Used by transforms like ValueMap that operate on any option set.
    AnyOptionSet,
    /// Null value (compatible with any target type).
    #[default]
    Null,
    /// Multiple possible types (from coalesce/match branches).
    Union(Vec<FieldType>),
}

impl ValueType {
    /// Convenience: create a `Known(Simple(attr))`.
    pub fn simple(attr: AttributeType) -> Self {
        ValueType::Known(FieldType::Simple(attr))
    }

    /// Convenience: create a `Known(Lookup { kind, targets })`.
    pub fn lookup(kind: AttributeType, targets: Vec<String>) -> Self {
        ValueType::Known(FieldType::Lookup { kind, targets })
    }

    /// Convenience: create a `Known(OptionSet { kind, name })`.
    pub fn option_set(kind: AttributeType, name: String) -> Self {
        ValueType::Known(FieldType::OptionSet { kind, name })
    }

    /// Check if this type is compatible with expected input type.
    ///
    /// Directional: `self` is the actual value type, `expected` is the slot type.
    /// "Can `self` flow into a slot expecting `expected`?"
    pub fn is_compatible_with(&self, expected: &ValueType) -> bool {
        match (self, expected) {
            // Any accepts anything
            (_, ValueType::Any) => true,

            // AnyOptionSet accepts any option set type
            (ValueType::Known(FieldType::OptionSet { .. }), ValueType::AnyOptionSet) => true,
            (ValueType::Union(types), ValueType::AnyOptionSet) => types
                .iter()
                .any(|t| matches!(t, FieldType::OptionSet { .. })),
            (ValueType::Null, ValueType::AnyOptionSet) => true,
            (_, ValueType::AnyOptionSet) => false,

            // Null is compatible with any known type
            (ValueType::Null, ValueType::Known(_)) => true,
            (ValueType::Null, ValueType::Union(_)) => true,

            // Known types must match
            (ValueType::Known(a), ValueType::Known(b)) => a.is_compatible_with(b),

            // Union is compatible if ANY member matches
            (ValueType::Union(types), ValueType::Known(b)) => {
                types.iter().any(|a| a.is_compatible_with(b))
            }
            (ValueType::Known(a), ValueType::Union(types)) => {
                types.iter().any(|b| a.is_compatible_with(b))
            }

            // Union to union: any overlap
            (ValueType::Union(a), ValueType::Union(b)) => {
                a.iter().any(|t| b.iter().any(|u| t.is_compatible_with(u)))
            }

            // Null-to-null
            (ValueType::Null, ValueType::Null) => true,

            // Any/AnyOptionSet as actual value (shouldn't happen in practice)
            (ValueType::Any, _) => true,
            (ValueType::AnyOptionSet, _) => true,

            // Known/Union to Null - not compatible (Null is a specific type)
            (ValueType::Known(_), ValueType::Null) => false,
            (ValueType::Union(_), ValueType::Null) => false,
        }
    }

    /// Display string for UI.
    pub fn display(&self) -> String {
        match self {
            ValueType::Known(ft) => ft.display(),
            ValueType::Any => "Any".to_string(),
            ValueType::AnyOptionSet => "OptionSet(any)".to_string(),
            ValueType::Null => "Null".to_string(),
            ValueType::Union(types) => {
                let names: Vec<_> = types.iter().map(|t| t.display()).collect();
                names.join(" | ")
            }
        }
    }
}

impl From<AttributeType> for ValueType {
    fn from(attr: AttributeType) -> Self {
        ValueType::Known(FieldType::from(attr))
    }
}

impl From<&Value> for ValueType {
    fn from(value: &Value) -> Self {
        match value {
            Value::Null => ValueType::Null,
            Value::Bool(_) => ValueType::simple(AttributeType::Boolean),
            Value::Int(_) => ValueType::simple(AttributeType::Integer),
            Value::Long(_) => ValueType::simple(AttributeType::BigInt),
            Value::Float(_) | Value::Decimal(_) => ValueType::simple(AttributeType::Decimal),
            Value::String(_) => ValueType::simple(AttributeType::String),
            Value::Guid(_) => ValueType::simple(AttributeType::Uniqueidentifier),
            Value::DateTime(_) => ValueType::simple(AttributeType::DateTime),
            Value::Money(_) => ValueType::simple(AttributeType::Money),
            Value::EntityReference(er) => {
                ValueType::lookup(AttributeType::Lookup, vec![er.entity.name().to_string()])
            }
            Value::EntityBinding(_) => ValueType::lookup(AttributeType::Lookup, vec![]),
            Value::OptionSet(_) => ValueType::option_set(AttributeType::Picklist, String::new()),
            Value::MultiOptionSet(_) => {
                ValueType::option_set(AttributeType::MultiSelectPicklist, String::new())
            }
            Value::File(_) => ValueType::simple(AttributeType::File),
            Value::Image(_) => ValueType::simple(AttributeType::Image),
            // Record/Records/Json don't have direct AttributeType mappings
            Value::Record(_) | Value::Records(_) | Value::Json(_) => ValueType::Any,
        }
    }
}

// =============================================================================
// Helpers
// =============================================================================

/// Check if an `AttributeType` is a lookup variant.
fn is_lookup_type(attr: AttributeType) -> bool {
    matches!(
        attr,
        AttributeType::Lookup | AttributeType::Customer | AttributeType::Owner
    )
}

/// Check if an `AttributeType` is an option set variant.
fn is_option_set_type(attr: AttributeType) -> bool {
    matches!(
        attr,
        AttributeType::Picklist
            | AttributeType::State
            | AttributeType::Status
            | AttributeType::MultiSelectPicklist
    )
}

/// Check if `from` type can be assigned to a slot expecting `to` (directional).
///
/// This is asymmetric for option set types:
/// - Single-select (Picklist/State/Status) → MultiSelectPicklist is OK (widening)
/// - MultiSelectPicklist → single-select is NOT OK (lossy)
fn attr_type_assignable_to(from: &AttributeType, to: &AttributeType) -> bool {
    if from == to {
        return true;
    }

    matches!(
        (from, to),
        // Integer types (bidirectional)
        (AttributeType::Integer, AttributeType::BigInt)
            | (AttributeType::BigInt, AttributeType::Integer)
            // Decimal types (bidirectional)
            | (AttributeType::Decimal, AttributeType::Double)
            | (AttributeType::Double, AttributeType::Decimal)
            // String types (bidirectional)
            | (AttributeType::String, AttributeType::Memo)
            | (AttributeType::Memo, AttributeType::String)
            // Lookup types (bidirectional)
            | (AttributeType::Lookup, AttributeType::Customer)
            | (AttributeType::Customer, AttributeType::Lookup)
            | (AttributeType::Lookup, AttributeType::Owner)
            | (AttributeType::Owner, AttributeType::Lookup)
            | (AttributeType::Customer, AttributeType::Owner)
            | (AttributeType::Owner, AttributeType::Customer)
            // Single-select option sets (bidirectional among themselves)
            | (AttributeType::Picklist, AttributeType::State)
            | (AttributeType::State, AttributeType::Picklist)
            | (AttributeType::Picklist, AttributeType::Status)
            | (AttributeType::Status, AttributeType::Picklist)
            | (AttributeType::State, AttributeType::Status)
            | (AttributeType::Status, AttributeType::State)
            // Single-select → MultiSelect OK (widening)
            | (AttributeType::Picklist, AttributeType::MultiSelectPicklist)
            | (AttributeType::State, AttributeType::MultiSelectPicklist)
            | (AttributeType::Status, AttributeType::MultiSelectPicklist) // MultiSelect → single-select is NOT listed (lossy, rejected)
    )
}
