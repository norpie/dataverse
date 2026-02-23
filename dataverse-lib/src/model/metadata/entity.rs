//! Entity metadata types

use serde::Deserialize;
use serde::Serialize;

use std::collections::HashMap;
use std::collections::HashSet;

use super::AttributeMetadata;
use super::ManyToManyRelationship;
use super::MultiSelectPicklistAttributeMetadata;
use super::OneToManyRelationship;
use super::PicklistAttributeMetadata;
use super::StateAttributeMetadata;
use super::StatusAttributeMetadata;

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

    /// Whether this entity is an intersect (junction) entity for an N:N relationship.
    #[serde(default)]
    pub is_intersect: bool,
}

/// Full entity metadata including attributes and relationships.
///
/// This is fetched when full metadata is needed (e.g., for validation,
/// form building, or accessing attribute/relationship information).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct EntityMetadata {
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

    /// Whether this entity is an intersect (junction) entity for an N:N relationship.
    #[serde(default)]
    pub is_intersect: bool,

    /// All attributes of this entity (base metadata without option sets).
    #[serde(default)]
    pub attributes: Vec<AttributeMetadata>,

    /// State attributes with their option sets.
    /// Populated by parallel fetch, not directly from the API response.
    /// Cached via bincode; absent from API JSON (defaults to empty).
    #[serde(default)]
    pub state_attributes: Vec<StateAttributeMetadata>,

    /// Status attributes with their option sets.
    /// Populated by parallel fetch, not directly from the API response.
    /// Cached via bincode; absent from API JSON (defaults to empty).
    #[serde(default)]
    pub status_attributes: Vec<StatusAttributeMetadata>,

    /// Picklist attributes with their option sets.
    /// Populated by parallel fetch, not directly from the API response.
    /// Cached via bincode; absent from API JSON (defaults to empty).
    #[serde(default)]
    pub picklist_attributes: Vec<PicklistAttributeMetadata>,

    /// Multi-select picklist attributes with their option sets.
    /// Populated by parallel fetch, not directly from the API response.
    /// Cached via bincode; absent from API JSON (defaults to empty).
    #[serde(default)]
    pub multi_select_picklist_attributes: Vec<MultiSelectPicklistAttributeMetadata>,

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

/// Metadata needed for executing CRUD operations against an entity.
///
/// Contains the minimal information needed to build Create, Update, Delete,
/// Associate, and Disassociate operations for batch execution.
#[derive(Debug, Clone)]
pub struct ExecutionMetadata {
    /// The logical name of the entity (e.g., "account").
    pub logical_name: String,
    /// The entity set name for Web API URLs (e.g., "accounts").
    pub entity_set_name: String,
    /// The primary key attribute name (e.g., "accountid").
    pub primary_key: String,
    /// Logical names of all lookup attributes (Lookup, Customer, Owner).
    pub lookup_attributes: HashSet<String>,
    /// For each lookup attribute, the target entity logical names.
    pub lookup_targets: HashMap<String, Vec<String>>,
    /// Whether this entity is an intersect (junction) entity.
    pub is_intersect: bool,
    /// For junction entities: the N:N relationship this entity backs.
    pub junction_relationship: Option<ManyToManyRelationship>,
    /// The default statuscode value for active state (statecode=0).
    pub default_active_statuscode: i32,
    /// The default statuscode value for inactive state (statecode=1).
    pub default_inactive_statuscode: i32,
    /// Mapping from lookup logical attribute name to navigation property name.
    /// e.g., "nrq_countryid" → "nrq_CountryId"
    /// Used for `@odata.bind` serialization which requires the nav property name.
    pub lookup_nav_properties: HashMap<String, String>,
}

impl EntityMetadata {
    /// Returns a reference to the core entity metadata.
    pub fn core(&self) -> EntityCore {
        EntityCore {
            logical_name: self.logical_name.clone(),
            entity_set_name: self.entity_set_name.clone(),
            schema_name: self.schema_name.clone(),
            primary_id_attribute: self.primary_id_attribute.clone(),
            primary_name_attribute: self.primary_name_attribute.clone(),
            object_type_code: self.object_type_code,
            is_intersect: self.is_intersect,
        }
    }

    /// Returns the logical name of the entity.
    pub fn logical_name(&self) -> &str {
        &self.logical_name
    }

