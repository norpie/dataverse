//! Centralized formatting utilities for Dataverse data display.

use dataverse_lib::model::metadata::AttributeType;
use dataverse_lib::model::Value;

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

/// Format a Dataverse Value for display.
pub fn format_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(b) => if *b { "Yes" } else { "No" }.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Long(n) => n.to_string(),
        Value::Float(n) => format!("{:.2}", n),
        Value::Decimal(d) => d.to_string(),
        Value::String(s) => s.clone(),
        Value::Guid(g) => g.to_string(),
        Value::DateTime(dt) => dt.format("%Y-%m-%d %H:%M").to_string(),
        Value::Money(m) => format!("{}", m.value()),
        Value::EntityReference(r) => r.name.clone().unwrap_or_else(|| r.id.to_string()),
        Value::OptionSet(o) => o.label.clone().unwrap_or_else(|| o.value.to_string()),
        Value::MultiOptionSet(o) => o
            .labels
            .as_ref()
            .map(|labels| labels.join(", "))
            .unwrap_or_else(|| {
                o.values
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            }),
        _ => "[complex]".to_string(),
    }
}
