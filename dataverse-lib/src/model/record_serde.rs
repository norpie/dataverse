//! Custom serialization for Record to handle Dataverse OData format.
//!
//! ## Write Format (Serialization)
//!
//! When serializing a Record for create/update operations:
//! - Regular fields serialize normally: `"name": "Contoso"`
//! - EntityBinding serializes as: `"field@odata.bind": "/entities(guid)"`
//! - OptionSet serializes as just the value: `"statecode": 0`
//! - Money serializes as decimal: `"revenue": 1000000.00`
//!
//! ## Read Format (Deserialization)
//!
//! When deserializing from Dataverse responses:
//! - Lookup fields come as: `"_primarycontactid_value": "guid"`
//! - Lookup metadata: `"_field_value@Microsoft.Dynamics.CRM.lookuplogicalname": "contact"`
//! - Formatted values: `"field@OData.Community.Display.V1.FormattedValue": "Display Text"`
//! - ETag: `"@odata.etag": "W/\"12345\""`

use std::collections::HashMap;
use std::fmt;

use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use serde::de::MapAccess;
use serde::de::Visitor;
use serde::ser::SerializeMap;
use uuid::Uuid;

use super::Record;
use super::Value;
use super::types::EntityReference;

// =============================================================================
// Serialization (for writes)
// =============================================================================

impl Serialize for Record {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Count fields: regular fields + EntityBinding (which serialize as @odata.bind)
        let mut map = serializer.serialize_map(Some(self.fields.len()))?;

        for (key, value) in &self.fields {
            match value {
                // EntityBinding serializes as "field@odata.bind": "/entities(guid)"
                Value::EntityBinding(binding) => {
                    let bind_key = format!("{}@odata.bind", key);
                    map.serialize_entry(&bind_key, &binding.odata_bind())?;
                }
                // OptionSet serializes as just the integer value
                Value::OptionSet(opt) => {
                    map.serialize_entry(key, &opt.value)?;
                }
                // MultiOptionSet serializes as comma-separated string
                Value::MultiOptionSet(opt) => {
                    let csv: String = opt
                        .values
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(",");
                    map.serialize_entry(key, &csv)?;
                }
                // Null values should not be serialized (Dataverse ignores them anyway)
                Value::Null => {
                    // Skip null values in serialization
                }
                // All other values serialize normally
                _ => {
                    map.serialize_entry(key, value)?;
                }
            }
        }

        map.end()
    }
}

// =============================================================================
// Deserialization (from reads)
// =============================================================================

impl<'de> Deserialize<'de> for Record {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(RecordVisitor)
    }
}

struct RecordVisitor;

impl<'de> Visitor<'de> for RecordVisitor {
    type Value = Record;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a map representing a Dataverse record")
    }

    fn visit_map<M>(self, mut map: M) -> Result<Record, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut record = Record::new("");
        let mut etag: Option<String> = None;
        let mut formatted_values: HashMap<String, String> = HashMap::new();
        let mut lookup_logical_names: HashMap<String, String> = HashMap::new();
        let mut raw_fields: HashMap<String, serde_json::Value> = HashMap::new();

        // First pass: collect all key-value pairs
        while let Some(key) = map.next_key::<String>()? {
            let value: serde_json::Value = map.next_value()?;

            if key == "@odata.etag" {
                if let serde_json::Value::String(s) = value {
                    etag = Some(s);
                }
            } else if key.contains("@OData.Community.Display.V1.FormattedValue") {
                // Extract the field name from the annotation
                let field_name = key
                    .strip_suffix("@OData.Community.Display.V1.FormattedValue")
                    .unwrap_or(&key);
                if let serde_json::Value::String(s) = value {
                    formatted_values.insert(field_name.to_string(), s);
                }
            } else if key.contains("@Microsoft.Dynamics.CRM.lookuplogicalname") {
                // Lookup logical name annotation
                let field_name = key
                    .strip_suffix("@Microsoft.Dynamics.CRM.lookuplogicalname")
                    .unwrap_or(&key);
                if let serde_json::Value::String(s) = value {
                    lookup_logical_names.insert(field_name.to_string(), s);
                }
            } else if key.contains("@Microsoft.Dynamics.CRM.associatednavigationproperty") {
                // Skip navigation property annotations
            } else if key.starts_with("@odata.") || key.starts_with("@Microsoft.") {
                // Skip other OData annotations
            } else {
                // Regular field
                raw_fields.insert(key, value);
            }
        }

        // Second pass: convert raw fields to typed Values
        for (key, json_value) in raw_fields {
            let value = if key.starts_with('_') && key.ends_with("_value") {
                // This is a lookup field: _fieldname_value
                let lookup_key = key.clone();

                match json_value {
                    serde_json::Value::Null => Value::Null,
                    serde_json::Value::String(guid_str) => {
                        // Try to parse as UUID
                        if let Ok(id) = Uuid::parse_str(&guid_str) {
                            let logical_name = lookup_logical_names
                                .get(&lookup_key)
                                .cloned()
                                .unwrap_or_default();
                            let display_name = formatted_values.get(&lookup_key).cloned();

                            let entity_ref = if let Some(name) = display_name {
                                EntityReference::with_name(&logical_name, id, name)
                            } else {
                                EntityReference::new(&logical_name, id)
                            };
                            Value::EntityReference(entity_ref)
                        } else {
                            // Not a valid UUID, treat as string
                            Value::String(guid_str)
                        }
                    }
                    other => json_value_to_value(other),
                }
            } else {
                // Regular field
                json_value_to_value(json_value)
            };

            // For lookup fields, use the clean name without underscore prefix/suffix
            let clean_key = if key.starts_with('_') && key.ends_with("_value") {
                key[1..key.len() - 6].to_string()
            } else {
                key
            };

            record.fields.insert(clean_key.clone(), value);

            // Store formatted value if available (for non-lookup fields)
            if let Some(formatted) = formatted_values.remove(&clean_key) {
                record.formatted_values.insert(clean_key, formatted);
            }
        }

        // Set etag
        if let Some(e) = etag {
            record.etag = Some(e);
        }

        // Copy remaining formatted values (for lookup fields the key was the raw _field_value)
        for (key, value) in formatted_values {
            // Try to map _field_value formatted values to clean field names
            if key.starts_with('_') && key.ends_with("_value") {
                let clean = key[1..key.len() - 6].to_string();
                record.formatted_values.insert(clean, value);
            } else {
                record.formatted_values.insert(key, value);
            }
        }

        Ok(record)
    }
}

