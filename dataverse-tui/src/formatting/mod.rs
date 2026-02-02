//! Centralized formatting utilities for Dataverse data display and parsing.

mod parse;

pub use parse::{ParseError, parse_bool, parse_datetime, parse_filter_value, string_to_value};

use dataverse_lib::model::Value;
use dataverse_lib::model::metadata::AttributeType;

/// Returns the default column width for a given attribute type.
pub fn default_column_width(attr_type: &AttributeType) -> u16 {
    match attr_type {
        AttributeType::Boolean => 8,
        AttributeType::Integer | AttributeType::BigInt => 12,
        AttributeType::Double | AttributeType::Decimal => 15,
        AttributeType::Money => 15,
        AttributeType::DateTime => 18,
        AttributeType::Uniqueidentifier => 36,
        AttributeType::Picklist | AttributeType::State | AttributeType::Status => 20,
        AttributeType::MultiSelectPicklist => 25,
        AttributeType::Lookup | AttributeType::Customer | AttributeType::Owner => 25,
        AttributeType::String | AttributeType::Memo => 25,
        _ => 20,
    }
}

/// Formatted value with both display and raw representations.
/// - display: Human-readable (e.g., "John Smith", "Yes", "Active")
/// - raw: Technical value (e.g., GUID for lookups, "true", status code)
#[derive(Clone, Debug, Default)]
pub struct FormattedValue {
    pub display: String,
    pub raw: String,
}

impl FormattedValue {
    pub fn new(display: impl Into<String>, raw: impl Into<String>) -> Self {
        Self {
            display: display.into(),
            raw: raw.into(),
        }
    }

    /// Create a FormattedValue where display and raw are the same.
    pub fn same(value: impl Into<String>) -> Self {
        let v = value.into();
        Self {
            display: v.clone(),
            raw: v,
        }
    }
}

/// Format a Dataverse Value for display, returning both display and raw values.
pub fn format_value(value: &Value) -> FormattedValue {
    match value {
        Value::Null => FormattedValue::default(),
        Value::Bool(b) => FormattedValue::new(if *b { "Yes" } else { "No" }, b.to_string()),
        Value::Int(n) => FormattedValue::same(n.to_string()),
        Value::Long(n) => FormattedValue::same(n.to_string()),
        Value::Float(n) => FormattedValue::same(format!("{:.2}", n)),
        Value::Decimal(d) => FormattedValue::same(d.to_string()),
        Value::String(s) => FormattedValue::same(s.clone()),
        Value::Guid(g) => FormattedValue::same(g.to_string()),
        Value::DateTime(dt) => FormattedValue::same(dt.format("%Y-%m-%d %H:%M").to_string()),
        Value::Money(m) => FormattedValue::same(format!("{}", m.value())),
        Value::EntityReference(r) => FormattedValue::new(
            r.name.clone().unwrap_or_else(|| r.id.to_string()),
            r.id.to_string(),
        ),
        Value::OptionSet(o) => FormattedValue::new(
            o.label.clone().unwrap_or_else(|| o.value.to_string()),
            o.value.to_string(),
        ),
        Value::MultiOptionSet(o) => FormattedValue::new(
            o.labels
                .as_ref()
                .map(|labels| labels.join(", "))
                .unwrap_or_else(|| {
                    o.values
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                }),
            o.values
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        ),
        Value::EntityBinding(b) => {
            FormattedValue::new(b.id.to_string(), format!("/{}({})", b.set_name, b.id))
        }
        Value::File(f) => FormattedValue::new(
            f.file_name.clone().unwrap_or_else(|| "[file]".to_string()),
            f.id.to_string(),
        ),
        Value::Image(i) => FormattedValue::new(
            i.file_name.clone().unwrap_or_else(|| "[image]".to_string()),
            i.id.to_string(),
        ),
        Value::Record(r) => FormattedValue::new(
            format!("[record: {}]", r.entity_name()),
            r.id()
                .map(|id| id.to_string())
                .unwrap_or_else(|| "[record]".to_string()),
        ),
        Value::Records(rs) => FormattedValue::same(format!("[{} records]", rs.len())),
        Value::Json(_) => FormattedValue::same("[json]".to_string()),
    }
}

/// Get placeholder hint text for a given attribute type.
///
/// Returns a short description of what value format is expected for input fields.
pub fn type_hint_text(attr_type: AttributeType) -> &'static str {
    match attr_type {
        AttributeType::String | AttributeType::Memo => "text value",
        AttributeType::Integer | AttributeType::BigInt => "integer",
        AttributeType::Double | AttributeType::Decimal | AttributeType::Money => "number",
        AttributeType::Boolean => "true or false",
        AttributeType::DateTime => "YYYY-MM-DD or RFC3339",
        AttributeType::Uniqueidentifier
        | AttributeType::Lookup
        | AttributeType::Customer
        | AttributeType::Owner => "guid",
        AttributeType::Picklist
        | AttributeType::State
        | AttributeType::Status
        | AttributeType::MultiSelectPicklist => "option value (integer)",
        _ => "value",
    }
}
