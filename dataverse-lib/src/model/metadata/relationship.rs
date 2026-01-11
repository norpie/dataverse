//! Relationship metadata types

use serde::Deserialize;
use serde::Serialize;

/// Metadata for a one-to-many (or many-to-one) relationship.
///
/// This is used for both `OneToManyRelationships` and `ManyToOneRelationships`
/// collections on `EntityMetadata`. The perspective determines whether this
/// entity is the "one" or "many" side.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct OneToManyRelationship {
    /// The unique metadata identifier.
    pub metadata_id: uuid::Uuid,

    /// The schema name of the relationship (e.g., "contact_customer_accounts").
    pub schema_name: String,

    /// The logical name of the referenced (primary/"one" side) entity.
    pub referenced_entity: String,

    /// The logical name of the referencing (related/"many" side) entity.
    pub referencing_entity: String,

    /// The logical name of the lookup attribute on the referencing entity.
    pub referencing_attribute: String,

    /// The logical name of the primary key attribute on the referenced entity.
    #[serde(default)]
    pub referenced_attribute: Option<String>,

    /// The navigation property name on the referenced entity (collection).
    #[serde(default)]
    pub referenced_entity_navigation_property_name: Option<String>,

    /// The navigation property name on the referencing entity (single value).
    #[serde(default)]
    pub referencing_entity_navigation_property_name: Option<String>,

    /// Whether this is a custom relationship.
    #[serde(default)]
    pub is_custom_relationship: bool,

    /// Whether this relationship is part of a managed solution.
    #[serde(default)]
    pub is_managed: bool,

    /// The relationship type.
    #[serde(default)]
    pub relationship_type: RelationshipType,

    /// The cascade configuration for this relationship.
    #[serde(default)]
    pub cascade_configuration: Option<CascadeConfiguration>,
}

/// Metadata for a many-to-many relationship.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ManyToManyRelationship {
    /// The unique metadata identifier.
    pub metadata_id: uuid::Uuid,

    /// The schema name of the relationship.
    pub schema_name: String,

    /// The logical name of the first entity in the relationship.
    pub entity1_logical_name: String,

    /// The logical name of the second entity in the relationship.
    pub entity2_logical_name: String,

    /// The navigation property name on entity 1.
    #[serde(default)]
    pub entity1_navigation_property_name: Option<String>,

    /// The navigation property name on entity 2.
    #[serde(default)]
    pub entity2_navigation_property_name: Option<String>,

    /// The logical name of the intersect (junction) entity.
    pub intersect_entity_name: String,

    /// The attribute on entity 1 used for the relationship.
    #[serde(default)]
    pub entity1_intersect_attribute: Option<String>,

    /// The attribute on entity 2 used for the relationship.
    #[serde(default)]
    pub entity2_intersect_attribute: Option<String>,

    /// Whether this is a custom relationship.
    #[serde(default)]
    pub is_custom_relationship: bool,

    /// Whether this relationship is part of a managed solution.
    #[serde(default)]
    pub is_managed: bool,

    /// The relationship type.
    #[serde(default)]
    pub relationship_type: RelationshipType,
}

impl ManyToManyRelationship {
    /// Returns the navigation property name for associating/disassociating
    /// from the perspective of the given entity.
    pub fn navigation_property_for(&self, entity_logical_name: &str) -> Option<&str> {
        if entity_logical_name == self.entity1_logical_name {
            self.entity1_navigation_property_name.as_deref()
        } else if entity_logical_name == self.entity2_logical_name {
            self.entity2_navigation_property_name.as_deref()
        } else {
            None
        }
    }

    /// Returns the other entity in this relationship.
    pub fn other_entity(&self, entity_logical_name: &str) -> Option<&str> {
        if entity_logical_name == self.entity1_logical_name {
            Some(&self.entity2_logical_name)
        } else if entity_logical_name == self.entity2_logical_name {
            Some(&self.entity1_logical_name)
        } else {
            None
        }
    }
}

/// Type of relationship.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationshipType {
    /// One-to-many relationship.
    #[default]
    OneToManyRelationship,
    /// Many-to-many relationship.
    ManyToManyRelationship,
}

/// Cascade configuration for relationship operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CascadeConfiguration {
    /// Cascade behavior on assign.
    #[serde(default)]
    pub assign: CascadeType,

    /// Cascade behavior on delete.
    #[serde(default)]
    pub delete: CascadeType,

    /// Cascade behavior on merge.
    #[serde(default)]
    pub merge: CascadeType,

    /// Cascade behavior on reparent.
    #[serde(default)]
    pub reparent: CascadeType,

    /// Cascade behavior on share.
    #[serde(default)]
    pub share: CascadeType,

    /// Cascade behavior on unshare.
    #[serde(default)]
    pub unshare: CascadeType,
}

/// Cascade behavior type.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CascadeType {
    /// No cascade.
    #[default]
    NoCascade,
    /// Cascade to all related records.
    Cascade,
    /// Cascade only to active related records.
    Active,
    /// Cascade only to user-owned related records.
    UserOwned,
    /// Remove the link (for delete operations).
    RemoveLink,
    /// Restrict the operation if related records exist.
    Restrict,
}
