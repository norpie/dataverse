//! Custom serialization for Record to handle both JSON (OData) and binary (bincode) formats.
//!
//! ## JSON Format (is_human_readable = true)
//!
//! ### Write Format (Serialization)
//!
//! When serializing a Record for create/update operations:
//! - Regular fields serialize normally: `"name": "Contoso"`
//! - EntityBinding serializes as: `"field@odata.bind": "/entities(guid)"`
//! - OptionSet serializes as just the value: `"statecode": 0`
//! - Money serializes as decimal: `"revenue": 1000000.00`
//!
//! ### Read Format (Deserialization)
//!
//! When deserializing from Dataverse responses:
//! - Lookup fields come as: `"_primarycontactid_value": "guid"`
//! - Lookup metadata: `"_field_value@Microsoft.Dynamics.CRM.lookuplogicalname": "contact"`
//! - Formatted values: `"field@OData.Community.Display.V1.FormattedValue": "Display Text"`
//! - ETag: `"@odata.etag": "W/\"12345\""`
//! - Expanded lookups: nested objects parsed as Records with entity from annotation
//!
//! ## Binary Format (is_human_readable = false)
//!
//! Uses a simple struct with all fields directly serialized.

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

use super::Entity;
use super::Record;
use super::Value;
use super::types::EntityReference;

// =============================================================================
// Binary format helper (for bincode)
// =============================================================================

/// Helper struct for binary serialization/deserialization.
#[derive(Serialize, Deserialize)]
struct BinaryRecord {
    entity: Entity,
    id: Option<Uuid>,
    fields: HashMap<String, Value>,
    formatted_values: HashMap<String, String>,
    etag: Option<String>,
}

// =============================================================================
// Serialization
// =============================================================================

impl Serialize for Record {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            // JSON format: OData-compatible (fields only, no metadata)
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
        } else {
            // Binary format: serialize all fields as a struct
            let binary = BinaryRecord {
                entity: self.entity.clone(),
                id: self.id,
                fields: self.fields.clone(),
                formatted_values: self.formatted_values.clone(),
                etag: self.etag.clone(),
            };
            binary.serialize(serializer)
        }
    }
}

// =============================================================================
// Deserialization
// =============================================================================

