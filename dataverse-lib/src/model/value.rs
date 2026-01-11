//! Value enum for dynamic field values

use chrono::DateTime;
use chrono::Utc;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

use super::types::EntityBinding;
use super::types::EntityReference;
use super::types::FileReference;
use super::types::ImageReference;
use super::types::Money;
use super::types::MultiSelectOptionSetValue;
use super::types::OptionSetValue;

/// A dynamic value that can hold any Dataverse field type.
///
/// This enum represents all possible values that can be stored in a Dataverse
/// field. It's used in [`Record`](super::Record) to store field values dynamically.
///
/// # Type Mapping
///
/// | Dataverse Type | Rust Variant |
/// |----------------|--------------|
/// | null | `Null` |
/// | Boolean | `Bool` |
/// | Integer | `Int` |
/// | BigInt | `Long` |
/// | Double | `Float` |
/// | Decimal | `Decimal` |
/// | String, Memo | `String` |
/// | UniqueIdentifier | `Guid` |
/// | DateTime | `DateTime` |
/// | Money | `Money` |
/// | Lookup (read) | `EntityReference` |
/// | Lookup (write) | `EntityBinding` |
/// | Picklist, State, Status | `OptionSet` |
/// | MultiSelectPicklist | `MultiOptionSet` |
///
/// # Example
///
/// ```
/// use dataverse_lib::model::Value;
///
/// let name = Value::from("Contoso");
/// let revenue = Value::from(1_000_000i64);
/// let active = Value::from(true);
/// let empty = Value::Null;
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    /// Null/empty value.
    Null,
    /// Boolean value.
    Bool(bool),
    /// 32-bit integer.
    Int(i32),
    /// 64-bit integer.
    Long(i64),
    /// 64-bit floating point.
    Float(f64),
    /// Arbitrary precision decimal.
    Decimal(Decimal),
    /// String value.
    String(String),
    /// GUID/UUID value.
    Guid(Uuid),
    /// Date and time with timezone.
    DateTime(DateTime<Utc>),
    /// Monetary value.
    Money(Money),
    /// Entity reference from a read operation.
    EntityReference(EntityReference),
    /// Entity binding for write operations.
    EntityBinding(EntityBinding),
    /// Single-select option set.
    OptionSet(OptionSetValue),
    /// Multi-select option set.
    MultiOptionSet(MultiSelectOptionSetValue),
    /// File reference.
    File(FileReference),
    /// Image reference.
    Image(ImageReference),
    /// Nested record (from expanded navigation property).
    Record(Box<super::Record>),
    /// Collection of records (from expanded collection navigation property).
    Records(Vec<super::Record>),
    /// Fallback for unrecognized JSON values.
    Json(serde_json::Value),
}

impl Value {
    /// Returns `true` if this is a null value.
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Returns the type name of this value.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Long(_) => "long",
            Value::Float(_) => "float",
            Value::Decimal(_) => "decimal",
            Value::String(_) => "string",
            Value::Guid(_) => "guid",
            Value::DateTime(_) => "datetime",
            Value::Money(_) => "money",
            Value::EntityReference(_) => "entity_reference",
            Value::EntityBinding(_) => "entity_binding",
            Value::OptionSet(_) => "option_set",
            Value::MultiOptionSet(_) => "multi_option_set",
            Value::File(_) => "file",
            Value::Image(_) => "image",
            Value::Record(_) => "record",
            Value::Records(_) => "records",
            Value::Json(_) => "json",
        }
    }
}

// =============================================================================
// From implementations
// =============================================================================

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Value::Int(v)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Long(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Float(v)
    }
}

impl From<Decimal> for Value {
    fn from(v: Decimal) -> Self {
        Value::Decimal(v)
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::String(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::String(v.to_string())
    }
}

impl From<Uuid> for Value {
    fn from(v: Uuid) -> Self {
        Value::Guid(v)
    }
}

impl From<DateTime<Utc>> for Value {
    fn from(v: DateTime<Utc>) -> Self {
        Value::DateTime(v)
    }
}

impl From<Money> for Value {
    fn from(v: Money) -> Self {
        Value::Money(v)
    }
}

impl From<EntityReference> for Value {
    fn from(v: EntityReference) -> Self {
        Value::EntityReference(v)
    }
}

impl From<EntityBinding> for Value {
    fn from(v: EntityBinding) -> Self {
        Value::EntityBinding(v)
    }
}

impl From<OptionSetValue> for Value {
    fn from(v: OptionSetValue) -> Self {
        Value::OptionSet(v)
    }
}

impl From<MultiSelectOptionSetValue> for Value {
    fn from(v: MultiSelectOptionSetValue) -> Self {
        Value::MultiOptionSet(v)
    }
}

impl From<FileReference> for Value {
    fn from(v: FileReference) -> Self {
        Value::File(v)
    }
}

impl From<ImageReference> for Value {
    fn from(v: ImageReference) -> Self {
        Value::Image(v)
    }
}

impl From<serde_json::Value> for Value {
    fn from(v: serde_json::Value) -> Self {
        Value::Json(v)
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(v: Option<T>) -> Self {
        match v {
            Some(inner) => inner.into(),
            None => Value::Null,
        }
    }
}

impl Default for Value {
    fn default() -> Self {
        Value::Null
    }
}
