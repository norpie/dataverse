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
    /// Option set type with available options.
    /// `options` may be empty if unknown (e.g., from a constant Value).
    OptionSet {
        kind: AttributeType,
        options: Vec<OptionInfo>,
    },
}

impl FieldType {
    /// Check if two field types are compatible.
    pub fn is_compatible_with(&self, other: &FieldType) -> bool {
        match (self, other) {
            (FieldType::Simple(a), FieldType::Simple(b)) => attr_types_compatible(a, b),
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
                if !attr_types_compatible(ka, kb) {
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
            (FieldType::OptionSet { kind: ka, .. }, FieldType::OptionSet { kind: kb, .. }) => {
                // All option set kinds are compatible with each other
                attr_types_compatible(ka, kb)
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
            FieldType::OptionSet { kind, .. } => format!("{:?}", kind),
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

    /// Returns the option set options, if this is an `OptionSet` variant.
    pub fn options(&self) -> Option<&[OptionInfo]> {
        match self {
            FieldType::OptionSet { options, .. } => Some(options),
            _ => None,
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
                options: vec![],
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
            let options = attr
                .options()
                .map(|os| {
                    os.options
                        .iter()
                        .map(|o| OptionInfo {
                            value: o.value,
                            label: o.label.text().unwrap_or("").to_string(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            FieldType::OptionSet {
                kind: attr.attribute_type,
                options,
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

    /// Convenience: create a `Known(OptionSet { kind, options })`.
    pub fn option_set(kind: AttributeType, options: Vec<OptionInfo>) -> Self {
        ValueType::Known(FieldType::OptionSet { kind, options })
    }

    /// Check if this type is compatible with expected input type.
    pub fn is_compatible_with(&self, expected: &ValueType) -> bool {
        match (self, expected) {
            // Any accepts anything
            (_, ValueType::Any) => true,

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

            // Any as actual value (shouldn't happen in practice)
            (ValueType::Any, _) => true,

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
            Value::OptionSet(_) => ValueType::option_set(AttributeType::Picklist, vec![]),
            Value::MultiOptionSet(_) => {
                ValueType::option_set(AttributeType::MultiSelectPicklist, vec![])
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

/// Check if two `AttributeType`s are compatible (same type or in same compatibility group).
fn attr_types_compatible(a: &AttributeType, b: &AttributeType) -> bool {
    if a == b {
        return true;
    }

    matches!(
        (a, b),
        // Integer types
        (AttributeType::Integer, AttributeType::BigInt)
            | (AttributeType::BigInt, AttributeType::Integer)
            // Decimal types
            | (AttributeType::Decimal, AttributeType::Double)
            | (AttributeType::Double, AttributeType::Decimal)
            // String types
            | (AttributeType::String, AttributeType::Memo)
            | (AttributeType::Memo, AttributeType::String)
            // Lookup types
            | (AttributeType::Lookup, AttributeType::Customer)
            | (AttributeType::Customer, AttributeType::Lookup)
            | (AttributeType::Lookup, AttributeType::Owner)
            | (AttributeType::Owner, AttributeType::Lookup)
            | (AttributeType::Customer, AttributeType::Owner)
            | (AttributeType::Owner, AttributeType::Customer)
            // OptionSet types
            | (AttributeType::Picklist, AttributeType::State)
            | (AttributeType::State, AttributeType::Picklist)
            | (AttributeType::Picklist, AttributeType::Status)
            | (AttributeType::Status, AttributeType::Picklist)
            | (AttributeType::State, AttributeType::Status)
            | (AttributeType::Status, AttributeType::State)
    )
}