impl<'de> Deserialize<'de> for Record {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            // JSON format: use custom visitor for OData format
            deserializer.deserialize_map(RecordVisitor)
        } else {
            // Binary format: deserialize as simple struct
            let binary = BinaryRecord::deserialize(deserializer)?;
            Ok(Record {
                entity: binary.entity,
                id: binary.id,
                fields: binary.fields,
                formatted_values: binary.formatted_values,
                etag: binary.etag,
            })
        }
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
                // Lookup logical name annotation - key is like "_primarycontactid_value"
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
        // Process expanded objects first, then lookup values
        // This way expanded Records take precedence over EntityReferences
        let mut processed_lookups: HashMap<String, Value> = HashMap::new();

        for (key, json_value) in raw_fields {
            if key.starts_with('_') && key.ends_with("_value") {
                // This is a lookup field: _fieldname_value
                // Store for later - expanded object may override
                let clean_key = key[1..key.len() - 6].to_string();
                let lookup_key = key.clone();

                let value = match json_value {
                    serde_json::Value::Null => Value::Null,
                    serde_json::Value::String(guid_str) => {
                        // Try to parse as UUID
                        if let Ok(id) = Uuid::parse_str(&guid_str) {
                            let entity = lookup_logical_names
                                .get(&lookup_key)
                                .map(|s| Entity::Logical(s.clone()))
                                .unwrap_or_else(|| Entity::Logical(String::new()));
                            let display_name = formatted_values.get(&lookup_key).cloned();

                            let entity_ref = if let Some(name) = display_name {
                                EntityReference::with_name(entity, id, name)
                            } else {
                                EntityReference::new(entity, id)
                            };
                            Value::EntityReference(entity_ref)
                        } else {
                            // Not a valid UUID, treat as string
                            Value::String(guid_str)
                        }
                    }
                    other => json_value_to_value(other, &lookup_logical_names),
                };

                processed_lookups.insert(clean_key, value);
            } else if let serde_json::Value::Object(obj) = json_value {
                // This could be an expanded lookup - check for entity type
                // The entity type comes from _fieldname_value annotation
                // For polymorphic lookups, the key may be like "parentcustomerid_account"
                // but the annotation is on "_parentcustomerid_value"
                let entity = find_entity_for_expanded_key(&key, &lookup_logical_names);

                let nested_record = json_object_to_record(obj, entity, &lookup_logical_names);
                record
                    .fields
                    .insert(key.clone(), Value::Record(Box::new(nested_record)));

                // Store formatted value if available
                if let Some(formatted) = formatted_values.remove(&key) {
                    record.formatted_values.insert(key, formatted);
                }
            } else if let serde_json::Value::Array(arr) = json_value {
                // Could be a collection navigation property (expanded 1:N)
                let entity = find_entity_for_expanded_key(&key, &lookup_logical_names);

                let records: Vec<Record> = arr
                    .into_iter()
                    .filter_map(|v| {
                        if let serde_json::Value::Object(obj) = v {
                            Some(json_object_to_record(
                                obj,
                                entity.clone(),
                                &lookup_logical_names,
                            ))
                        } else {
                            None
                        }
                    })
                    .collect();

                if !records.is_empty() {
                    record.fields.insert(key.clone(), Value::Records(records));
                } else {
                    // Not a record array, keep as JSON
                    record.fields.insert(
                        key.clone(),
                        json_value_to_value(
                            serde_json::Value::Array(vec![]), // Empty, original consumed
                            &lookup_logical_names,
                        ),
                    );
                }

                if let Some(formatted) = formatted_values.remove(&key) {
                    record.formatted_values.insert(key, formatted);
                }
            } else {
                // Regular field
                let value = json_value_to_value(json_value, &lookup_logical_names);
                record.fields.insert(key.clone(), value);

                if let Some(formatted) = formatted_values.remove(&key) {
                    record.formatted_values.insert(key, formatted);
                }
            }
        }

        // Add lookup values that weren't overridden by expanded objects
        for (key, value) in processed_lookups {
            if !record.fields.contains_key(&key) {
                record.fields.insert(key, value);
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

/// Find the entity type for an expanded navigation property key.
///
/// For regular lookups, the key is the same as the field name (e.g., "primarycontactid")
/// and the annotation is on "_primarycontactid_value".
///
/// For polymorphic lookups, the key includes the target entity (e.g., "parentcustomerid_account")
/// and the annotation is on "_parentcustomerid_value".
fn find_entity_for_expanded_key(
    key: &str,
    lookup_logical_names: &HashMap<String, String>,
) -> Entity {
    // Try direct match first: _key_value
    let direct_key = format!("_{}_value", key);
    if let Some(logical_name) = lookup_logical_names.get(&direct_key) {
        return Entity::Logical(logical_name.clone());
    }

    // For polymorphic lookups, try stripping the entity suffix
    // e.g., "parentcustomerid_account" -> try "_parentcustomerid_value"
    if let Some(underscore_pos) = key.rfind('_') {
        let base_field = &key[..underscore_pos];
        let suffix = &key[underscore_pos + 1..];
        let base_key = format!("_{}_value", base_field);

        if let Some(logical_name) = lookup_logical_names.get(&base_key) {
            // Verify the suffix matches the logical name (they should be related)
            // The suffix is typically the entity set name or logical name
            return Entity::Logical(logical_name.clone());
        }

        // If the suffix itself looks like an entity name, use it
        if !suffix.is_empty() && suffix.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Entity::Logical(suffix.to_string());
        }
    }

    Entity::Logical(String::new())
}

/// Converts a JSON object to a Record with the given entity type.
fn json_object_to_record(
    obj: serde_json::Map<String, serde_json::Value>,
    entity: Entity,
    parent_lookup_names: &HashMap<String, String>,
) -> Record {
    // Re-parse the object through our normal deserialization
    // This handles nested annotations properly
    let json_value = serde_json::Value::Object(obj);
    let mut nested: Record = serde_json::from_value(json_value).unwrap_or_else(|_| Record::new(""));
    nested.entity = entity;
    nested
}

/// Converts a serde_json::Value to our Value enum.
fn json_value_to_value(
    json: serde_json::Value,
    lookup_logical_names: &HashMap<String, String>,
) -> Value {
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
            // Could be collection of records or other array
            Value::Json(serde_json::Value::Array(arr))
        }
        serde_json::Value::Object(obj) => {
            // Parse as a nested record with unknown entity type
            let nested =
                json_object_to_record(obj, Entity::Logical(String::new()), lookup_logical_names);
            Value::Record(Box::new(nested))
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
        assert_eq!(entity_ref.entity, Entity::logical("contact"));
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

    #[test]
    fn test_deserialize_expanded_lookup() {
        let json = r#"{
            "accountid": "12345678-1234-1234-1234-123456789012",
            "name": "Contoso",
            "_primarycontactid_value": "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
            "_primarycontactid_value@Microsoft.Dynamics.CRM.lookuplogicalname": "contact",
            "primarycontactid": {
                "contactid": "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
                "fullname": "John Smith"
            }
        }"#;
        let record: Record = serde_json::from_str(json).unwrap();

        // The expanded record should be accessible
        let contact = record.get_record("primarycontactid").unwrap();
        assert!(contact.is_some());
        let contact = contact.unwrap();

        // The expanded record should have the entity type from the annotation
        assert_eq!(contact.entity(), &Entity::logical("contact"));

        // Fields should be accessible
        assert_eq!(contact.get_string("fullname").unwrap(), Some("John Smith"));
    }

    #[test]
    fn test_deserialize_nested_expanded_lookups() {
        let json = r#"{
            "contactid": "11111111-1111-1111-1111-111111111111",
            "fullname": "John Smith",
            "_parentcustomerid_value": "22222222-2222-2222-2222-222222222222",
            "_parentcustomerid_value@Microsoft.Dynamics.CRM.lookuplogicalname": "account",
            "parentcustomerid_account": {
                "accountid": "22222222-2222-2222-2222-222222222222",
                "name": "Contoso",
                "_primarycontactid_value": "33333333-3333-3333-3333-333333333333",
                "_primarycontactid_value@Microsoft.Dynamics.CRM.lookuplogicalname": "contact",
                "primarycontactid": {
                    "contactid": "33333333-3333-3333-3333-333333333333",
                    "fullname": "Jane Doe"
                }
            }
        }"#;
        let record: Record = serde_json::from_str(json).unwrap();

        // Navigate through nested records
        let account = record
            .get_record("parentcustomerid_account")
            .unwrap()
            .unwrap();
        assert_eq!(account.entity(), &Entity::logical("account"));
        assert_eq!(account.get_string("name").unwrap(), Some("Contoso"));

        let nested_contact = account.get_record("primarycontactid").unwrap().unwrap();
        assert_eq!(nested_contact.entity(), &Entity::logical("contact"));
        assert_eq!(
            nested_contact.get_string("fullname").unwrap(),
            Some("Jane Doe")
        );
    }
}
