//! Custom serialization for Value to support both human-readable (JSON) and
//! binary (bincode) formats.
//!
//! The problem: `#[serde(untagged)]` works for JSON (serde infers variants from
//! data structure) but fails for binary formats like bincode (no structural info
//! in the byte stream to distinguish variants).
//!
//! Solution: use `Serializer::is_human_readable()` / `Deserializer::is_human_readable()`
//! to branch between untagged (JSON-compatible) and tagged (bincode-compatible)
//! representations.

use chrono::DateTime;
use chrono::Utc;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use uuid::Uuid;

use super::types::EntityBinding;
use super::types::EntityReference;
use super::types::FileReference;
use super::types::ImageReference;
use super::types::Money;
use super::types::MultiSelectOptionSetValue;
use super::types::OptionSetValue;
use super::Record;
use super::Value;

// =============================================================================
// Serialization helpers
// =============================================================================

/// Helper enum for binary serialization (externally tagged, derives Serialize).
/// Uses references to avoid cloning during serialization.
#[derive(Serialize)]
enum BinaryRef<'a> {
    Null,
    Bool(bool),
    Int(i32),
    Long(i64),
    Float(f64),
    Decimal(&'a Decimal),
    String(&'a str),
    Guid(&'a Uuid),
    DateTime(&'a DateTime<Utc>),
    Money(&'a Money),
    EntityReference(&'a EntityReference),
    EntityBinding(&'a EntityBinding),
    OptionSet(&'a OptionSetValue),
    MultiOptionSet(&'a MultiSelectOptionSetValue),
    File(&'a FileReference),
    Image(&'a ImageReference),
    Record(&'a Record),
    Records(&'a Vec<Record>),
    Json(&'a serde_json::Value),
}

impl Serialize for Value {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            // Untagged: serialize inner value directly (JSON-compatible)
            match self {
                Value::Null => serializer.serialize_unit(),
                Value::Bool(v) => serializer.serialize_bool(*v),
                Value::Int(v) => serializer.serialize_i32(*v),
                Value::Long(v) => serializer.serialize_i64(*v),
                Value::Float(v) => serializer.serialize_f64(*v),
                Value::Decimal(v) => Serialize::serialize(v, serializer),
                Value::String(v) => serializer.serialize_str(v),
                Value::Guid(v) => v.serialize(serializer),
                Value::DateTime(v) => v.serialize(serializer),
                Value::Money(v) => v.serialize(serializer),
                Value::EntityReference(v) => v.serialize(serializer),
                Value::EntityBinding(v) => v.serialize(serializer),
                Value::OptionSet(v) => v.serialize(serializer),
                Value::MultiOptionSet(v) => v.serialize(serializer),
                Value::File(v) => v.serialize(serializer),
                Value::Image(v) => v.serialize(serializer),
                Value::Record(v) => v.serialize(serializer),
                Value::Records(v) => v.serialize(serializer),
                Value::Json(v) => v.serialize(serializer),
            }
        } else {
            // Tagged: delegate to BinaryRef which derives standard enum serialization
            let binary_ref = match self {
                Value::Null => BinaryRef::Null,
                Value::Bool(v) => BinaryRef::Bool(*v),
                Value::Int(v) => BinaryRef::Int(*v),
                Value::Long(v) => BinaryRef::Long(*v),
                Value::Float(v) => BinaryRef::Float(*v),
                Value::Decimal(v) => BinaryRef::Decimal(v),
                Value::String(v) => BinaryRef::String(v),
                Value::Guid(v) => BinaryRef::Guid(v),
                Value::DateTime(v) => BinaryRef::DateTime(v),
                Value::Money(v) => BinaryRef::Money(v),
                Value::EntityReference(v) => BinaryRef::EntityReference(v),
                Value::EntityBinding(v) => BinaryRef::EntityBinding(v),
                Value::OptionSet(v) => BinaryRef::OptionSet(v),
                Value::MultiOptionSet(v) => BinaryRef::MultiOptionSet(v),
                Value::File(v) => BinaryRef::File(v),
                Value::Image(v) => BinaryRef::Image(v),
                Value::Record(v) => BinaryRef::Record(v),
                Value::Records(v) => BinaryRef::Records(v),
                Value::Json(v) => BinaryRef::Json(v),
            };
            binary_ref.serialize(serializer)
        }
    }
}

// =============================================================================
// Deserialization
// =============================================================================

/// Helper enum for human-readable deserialization (untagged, tries variants in order).
#[derive(Deserialize)]
#[serde(untagged)]
enum HumanReadable {
    Null,
    Bool(bool),
    Int(i32),
    Long(i64),
    Float(f64),
    Decimal(Decimal),
    String(String),
    Guid(Uuid),
    DateTime(DateTime<Utc>),
    Money(Money),
    EntityReference(EntityReference),
    EntityBinding(EntityBinding),
    OptionSet(OptionSetValue),
    MultiOptionSet(MultiSelectOptionSetValue),
    File(FileReference),
    Image(ImageReference),
    Record(Box<Record>),
    Records(Vec<Record>),
    Json(serde_json::Value),
}

/// Helper enum for binary deserialization (externally tagged with variant index).
#[derive(Deserialize)]
enum Binary {
    Null,
    Bool(bool),
    Int(i32),
    Long(i64),
    Float(f64),
    Decimal(Decimal),
    String(String),
    Guid(Uuid),
    DateTime(DateTime<Utc>),
    Money(Money),
    EntityReference(EntityReference),
    EntityBinding(EntityBinding),
    OptionSet(OptionSetValue),
    MultiOptionSet(MultiSelectOptionSetValue),
    File(FileReference),
    Image(ImageReference),
    Record(Box<Record>),
    Records(Vec<Record>),
    Json(serde_json::Value),
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if deserializer.is_human_readable() {
            HumanReadable::deserialize(deserializer).map(|hr| match hr {
                HumanReadable::Null => Value::Null,
                HumanReadable::Bool(v) => Value::Bool(v),
                HumanReadable::Int(v) => Value::Int(v),
                HumanReadable::Long(v) => Value::Long(v),
                HumanReadable::Float(v) => Value::Float(v),
                HumanReadable::Decimal(v) => Value::Decimal(v),
                HumanReadable::String(v) => Value::String(v),
                HumanReadable::Guid(v) => Value::Guid(v),
                HumanReadable::DateTime(v) => Value::DateTime(v),
                HumanReadable::Money(v) => Value::Money(v),
                HumanReadable::EntityReference(v) => Value::EntityReference(v),
                HumanReadable::EntityBinding(v) => Value::EntityBinding(v),
                HumanReadable::OptionSet(v) => Value::OptionSet(v),
                HumanReadable::MultiOptionSet(v) => Value::MultiOptionSet(v),
                HumanReadable::File(v) => Value::File(v),
                HumanReadable::Image(v) => Value::Image(v),
                HumanReadable::Record(v) => Value::Record(v),
                HumanReadable::Records(v) => Value::Records(v),
                HumanReadable::Json(v) => Value::Json(v),
            })
        } else {
            Binary::deserialize(deserializer).map(|b| match b {
                Binary::Null => Value::Null,
                Binary::Bool(v) => Value::Bool(v),
                Binary::Int(v) => Value::Int(v),
                Binary::Long(v) => Value::Long(v),
                Binary::Float(v) => Value::Float(v),
                Binary::Decimal(v) => Value::Decimal(v),
                Binary::String(v) => Value::String(v),
                Binary::Guid(v) => Value::Guid(v),
                Binary::DateTime(v) => Value::DateTime(v),
                Binary::Money(v) => Value::Money(v),
                Binary::EntityReference(v) => Value::EntityReference(v),
                Binary::EntityBinding(v) => Value::EntityBinding(v),
                Binary::OptionSet(v) => Value::OptionSet(v),
                Binary::MultiOptionSet(v) => Value::MultiOptionSet(v),
                Binary::File(v) => Value::File(v),
                Binary::Image(v) => Value::Image(v),
                Binary::Record(v) => Value::Record(v),
                Binary::Records(v) => Value::Records(v),
                Binary::Json(v) => Value::Json(v),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BINCODE_CONFIG: bincode::config::Configuration = bincode::config::standard();

    fn bincode_roundtrip(value: &Value) -> Value {
        let bytes = bincode::serde::encode_to_vec(value, BINCODE_CONFIG).unwrap();
        let (deserialized, _): (Value, _) =
            bincode::serde::decode_from_slice(&bytes, BINCODE_CONFIG).unwrap();
        deserialized
    }

    #[test]
    fn test_json_roundtrip_null() {
        let value = Value::Null;
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, "null");
        let deserialized: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, Value::Null);
    }

    #[test]
    fn test_json_roundtrip_bool() {
        let value = Value::Bool(true);
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, "true");
        let deserialized: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, Value::Bool(true));
    }

    #[test]
    fn test_json_roundtrip_int() {
        let value = Value::Int(42);
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, "42");
        let deserialized: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, Value::Int(42));
    }

    #[test]
    fn test_json_roundtrip_string() {
        let value = Value::String("hello".to_string());
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, "\"hello\"");
        let deserialized: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, Value::String("hello".to_string()));
    }

    #[test]
    fn test_bincode_roundtrip_null() {
        assert_eq!(bincode_roundtrip(&Value::Null), Value::Null);
    }

    #[test]
    fn test_bincode_roundtrip_bool() {
        assert_eq!(bincode_roundtrip(&Value::Bool(true)), Value::Bool(true));
    }

    #[test]
    fn test_bincode_roundtrip_int() {
        assert_eq!(bincode_roundtrip(&Value::Int(42)), Value::Int(42));
    }

    #[test]
    fn test_bincode_roundtrip_long() {
        assert_eq!(
            bincode_roundtrip(&Value::Long(i64::MAX)),
            Value::Long(i64::MAX)
        );
    }

    #[test]
    fn test_bincode_roundtrip_float() {
        assert_eq!(bincode_roundtrip(&Value::Float(3.14)), Value::Float(3.14));
    }

    #[test]
    fn test_bincode_roundtrip_string() {
        let value = Value::String("hello world".to_string());
        assert_eq!(bincode_roundtrip(&value), value);
    }

    #[test]
    fn test_bincode_roundtrip_guid() {
        let uuid = Uuid::new_v4();
        let value = Value::Guid(uuid);
        assert_eq!(bincode_roundtrip(&value), value);
    }

    #[test]
    fn test_bincode_roundtrip_option_set() {
        let value = Value::OptionSet(OptionSetValue::new(5));
        assert_eq!(bincode_roundtrip(&value), value);
    }

    #[test]
    fn test_bincode_roundtrip_entity_reference() {
        let id = Uuid::new_v4();
        let value = Value::EntityReference(EntityReference::new("account", id));
        assert_eq!(bincode_roundtrip(&value), value);
    }

    #[test]
    fn test_bincode_different_variants_produce_different_bytes() {
        let int_val = Value::Int(1);
        let long_val = Value::Long(1);
        let int_bytes = bincode::serde::encode_to_vec(&int_val, BINCODE_CONFIG).unwrap();
        let long_bytes = bincode::serde::encode_to_vec(&long_val, BINCODE_CONFIG).unwrap();
        // Tagged format means different discriminants = different bytes
        assert_ne!(int_bytes, long_bytes);
    }
}