    /// Returns the entity set name for Web API URLs.
    pub fn entity_set_name(&self) -> &str {
        &self.entity_set_name
    }

    /// Returns the primary ID attribute name.
    pub fn primary_id_attribute(&self) -> &str {
        &self.primary_id_attribute
    }

    /// Returns the primary name attribute name, if any.
    pub fn primary_name_attribute(&self) -> Option<&str> {
        self.primary_name_attribute.as_deref()
    }

    /// Finds an attribute by logical name.
    pub fn attribute(&self, logical_name: &str) -> Option<&AttributeMetadata> {
        self.attributes
            .iter()
            .find(|a| a.logical_name == logical_name)
    }

    /// Finds a state attribute by logical name.
    pub fn state_attribute(&self, logical_name: &str) -> Option<&StateAttributeMetadata> {
        self.state_attributes
            .iter()
            .find(|a| a.logical_name == logical_name)
    }

    /// Finds a status attribute by logical name.
    pub fn status_attribute(&self, logical_name: &str) -> Option<&StatusAttributeMetadata> {
        self.status_attributes
            .iter()
            .find(|a| a.logical_name == logical_name)
    }

    /// Finds a picklist attribute by logical name.
    pub fn picklist_attribute(&self, logical_name: &str) -> Option<&PicklistAttributeMetadata> {
        self.picklist_attributes
            .iter()
            .find(|a| a.logical_name == logical_name)
    }

    /// Finds a multi-select picklist attribute by logical name.
    pub fn multi_select_picklist_attribute(
        &self,
        logical_name: &str,
    ) -> Option<&MultiSelectPicklistAttributeMetadata> {
        self.multi_select_picklist_attributes
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

    /// Returns execution metadata for building CRUD/associate operations.
    ///
    /// For junction entities, finds the N:N relationship by scanning
    /// `many_to_many_relationships` for one whose `intersect_entity_name`
    /// matches this entity's logical name.
    pub fn execution_metadata(&self) -> Result<ExecutionMetadata, String> {
        let lookup_attributes = self
            .attributes
            .iter()
            .filter(|a| a.is_lookup())
            .map(|a| a.logical_name.clone())
            .collect();

        let junction_relationship = if self.is_intersect {
            self.many_to_many_relationships
                .iter()
                .find(|r| r.intersect_entity_name == self.logical_name)
                .cloned()
        } else {
            None
        };

        // Find default active/inactive statuscodes from status attribute options.
        // Some system entities (e.g., systemuser, organization) have no status
        // attributes; default to 0 so execution_metadata() still succeeds —
        // these values are only used by Activate/Deactivate passes which won't
        // run against such entities.
        let (default_active_statuscode, default_inactive_statuscode) =
            match self.status_attributes.first() {
                Some(status_options) => {
                    let active = status_options
                        .option_set
                        .options
                        .iter()
                        .find(|o| o.state == 0)
                        .map(|o| o.value)
                        .unwrap_or(0);
                    let inactive = status_options
                        .option_set
                        .options
                        .iter()
                        .find(|o| o.state == 1)
                        .map(|o| o.value)
                        .unwrap_or(0);
                    (active, inactive)
                }
                None => (0, 0),
            };

        let lookup_targets: HashMap<String, Vec<String>> = self
            .attributes
            .iter()
            .filter(|a| a.is_lookup())
            .map(|a| (a.logical_name.clone(), a.targets.clone()))
            .collect();

        // Build lookup logical name → navigation property name mapping
        // from many-to-one relationships (the "referencing" side has the lookup).
        let lookup_nav_properties: HashMap<String, String> = self
            .many_to_one_relationships
            .iter()
            .filter_map(|r| {
                r.referencing_entity_navigation_property_name
                    .as_ref()
                    .map(|nav| (r.referencing_attribute.clone(), nav.clone()))
            })
            .collect();

        Ok(ExecutionMetadata {
            logical_name: self.logical_name.clone(),
            entity_set_name: self.entity_set_name.clone(),
            primary_key: self.primary_id_attribute.clone(),
            lookup_attributes,
            lookup_targets,
            is_intersect: self.is_intersect,
            junction_relationship,
            default_active_statuscode,
            default_inactive_statuscode,
            lookup_nav_properties,
        })
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
