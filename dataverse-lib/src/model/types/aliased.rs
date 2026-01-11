//! AliasedValue type for FetchXML aliases

use serde::Deserialize;
use serde::Serialize;

use crate::model::Value;

/// A value with alias metadata from a FetchXML query.
///
/// When using aliases in FetchXML queries (especially with linked entities),
/// Dataverse returns additional metadata about which entity and attribute
/// the value came from.
///
/// # Example
///
/// FetchXML with alias:
/// ```xml
/// <link-entity name="contact" alias="c">
///   <attribute name="fullname" alias="contact_name" />
/// </link-entity>
/// ```
///
/// The response includes:
/// ```json
/// {
///   "contact_name": "John Smith",
///   "contact_name@OData.Community.Display.V1.AliasedValue": {
///     "Value": "John Smith",
///     "EntityLogicalName": "contact",
///     "AttributeLogicalName": "fullname"
///   }
/// }
/// ```
///
/// This struct preserves that metadata:
/// ```
/// use dataverse_lib::model::types::AliasedValue;
/// use dataverse_lib::model::Value;
///
/// let aliased = AliasedValue {
///     value: Value::from("John Smith"),
///     entity_logical_name: "contact".to_string(),
///     attribute_logical_name: "fullname".to_string(),
/// };
///
/// assert_eq!(aliased.entity_logical_name, "contact");
/// assert_eq!(aliased.attribute_logical_name, "fullname");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AliasedValue {
    /// The actual value.
    pub value: Value,
    /// The logical name of the entity this value came from.
    pub entity_logical_name: String,
    /// The logical name of the attribute this value came from.
    pub attribute_logical_name: String,
}

impl AliasedValue {
    /// Creates a new aliased value.
    pub fn new(
        value: impl Into<Value>,
        entity_logical_name: impl Into<String>,
        attribute_logical_name: impl Into<String>,
    ) -> Self {
        Self {
            value: value.into(),
            entity_logical_name: entity_logical_name.into(),
            attribute_logical_name: attribute_logical_name.into(),
        }
    }

    /// Returns the inner value, discarding the alias metadata.
    pub fn into_value(self) -> Value {
        self.value
    }
}
