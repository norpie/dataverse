//! Design-time value type for type tracking.

use super::metadata::AttributeType;
use super::Value;

/// Value type for design-time type tracking in transform chains.
///
/// Wraps `AttributeType` with additional variants for type inference.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ValueType {
    /// Known attribute type from metadata.
    Known(AttributeType),
    /// Accepts any type (for transform input signatures only).
    Any,
    /// Null value (compatible with any target type).
    #[default]
    Null,
    /// Multiple possible types (from coalesce/match branches).
    Union(Vec<AttributeType>),
}

impl ValueType {
    /// Check if this type is compatible with expected input type.
    pub fn is_compatible_with(&self, expected: &ValueType) -> bool {
        match (self, expected) {
            // Any accepts anything
            (_, ValueType::Any) => true,

            // Null is compatible with any known type
            (ValueType::Null, ValueType::Known(_)) => true,
            (ValueType::Null, ValueType::Union(_)) => true,

            // Known types must match
            (ValueType::Known(a), ValueType::Known(b)) => types_compatible(a, b),

            // Union is compatible if ANY member matches
            (ValueType::Union(types), ValueType::Known(b)) => {
                types.iter().any(|a| types_compatible(a, b))
            }
            (ValueType::Known(a), ValueType::Union(types)) => {
                types.iter().any(|b| types_compatible(a, b))
            }

            // Union to union: any overlap
            (ValueType::Union(a), ValueType::Union(b)) => {
                a.iter().any(|t| b.iter().any(|u| types_compatible(t, u)))
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
            ValueType::Known(attr) => format!("{:?}", attr),
            ValueType::Any => "Any".to_string(),
            ValueType::Null => "Null".to_string(),
            ValueType::Union(types) => {
                let names: Vec<_> = types.iter().map(|t| format!("{:?}", t)).collect();
                names.join(" | ")
            }
        }
    }
}

/// Check if two AttributeTypes are compatible.
fn types_compatible(a: &AttributeType, b: &AttributeType) -> bool {
    if a == b {
        return true;
    }

    // Group compatible types
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

impl From<AttributeType> for ValueType {
    fn from(attr: AttributeType) -> Self {
        ValueType::Known(attr)
    }
}

impl From<&Value> for ValueType {
    fn from(value: &Value) -> Self {
        match value {
            Value::Null => ValueType::Null,
            Value::Bool(_) => ValueType::Known(AttributeType::Boolean),
            Value::Int(_) => ValueType::Known(AttributeType::Integer),
            Value::Long(_) => ValueType::Known(AttributeType::BigInt),
            Value::Float(_) | Value::Decimal(_) => ValueType::Known(AttributeType::Decimal),
            Value::String(_) => ValueType::Known(AttributeType::String),
            Value::Guid(_) => ValueType::Known(AttributeType::Uniqueidentifier),
            Value::DateTime(_) => ValueType::Known(AttributeType::DateTime),
            Value::Money(_) => ValueType::Known(AttributeType::Money),
            Value::EntityReference(_) | Value::EntityBinding(_) => {
                ValueType::Known(AttributeType::Lookup)
            }
            Value::OptionSet(_) => ValueType::Known(AttributeType::Picklist),
            Value::MultiOptionSet(_) => ValueType::Known(AttributeType::MultiSelectPicklist),
            Value::File(_) => ValueType::Known(AttributeType::File),
            Value::Image(_) => ValueType::Known(AttributeType::Image),
            // Record/Records/Json don't have direct AttributeType mappings
            Value::Record(_) | Value::Records(_) | Value::Json(_) => ValueType::Any,
        }
    }
}
