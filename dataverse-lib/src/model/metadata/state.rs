//! State attribute metadata types
//!
//! Contains types for deserializing StateAttributeMetadata from the Dataverse API.
//! These are returned when querying:
//! `EntityDefinitions(...)/Attributes/Microsoft.Dynamics.CRM.StateAttributeMetadata?$expand=OptionSet`

use serde::Deserialize;
use serde::Serialize;

use super::attribute::RequiredLevel;
use super::entity::Label;

/// Metadata for a state (statecode) attribute.
///
/// State attributes control the lifecycle of a record (Active, Inactive, etc.)
/// and determine which status values are valid.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct StateAttributeMetadata {
    /// The unique metadata identifier.
    pub metadata_id: uuid::Uuid,

    /// The logical name of the attribute (typically "statecode").
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

    /// The default form value (default state).
    /// `None` when not defined for the entity.
    #[serde(default)]
    pub default_form_value: Option<i32>,

    /// The option set containing state options.
    pub option_set: StateOptionSetMetadata,
}

/// Option set metadata for state attributes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct StateOptionSetMetadata {
    /// The unique metadata identifier.
    #[serde(default)]
    pub metadata_id: Option<uuid::Uuid>,

    /// The name of the option set (e.g., "account_statecode").
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

    /// The available state options.
    #[serde(default)]
    pub options: Vec<StateOptionMetadata>,
}

/// Metadata for a single state option.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct StateOptionMetadata {
    /// The integer value of the state (e.g., 0 for Active, 1 for Inactive).
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

    /// The default status value for this state.
    /// When a record enters this state, this is the default status assigned.
    pub default_status: i32,

    /// Language-independent name for the state (e.g., "Active", "Inactive").
    #[serde(default)]
    pub invariant_name: Option<String>,
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
    fn test_state_option_metadata_bincode_roundtrip() {
        let option = StateOptionMetadata {
            value: 0,
            label: Label::default(),
            description: Label::default(),
            color: None,
            is_managed: true,
            default_status: 1,
            invariant_name: Some("Active".to_string()),
        };

        let roundtripped = bincode_roundtrip(&option);
        assert_eq!(roundtripped.value, 0);
        assert_eq!(roundtripped.default_status, 1);
        assert_eq!(roundtripped.invariant_name, Some("Active".to_string()));
    }

    #[test]
    fn test_state_option_set_metadata_bincode_roundtrip() {
        let option_set = StateOptionSetMetadata {
            metadata_id: Some(uuid::Uuid::new_v4()),
            name: Some("account_statecode".to_string()),
            display_name: Label::default(),
            is_global: false,
            is_managed: true,
            options: vec![
                StateOptionMetadata {
                    value: 0,
                    label: Label::default(),
                    description: Label::default(),
                    color: None,
                    is_managed: true,
                    default_status: 1,
                    invariant_name: Some("Active".to_string()),
                },
                StateOptionMetadata {
                    value: 1,
                    label: Label::default(),
                    description: Label::default(),
                    color: None,
                    is_managed: true,
                    default_status: 2,
                    invariant_name: Some("Inactive".to_string()),
                },
            ],
        };

        let roundtripped = bincode_roundtrip(&option_set);
        assert_eq!(roundtripped.name, Some("account_statecode".to_string()));
        assert_eq!(roundtripped.options.len(), 2);
        assert_eq!(roundtripped.options[0].default_status, 1);
        assert_eq!(roundtripped.options[1].default_status, 2);
    }
}
