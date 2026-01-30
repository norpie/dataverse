//! Value conversion utilities for import operations.

use chrono::{DateTime, NaiveDateTime, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

use dataverse_lib::model::metadata::AttributeType;
use dataverse_lib::model::types::{
    EntityBinding, Money, MultiSelectOptionSetValue, OptionSetValue,
};
use dataverse_lib::model::Value;

/// Error during string-to-value conversion.
#[derive(Debug, thiserror::Error)]
pub enum ConvertError {
    #[error("Invalid integer: {0}")]
    InvalidInt(String),

    #[error("Invalid number: {0}")]
    InvalidNumber(String),

    #[error("Invalid UUID: {0}")]
    InvalidUuid(String),

    #[error("Invalid boolean: {0}")]
    InvalidBool(String),

    #[error("Invalid datetime: {0}")]
    InvalidDateTime(String),

    #[error("Lookup requires target entity set name")]
    LookupMissingTarget,
}

/// Convert a string value to a Dataverse Value based on attribute type.
///
/// Returns `Value::Null` for empty/whitespace strings.
///
/// For lookups, `target_set_name` must be provided (entity set name from attribute metadata).
pub fn string_to_value(
    s: &str,
    attr_type: &AttributeType,
    target_set_name: Option<&str>,
) -> Result<Value, ConvertError> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(Value::Null);
    }

    match attr_type {
        AttributeType::String | AttributeType::Memo => Ok(Value::String(s.to_string())),

        AttributeType::Integer => {
            let n: i32 = s
                .parse()
                .map_err(|_| ConvertError::InvalidInt(s.to_string()))?;
            Ok(Value::Int(n))
        }

        AttributeType::BigInt => {
            let n: i64 = s
                .parse()
                .map_err(|_| ConvertError::InvalidInt(s.to_string()))?;
            Ok(Value::Long(n))
        }

        AttributeType::Double => {
            let n: f64 = s
                .parse()
                .map_err(|_| ConvertError::InvalidNumber(s.to_string()))?;
            Ok(Value::Float(n))
        }

        AttributeType::Decimal => {
            let d: Decimal = s
                .parse()
                .map_err(|_| ConvertError::InvalidNumber(s.to_string()))?;
            Ok(Value::Decimal(d))
        }

        AttributeType::Money => {
            let d: Decimal = s
                .parse()
                .map_err(|_| ConvertError::InvalidNumber(s.to_string()))?;
            Ok(Value::Money(Money::new(d)))
        }

        AttributeType::Boolean => {
            let b = parse_bool(s).ok_or_else(|| ConvertError::InvalidBool(s.to_string()))?;
            Ok(Value::Bool(b))
        }

        AttributeType::DateTime => {
            let dt =
                parse_datetime(s).ok_or_else(|| ConvertError::InvalidDateTime(s.to_string()))?;
            Ok(Value::DateTime(dt))
        }

        AttributeType::Uniqueidentifier => {
            let id: Uuid = s
                .parse()
                .map_err(|_| ConvertError::InvalidUuid(s.to_string()))?;
            Ok(Value::Guid(id))
        }

        AttributeType::Lookup | AttributeType::Customer | AttributeType::Owner => {
            let target = target_set_name.ok_or(ConvertError::LookupMissingTarget)?;
            let id: Uuid = s
                .parse()
                .map_err(|_| ConvertError::InvalidUuid(s.to_string()))?;
            Ok(Value::EntityBinding(EntityBinding::new(target, id)))
        }

        AttributeType::Picklist | AttributeType::State | AttributeType::Status => {
            let n: i32 = s
                .parse()
                .map_err(|_| ConvertError::InvalidInt(s.to_string()))?;
            Ok(Value::OptionSet(OptionSetValue {
                value: n,
                label: None,
            }))
        }

        AttributeType::MultiSelectPicklist => {
            let values: Result<Vec<i32>, _> =
                s.split(',').map(|part| part.trim().parse()).collect();
            let values = values.map_err(|_| ConvertError::InvalidInt(s.to_string()))?;
            Ok(Value::MultiOptionSet(MultiSelectOptionSetValue {
                values,
                labels: None,
            }))
        }

        // For unknown types, treat as string
        _ => Ok(Value::String(s.to_string())),
    }
}

/// Parse a boolean from various string representations.
fn parse_bool(s: &str) -> Option<bool> {
    match s.to_lowercase().as_str() {
        "true" | "1" | "yes" | "y" => Some(true),
        "false" | "0" | "no" | "n" => Some(false),
        _ => None,
    }
}

/// Parse a datetime from various string formats.
fn parse_datetime(s: &str) -> Option<DateTime<Utc>> {
    // Try ISO 8601 with timezone
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }

    // Try ISO 8601 without timezone (assume UTC)
    if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Some(naive.and_utc());
    }

    // Try ISO 8601 with milliseconds
    if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f") {
        return Some(naive.and_utc());
    }

    // Try date only (midnight UTC)
    if let Ok(naive) =
        NaiveDateTime::parse_from_str(&format!("{} 00:00:00", s), "%Y-%m-%d %H:%M:%S")
    {
        return Some(naive.and_utc());
    }

    // Try common datetime format
    if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(naive.and_utc());
    }

    None
}
