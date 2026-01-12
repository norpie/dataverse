//! Global option set metadata types

use serde::Deserialize;
use serde::Serialize;

use super::entity::Label;
use super::attribute::OptionMetadata;
use super::attribute::OptionSetType;

/// Metadata for a global option set.
///
/// Global option sets can be shared across multiple entities, unlike local
/// option sets which are defined per-attribute.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct GlobalOptionSetMetadata {
    /// The unique metadata identifier.
    pub metadata_id: uuid::Uuid,

    /// The name of the option set.
    pub name: String,

    /// Display name of the option set.
    #[serde(default)]
    pub display_name: Label,

    /// Description of the option set.
    #[serde(default)]
    pub description: Label,

    /// The type of option set.
    #[serde(default)]
    pub option_set_type: OptionSetType,

    /// Whether this is managed by a solution.
    #[serde(default)]
    pub is_managed: bool,

    /// Whether this option set is customizable.
    #[serde(default)]
    pub is_customizable: Option<BooleanManagedProperty>,

    /// The available options.
    #[serde(default)]
    pub options: Vec<OptionMetadata>,
}

/// A boolean property that may be managed by a solution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BooleanManagedProperty {
    /// The value of the property.
    pub value: bool,

    /// Whether this can be changed.
    #[serde(default)]
    pub can_be_changed: bool,

    /// The managed property logical name.
    #[serde(default)]
    pub managed_property_logical_name: Option<String>,
}
