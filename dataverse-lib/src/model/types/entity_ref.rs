//! Entity reference types for lookups

use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

/// An entity reference from a read operation.
///
/// Contains metadata returned by Dataverse when reading lookup fields,
/// including the logical name and display name of the referenced record.
///
/// To use this reference in a write operation, convert it to an [`EntityBinding`]
/// using the [`bind`](Self::bind) method.
///
/// # Example
///
/// ```
/// use dataverse_lib::model::types::EntityReference;
/// use uuid::Uuid;
///
/// // Typically from a Dataverse response, not constructed manually
/// let contact_ref = EntityReference {
///     id: Uuid::new_v4(),
///     logical_name: "contact".to_string(),
///     name: Some("John Smith".to_string()),
/// };
///
/// // Convert to binding for writes
/// let binding = contact_ref.bind("contacts");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityReference {
    /// The unique identifier of the referenced record.
    pub id: Uuid,
    /// The logical name of the entity (e.g., "contact").
    pub logical_name: String,
    /// The display name of the referenced record, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl EntityReference {
    /// Creates a new entity reference.
    pub fn new(logical_name: impl Into<String>, id: Uuid) -> Self {
        Self {
            id,
            logical_name: logical_name.into(),
            name: None,
        }
    }

    /// Creates a new entity reference with a display name.
    pub fn with_name(logical_name: impl Into<String>, id: Uuid, name: impl Into<String>) -> Self {
        Self {
            id,
            logical_name: logical_name.into(),
            name: Some(name.into()),
        }
    }

    /// Converts this reference to an [`EntityBinding`] for write operations.
    ///
    /// You must provide the entity set name (e.g., "contacts" for the "contact" entity).
    pub fn bind(&self, set_name: impl Into<String>) -> EntityBinding {
        EntityBinding {
            id: self.id,
            set_name: set_name.into(),
        }
    }
}

/// An entity binding for write operations.
///
/// Used when setting lookup fields in create/update operations. Contains
/// the entity set name (required for the `@odata.bind` format) and record ID.
///
/// # Example
///
/// ```
/// use dataverse_lib::model::types::EntityBinding;
/// use uuid::Uuid;
///
/// // Create a binding to set a lookup field
/// let contact_binding = EntityBinding::new("contacts", Uuid::new_v4());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityBinding {
    /// The unique identifier of the record to bind to.
    pub id: Uuid,
    /// The entity set name (e.g., "contacts").
    pub set_name: String,
}

impl EntityBinding {
    /// Creates a new entity binding.
    pub fn new(set_name: impl Into<String>, id: Uuid) -> Self {
        Self {
            id,
            set_name: set_name.into(),
        }
    }

    /// Returns the OData bind path (e.g., "/contacts(abc-123)").
    pub fn odata_bind(&self) -> String {
        format!("/{}({})", self.set_name, self.id)
    }
}
