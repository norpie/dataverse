//! Multi-select picklist attribute metadata types
//!
//! Contains types for deserializing MultiSelectPicklistAttributeMetadata from the Dataverse API.
//! These are returned when querying:
//! `EntityDefinitions(...)/Attributes/Microsoft.Dynamics.CRM.MultiSelectPicklistAttributeMetadata?$expand=OptionSet`

use serde::Deserialize;
use serde::Serialize;

use super::attribute::OptionMetadata;
use super::attribute::RequiredLevel;
use super::entity::Label;

/// Metadata for a multi-select picklist (choices) attribute.
///
/// Multi-select picklist attributes allow selection of multiple values from a predefined set.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MultiSelectPicklistAttributeMetadata {
    /// The unique metadata identifier.
    pub metadata_id: uuid::Uuid,

    /// The logical name of the attribute.
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

    /// The option set containing multi-select picklist options.
    pub option_set: MultiSelectPicklistOptionSetMetadata,
}

/// Option set metadata for multi-select picklist attributes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MultiSelectPicklistOptionSetMetadata {
    /// The unique metadata identifier.
    #[serde(default)]
    pub metadata_id: Option<uuid::Uuid>,

    /// The name of the option set.
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

    /// The available options.
    #[serde(default)]
    pub options: Vec<OptionMetadata>,
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
    fn test_multi_select_option_set_metadata_bincode_roundtrip() {
        let option_set = MultiSelectPicklistOptionSetMetadata {
            metadata_id: Some(uuid::Uuid::new_v4()),
            name: Some("account_categories".to_string()),
            display_name: Label::default(),
            is_global: false,
            is_managed: true,
            options: vec![
                OptionMetadata {
                    value: 1,
                    label: Label::default(),
                    description: Label::default(),
                    color: None,
                    is_managed: true,
                },
                OptionMetadata {
                    value: 2,
                    label: Label::default(),
                    description: Label::default(),
                    color: None,
                    is_managed: true,
                },
            ],
        };

        let roundtripped = bincode_roundtrip(&option_set);
        assert_eq!(roundtripped.name, Some("account_categories".to_string()));
        assert_eq!(roundtripped.options.len(), 2);
    }
}