/// Converts a serde_json::Value to our Value enum.
fn json_value_to_value(json: serde_json::Value) -> Value {
    match json {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
                    Value::Int(i as i32)
                } else {
                    Value::Long(i)
                }
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Json(serde_json::Value::Number(n))
            }
        }
        serde_json::Value::String(s) => {
            // Try to parse as UUID
            if let Ok(uuid) = Uuid::parse_str(&s) {
                Value::Guid(uuid)
            }
            // Try to parse as DateTime (ISO 8601)
            else if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&s) {
                Value::DateTime(dt.with_timezone(&chrono::Utc))
            }
            // Otherwise keep as string
            else {
                Value::String(s)
            }
        }
        serde_json::Value::Array(arr) => {
            // Could be multi-select option set (as array of ints) or nested records
            // For now, treat as JSON
            Value::Json(serde_json::Value::Array(arr))
        }
        serde_json::Value::Object(obj) => {
            // Could be a nested record from $expand
            // For now, treat as JSON - we can enhance this later
            Value::Json(serde_json::Value::Object(obj))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::types::EntityBinding;
    use crate::model::types::OptionSetValue;

    #[test]
    fn test_serialize_simple_fields() {
        let record = Record::new("account")
            .set("name", "Contoso")
            .set("revenue", 1_000_000i64);

        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("\"name\":\"Contoso\""));
        assert!(json.contains("\"revenue\":1000000"));
    }

    #[test]
    fn test_serialize_entity_binding() {
        let id = Uuid::parse_str("12345678-1234-1234-1234-123456789012").unwrap();
        let binding = EntityBinding::new("contacts", id);

        let record = Record::new("account").set("primarycontactid", binding);

        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains(
            "\"primarycontactid@odata.bind\":\"/contacts(12345678-1234-1234-1234-123456789012)\""
        ));
    }

    #[test]
    fn test_serialize_option_set() {
        let record = Record::new("account").set("statecode", OptionSetValue::new(0));

        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("\"statecode\":0"));
    }

    #[test]
    fn test_deserialize_simple_fields() {
        let json = r#"{"name": "Contoso", "revenue": 1000000}"#;
        let record: Record = serde_json::from_str(json).unwrap();

        assert_eq!(record.get_string("name").unwrap(), Some("Contoso"));
        assert_eq!(record.get_long("revenue").unwrap(), Some(1_000_000));
    }

    #[test]
    fn test_deserialize_with_etag() {
        let json = r#"{"@odata.etag": "W/\"12345\"", "name": "Contoso"}"#;
        let record: Record = serde_json::from_str(json).unwrap();

        assert_eq!(record.etag(), Some("W/\"12345\""));
        assert_eq!(record.get_string("name").unwrap(), Some("Contoso"));
    }

    #[test]
    fn test_deserialize_lookup_field() {
        let json = r#"{
            "_primarycontactid_value": "12345678-1234-1234-1234-123456789012",
            "_primarycontactid_value@Microsoft.Dynamics.CRM.lookuplogicalname": "contact",
            "_primarycontactid_value@OData.Community.Display.V1.FormattedValue": "John Smith"
        }"#;
        let record: Record = serde_json::from_str(json).unwrap();

        let entity_ref = record.get_entity_reference("primarycontactid").unwrap();
        assert!(entity_ref.is_some());
        let entity_ref = entity_ref.unwrap();
        assert_eq!(entity_ref.logical_name, "contact");
        assert_eq!(entity_ref.name, Some("John Smith".to_string()));
    }

    #[test]
    fn test_deserialize_formatted_value() {
        let json = r#"{
            "revenue": 1000000.00,
            "revenue@OData.Community.Display.V1.FormattedValue": "$1,000,000.00"
        }"#;
        let record: Record = serde_json::from_str(json).unwrap();

        assert_eq!(record.get_formatted("revenue"), Some("$1,000,000.00"));
    }
}
