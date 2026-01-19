//! Centralized formatting utilities for Dataverse data display.

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
