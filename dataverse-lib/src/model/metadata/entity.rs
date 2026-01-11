//! Entity metadata types

use serde::Deserialize;
use serde::Serialize;

use super::AttributeMetadata;
use super::ManyToManyRelationship;
use super::OneToManyRelationship;

/// Core entity metadata needed for CRUD operations.
///
/// This is the minimal metadata fetched on-demand when resolving
/// `Entity::Logical` to `Entity::Set` for API calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct EntityCore {
    /// The logical name of the entity (e.g., "account").
    pub logical_name: String,

    /// The entity set name used in Web API URLs (e.g., "accounts").
    pub entity_set_name: String,

    /// The schema name of the entity (e.g., "Account").
    pub schema_name: String,

    /// The logical name of the primary ID attribute (e.g., "accountid").
    pub primary_id_attribute: String,

    /// The logical name of the primary name attribute (e.g., "name").
    /// Some entities (like intersection tables) don't have a primary name attribute.
    #[serde(default)]
    pub primary_name_attribute: Option<String>,

    /// The entity type code (object type code).
    pub object_type_code: i32,
}

/// Full entity metadata including attributes and relationships.
///
/// This is fetched when full metadata is needed (e.g., for validation,
/// form building, or accessing attribute/relationship information).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct EntityMetadata {
    /// Core entity information (logical name, entity set name, etc.).
    #[serde(flatten)]
    pub core: EntityCore,

    /// The unique metadata identifier.
    pub metadata_id: uuid::Uuid,

    /// Display name of the entity.
    #[serde(default)]
    pub display_name: Label,

    /// Plural display name of the entity.
    #[serde(default)]
    pub display_collection_name: Label,

    /// Description of the entity.
    #[serde(default)]
    pub description: Label,

    /// The logical collection name.
    #[serde(default)]
    pub logical_collection_name: Option<String>,

    /// Whether this entity is a custom entity.
    #[serde(default)]
    pub is_custom_entity: bool,

    /// Whether this entity is an activity.
    #[serde(default)]
    pub is_activity: bool,

    /// Whether this entity can participate in activities.
    #[serde(default)]
    pub is_activity_party: bool,

    /// The ownership type (None, UserOwned, OrganizationOwned, etc.).
    #[serde(default)]
    pub ownership_type: OwnershipType,

    /// Whether this entity has notes enabled.
    #[serde(default)]
    pub has_notes: bool,

    /// Whether this entity has activities enabled.
    #[serde(default)]
    pub has_activities: bool,

    /// Whether change tracking is enabled.
    #[serde(default)]
    pub change_tracking_enabled: bool,

    /// Whether this entity is valid for advanced find.
    #[serde(default)]
    pub is_valid_for_advanced_find: bool,

    /// All attributes of this entity.
    #[serde(default)]
    pub attributes: Vec<AttributeMetadata>,

    /// One-to-many relationships where this entity is the primary (referenced) entity.
    #[serde(default)]
    pub one_to_many_relationships: Vec<OneToManyRelationship>,

    /// Many-to-one relationships where this entity is the related (referencing) entity.
    #[serde(default)]
    pub many_to_one_relationships: Vec<OneToManyRelationship>,

    /// Many-to-many relationships this entity participates in.
    #[serde(default)]
    pub many_to_many_relationships: Vec<ManyToManyRelationship>,
}

impl EntityMetadata {
    /// Returns a reference to the core entity metadata.
    pub fn core(&self) -> &EntityCore {
        &self.core
    }

    /// Returns the logical name of the entity.
    pub fn logical_name(&self) -> &str {
        &self.core.logical_name
    }

    /// Returns the entity set name for Web API URLs.
    pub fn entity_set_name(&self) -> &str {
        &self.core.entity_set_name
    }

    /// Returns the primary ID attribute name.
    pub fn primary_id_attribute(&self) -> &str {
        &self.core.primary_id_attribute
    }

    /// Returns the primary name attribute name, if any.
    pub fn primary_name_attribute(&self) -> Option<&str> {
        self.core.primary_name_attribute.as_deref()
    }

    /// Finds an attribute by logical name.
    pub fn attribute(&self, logical_name: &str) -> Option<&AttributeMetadata> {
        self.attributes
            .iter()
            .find(|a| a.logical_name == logical_name)
    }

    /// Finds a one-to-many relationship by schema name.
    pub fn one_to_many_relationship(&self, schema_name: &str) -> Option<&OneToManyRelationship> {
        self.one_to_many_relationships
            .iter()
            .find(|r| r.schema_name == schema_name)
    }

    /// Finds a many-to-one relationship by schema name.
    pub fn many_to_one_relationship(&self, schema_name: &str) -> Option<&OneToManyRelationship> {
        self.many_to_one_relationships
            .iter()
            .find(|r| r.schema_name == schema_name)
    }

    /// Finds a many-to-many relationship by schema name.
    pub fn many_to_many_relationship(&self, schema_name: &str) -> Option<&ManyToManyRelationship> {
        self.many_to_many_relationships
            .iter()
            .find(|r| r.schema_name == schema_name)
    }
}

/// A localized label with user-specific and all localized values.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Label {
    /// The label in the user's current language.
    #[serde(default)]
    pub user_localized_label: Option<LocalizedLabel>,

    /// All localized labels for different languages.
    #[serde(default)]
    pub localized_labels: Vec<LocalizedLabel>,
}

impl Label {
    /// Returns the label text in the user's language, or the first available label.
    pub fn text(&self) -> Option<&str> {
        self.user_localized_label
            .as_ref()
            .map(|l| l.label.as_str())
            .or_else(|| self.localized_labels.first().map(|l| l.label.as_str()))
    }

    /// Returns the label text, or a default value if not available.
    pub fn text_or<'a>(&'a self, default: &'a str) -> &'a str {
        self.text().unwrap_or(default)
    }
}

/// A label localized to a specific language.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LocalizedLabel {
    /// The localized text.
    pub label: String,

    /// The language code (LCID), e.g., 1033 for English.
    pub language_code: i32,
}

/// Ownership type of an entity.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OwnershipType {
    /// No ownership.
    #[default]
    None,
    /// Owned by a user or team.
    UserOwned,
    /// Owned by the organization (business unit).
    OrganizationOwned,
    /// Business-owned (parented).
    BusinessOwned,
    /// Business-parented.
    BusinessParented,
}
