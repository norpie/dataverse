//! Status attribute metadata types
//!
//! Contains types for deserializing StatusAttributeMetadata from the Dataverse API.
//! These are returned when querying:
//! `EntityDefinitions(...)/Attributes/Microsoft.Dynamics.CRM.StatusAttributeMetadata?$expand=OptionSet`

use serde::Deserialize;
use serde::Serialize;

use super::attribute::RequiredLevel;
use super::entity::Label;

/// Metadata for a status (statuscode) attribute.
///
/// Status attributes provide the reason for a record's state and are linked
/// to valid state values.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct StatusAttributeMetadata {
    /// The unique metadata identifier.
    pub metadata_id: uuid::Uuid,

    /// The logical name of the attribute (typically "statuscode").
    pub logical_name: String,

    /// The schema name of the attribute.
    pub schema_name: String,

    /// Display name of the attribute.
    #[serde(default)]
    pub display_name: Label,

    /// Description of the attribute.
    #[serde(default)]
    pub description: Label,

    /// The logical name of the parent entity.
    #[serde(default)]
    pub entity_logical_name: Option<String>,

    /// Whether this attribute is valid for create operations.
    #[serde(default)]
    pub is_valid_for_create: bool,

    /// Whether this attribute is valid for read operations.
    #[serde(default)]
    pub is_valid_for_read: bool,

    /// Whether this attribute is valid for update operations.
    #[serde(default)]
    pub is_valid_for_update: bool,

    /// Whether this attribute is required.
    #[serde(default)]
    pub required_level: RequiredLevel,

    /// The default form value (-1 means use the state's default status).
    #[serde(default)]
    pub default_form_value: i32,

    /// The option set containing status options.
    pub option_set: StatusOptionSetMetadata,
}

/// Option set metadata for status attributes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct StatusOptionSetMetadata {
    /// The unique metadata identifier.
    #[serde(default)]
    pub metadata_id: Option<uuid::Uuid>,

    /// The name of the option set (e.g., "account_statuscode").
    #[serde(default)]
    pub name: Option<String>,

    /// Display name of the option set.
    #[serde(default)]
    pub display_name: Label,

    /// Whether this is a global option set.
    #[serde(default)]
    pub is_global: bool,

    /// Whether this option set is managed.
    #[serde(default)]
    pub is_managed: bool,

    /// The available status options.
    #[serde(default)]
    pub options: Vec<StatusOptionMetadata>,
}

/// Metadata for a single status option.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct StatusOptionMetadata {
    /// The integer value of the status.
    pub value: i32,

    /// The display label.
    #[serde(default)]
    pub label: Label,

    /// Description of the option.
    #[serde(default)]
    pub description: Label,

    /// Color associated with the option (hex code).
    #[serde(default)]
    pub color: Option<String>,

    /// Whether this option is part of a managed solution.
    #[serde(default)]
    pub is_managed: bool,

    /// The state value this status belongs to.
    /// A status is only valid when the record is in this state.
    pub state: i32,

    /// JSON string describing allowed status transitions.
    /// Empty string if no transition rules are defined.
    #[serde(default)]
    pub transition_data: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    const BINCODE_CONFIG: bincode::config::Configuration = bincode::config::standard();

    fn bincode_roundtrip<T>(value: &T) -> T
    where
        T: serde::Serialize + serde::de::DeserializeOwned,
    {
        let bytes = bincode::serde::encode_to_vec(value, BINCODE_CONFIG).unwrap();
        let (deserialized, _): (T, _) =
            bincode::serde::decode_from_slice(&bytes, BINCODE_CONFIG).unwrap();
        deserialized
    }

    #[test]
    fn test_status_option_metadata_bincode_roundtrip() {
        let option = StatusOptionMetadata {
            value: 1,
            label: Label::default(),
            description: Label::default(),
            color: None,
            is_managed: true,
            state: 0,
            transition_data: String::new(),
        };

        let roundtripped = bincode_roundtrip(&option);
        assert_eq!(roundtripped.value, 1);
        assert_eq!(roundtripped.state, 0);
        assert_eq!(roundtripped.transition_data, "");
    }

    #[test]
    fn test_status_option_set_metadata_bincode_roundtrip() {
        let option_set = StatusOptionSetMetadata {
            metadata_id: Some(uuid::Uuid::new_v4()),
            name: Some("account_statuscode".to_string()),
            display_name: Label::default(),
            is_global: false,
            is_managed: true,
            options: vec![
                StatusOptionMetadata {
                    value: 1,
                    label: Label::default(),
                    description: Label::default(),
                    color: None,
                    is_managed: true,
                    state: 0,
                    transition_data: String::new(),
                },
                StatusOptionMetadata {
                    value: 2,
                    label: Label::default(),
                    description: Label::default(),
                    color: None,
                    is_managed: true,
                    state: 1,
                    transition_data: String::new(),
                },
            ],
        };

        let roundtripped = bincode_roundtrip(&option_set);
        assert_eq!(roundtripped.name, Some("account_statuscode".to_string()));
        assert_eq!(roundtripped.options.len(), 2);
        assert_eq!(roundtripped.options[0].state, 0);
        assert_eq!(roundtripped.options[1].state, 1);
    }
}
