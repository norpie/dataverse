//! Attribute metadata types

use serde::Deserialize;
use serde::Serialize;

use super::entity::Label;

/// Metadata for an entity attribute (column).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AttributeMetadata {
    /// The unique metadata identifier.
    pub metadata_id: uuid::Uuid,

    /// The logical name of the attribute (e.g., "name", "accountid").
    pub logical_name: String,

    /// The schema name of the attribute (e.g., "Name", "AccountId").
    pub schema_name: String,

    /// The attribute type.
    pub attribute_type: AttributeType,

    /// Display name of the attribute.
    #[serde(default)]
    pub display_name: Label,

    /// Description of the attribute.
    #[serde(default)]
    pub description: Label,

    /// The logical name of the parent entity.
    #[serde(default)]
    pub entity_logical_name: Option<String>,

    /// Whether this is the primary ID attribute.
    #[serde(default)]
    pub is_primary_id: bool,

    /// Whether this is the primary name attribute.
    #[serde(default)]
    pub is_primary_name: bool,

    /// Whether this is a custom attribute.
    #[serde(default)]
    pub is_custom_attribute: bool,

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

    /// The name of the attribute this extends (for calculated/rollup fields).
    #[serde(default)]
    pub attribute_of: Option<String>,

    /// Maximum length for string attributes.
    #[serde(default)]
    pub max_length: Option<i32>,

    /// Minimum value for numeric attributes.
    #[serde(default)]
    pub min_value: Option<f64>,

    /// Maximum value for numeric attributes.
    #[serde(default)]
    pub max_value: Option<f64>,

    /// Precision for decimal/money attributes.
    #[serde(default)]
    pub precision: Option<i32>,

    /// Date/time behavior for datetime attributes.
    #[serde(default)]
    pub date_time_behavior: Option<DateTimeBehavior>,

    /// Format for datetime attributes.
    #[serde(default)]
    pub format: Option<String>,

    /// For lookup attributes, the targets (entity logical names).
    #[serde(default)]
    pub targets: Vec<String>,

    /// For picklist attributes, the option set metadata.
    #[serde(default)]
    pub option_set: Option<OptionSetMetadata>,

    /// For global picklist attributes, the global option set metadata.
    #[serde(default)]
    pub global_option_set: Option<OptionSetMetadata>,
}

impl AttributeMetadata {
    /// Returns true if this is a lookup attribute.
    pub fn is_lookup(&self) -> bool {
        matches!(
            self.attribute_type,
            AttributeType::Lookup | AttributeType::Customer | AttributeType::Owner
        )
    }

    /// Returns true if this is a picklist (option set) attribute.
    pub fn is_picklist(&self) -> bool {
        matches!(
            self.attribute_type,
            AttributeType::Picklist | AttributeType::State | AttributeType::Status
        )
    }

    /// Returns the option set for this attribute (local or global).
    pub fn options(&self) -> Option<&OptionSetMetadata> {
        self.option_set
            .as_ref()
            .or(self.global_option_set.as_ref())
    }
}

/// Attribute type enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttributeType {
    /// Boolean (true/false).
    Boolean,
    /// Customer lookup (account or contact).
    Customer,
    /// Date and time.
    DateTime,
    /// Decimal number.
    Decimal,
    /// Double-precision floating point.
    Double,
    /// Integer.
    Integer,
    /// Lookup to another entity.
    Lookup,
    /// Multi-line text.
    Memo,
    /// Currency value.
    Money,
    /// Owner lookup (user or team).
    Owner,
    /// Party list (activity parties).
    PartyList,
    /// Option set (picklist).
    Picklist,
    /// State (statecode).
    State,
    /// Status (statuscode).
    Status,
    /// Single-line text.
    String,
    /// Unique identifier (GUID).
    Uniqueidentifier,
    /// Virtual attribute (computed).
    Virtual,
    /// Big integer.
    BigInt,
    /// Managed property.
    ManagedProperty,
    /// Entity name.
    EntityName,
    /// Image.
    Image,
    /// File.
    File,
    /// Multi-select option set.
    MultiSelectPicklist,
}

/// Required level for an attribute.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RequiredLevel {
    /// The required level value.
    #[serde(default)]
    pub value: RequiredLevelValue,
}

/// Required level value.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RequiredLevelValue {
    /// No requirement.
    #[default]
    None,
    /// System required (cannot be changed).
    SystemRequired,
    /// Application required (can be changed by user).
    ApplicationRequired,
    /// Recommended but not required.
    Recommended,
}

/// Date/time behavior for datetime attributes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DateTimeBehavior {
    /// The behavior value.
    pub value: DateTimeBehaviorValue,
}

/// Date/time behavior value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DateTimeBehaviorValue {
    /// User's local time.
    UserLocal,
    /// Date only (no time component).
    DateOnly,
    /// Time zone independent.
    TimeZoneIndependent,
}

/// Option set (picklist) metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct OptionSetMetadata {
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

    /// The type of option set.
    #[serde(default)]
    pub option_set_type: OptionSetType,

    /// The available options.
    #[serde(default)]
    pub options: Vec<OptionMetadata>,
}

/// Type of option set.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptionSetType {
    /// Standard picklist.
    #[default]
    Picklist,
    /// State option set.
    State,
    /// Status option set.
    Status,
    /// Boolean option set.
    Boolean,
}

/// Metadata for a single option in an option set.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct OptionMetadata {
    /// The integer value of the option.
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
}
