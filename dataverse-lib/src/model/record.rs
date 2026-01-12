//! Dynamic entity record

use std::collections::HashMap;

use chrono::DateTime;
use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use super::Value;
use super::types::EntityBinding;
use super::types::EntityReference;
use super::types::FileReference;
use super::types::ImageReference;
use super::types::Money;
use super::types::MultiSelectOptionSetValue;
use super::types::OptionSetValue;
use crate::api::ContentIdRef;
use crate::error::FieldError;

/// A dynamic entity record from Dataverse.
///
/// Records hold field values as a `HashMap<String, Value>`, allowing dynamic
/// access to any field. Typed getter methods provide safe access with proper
/// error handling.
///
/// # Example
///
/// ```
/// use dataverse_lib::model::Record;
///
/// // Create a new record for writing
/// let record = Record::new("account")
///     .set("name", "Contoso")
///     .set("revenue", 1_000_000i64);
///
/// // Access fields
/// assert_eq!(record.get_string("name").unwrap(), Some("Contoso"));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    /// The logical name of the entity.
    pub(crate) entity_name: String,

    /// The unique identifier of the record.
    pub(crate) id: Option<Uuid>,

    /// The field values.
    pub(crate) fields: HashMap<String, Value>,

    /// Formatted display values (from @OData.Community.Display.V1.FormattedValue).
    pub(crate) formatted_values: HashMap<String, String>,

    /// The ETag for concurrency control.
    pub(crate) etag: Option<String>,
}

impl Record {
    /// Creates a new empty record for the given entity.
    pub fn new(entity_name: impl Into<String>) -> Self {
        Self {
            entity_name: entity_name.into(),
            id: None,
            fields: HashMap::new(),
            formatted_values: HashMap::new(),
            etag: None,
        }
    }

    /// Creates a new record with the given ID.
    pub fn with_id(entity_name: impl Into<String>, id: Uuid) -> Self {
        Self {
            entity_name: entity_name.into(),
            id: Some(id),
            fields: HashMap::new(),
            formatted_values: HashMap::new(),
            etag: None,
        }
    }

    // =========================================================================
    // Metadata accessors
    // =========================================================================

    /// Returns the entity logical name.
    pub fn entity_name(&self) -> &str {
        &self.entity_name
    }

    /// Returns the record ID, if set.
    pub fn id(&self) -> Option<Uuid> {
        self.id
    }

    /// Returns the ETag for concurrency control, if available.
    pub fn etag(&self) -> Option<&str> {
        self.etag.as_deref()
    }

    /// Sets the entity name.
    pub fn set_entity_name(&mut self, name: impl Into<String>) {
        self.entity_name = name.into();
    }

    /// Sets the record ID.
    pub fn set_id(&mut self, id: Uuid) {
        self.id = Some(id);
    }

    /// Sets the ETag.
    pub fn set_etag(&mut self, etag: impl Into<String>) {
        self.etag = Some(etag.into());
    }

    // =========================================================================
    // Raw field access
    // =========================================================================

    /// Returns a reference to the field value, if it exists.
    pub fn get(&self, field: &str) -> Option<&Value> {
        self.fields.get(field)
    }

    /// Returns `true` if the record contains the given field.
    pub fn contains(&self, field: &str) -> bool {
        self.fields.contains_key(field)
    }

    /// Returns a reference to all fields.
    pub fn fields(&self) -> &HashMap<String, Value> {
        &self.fields
    }

    /// Returns a mutable reference to all fields.
    pub fn fields_mut(&mut self) -> &mut HashMap<String, Value> {
        &mut self.fields
    }

    /// Returns the formatted value for a field, if available.
    pub fn get_formatted(&self, field: &str) -> Option<&str> {
        self.formatted_values.get(field).map(|s| s.as_str())
    }

    /// Returns a reference to all formatted values.
    pub fn formatted_values(&self) -> &HashMap<String, String> {
        &self.formatted_values
    }

    // =========================================================================
    // Setters
    // =========================================================================

    /// Sets a field value (builder pattern).
    pub fn set(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
        self.fields.insert(field.into(), value.into());
        self
    }

    /// Inserts a field value.
    pub fn insert(&mut self, field: impl Into<String>, value: impl Into<Value>) {
        self.fields.insert(field.into(), value.into());
    }

    /// Removes a field and returns its value.
    pub fn remove(&mut self, field: &str) -> Option<Value> {
        self.fields.remove(field)
    }

    /// Sets a formatted value.
    pub fn set_formatted(&mut self, field: impl Into<String>, value: impl Into<String>) {
        self.formatted_values.insert(field.into(), value.into());
    }

    /// Binds a lookup field to a ContentIdRef for batch references.
    ///
    /// Used within changesets to reference the result of an earlier operation.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let record = Record::new("contact")
    ///     .set("firstname", "John")
    ///     .bind_ref("parentcustomerid", "accounts", &account_ref);
    /// ```
    pub fn bind_ref(
        mut self,
        field: impl Into<String>,
        entity_set: &str,
        reference: &ContentIdRef,
    ) -> Self {
        // Format: field@odata.bind = "entity_set/$N"
        let bind_key = format!("{}@odata.bind", field.into());
        let bind_value = format!("{}/{}", entity_set, reference.as_ref_string());
        self.fields.insert(bind_key, Value::String(bind_value));
        self
    }

    // =========================================================================
    // Typed getters
    //
    // Return Err if field is missing or wrong type.
    // Return Ok(None) only if the field exists and is Value::Null.
    // =========================================================================

    /// Gets a string field value.
    pub fn get_string(&self, field: &str) -> Result<Option<&str>, FieldError> {
        match self.fields.get(field) {
            None => Err(FieldError::missing(field)),
            Some(Value::Null) => Ok(None),
            Some(Value::String(s)) => Ok(Some(s.as_str())),
            Some(other) => Err(FieldError::type_mismatch(
                field,
                "string",
                other.type_name(),
            )),
        }
    }

    /// Gets a boolean field value.
    pub fn get_bool(&self, field: &str) -> Result<Option<bool>, FieldError> {
        match self.fields.get(field) {
            None => Err(FieldError::missing(field)),
            Some(Value::Null) => Ok(None),
            Some(Value::Bool(b)) => Ok(Some(*b)),
            Some(other) => Err(FieldError::type_mismatch(field, "bool", other.type_name())),
        }
    }

    /// Gets an i32 field value.
    pub fn get_int(&self, field: &str) -> Result<Option<i32>, FieldError> {
        match self.fields.get(field) {
            None => Err(FieldError::missing(field)),
            Some(Value::Null) => Ok(None),
            Some(Value::Int(n)) => Ok(Some(*n)),
            Some(other) => Err(FieldError::type_mismatch(field, "int", other.type_name())),
        }
    }

    /// Gets an i64 field value.
    pub fn get_long(&self, field: &str) -> Result<Option<i64>, FieldError> {
        match self.fields.get(field) {
            None => Err(FieldError::missing(field)),
            Some(Value::Null) => Ok(None),
            Some(Value::Long(n)) => Ok(Some(*n)),
            Some(Value::Int(n)) => Ok(Some(*n as i64)), // Allow widening
            Some(other) => Err(FieldError::type_mismatch(field, "long", other.type_name())),
        }
    }

    /// Gets an f64 field value.
    pub fn get_float(&self, field: &str) -> Result<Option<f64>, FieldError> {
        match self.fields.get(field) {
            None => Err(FieldError::missing(field)),
            Some(Value::Null) => Ok(None),
            Some(Value::Float(n)) => Ok(Some(*n)),
            Some(other) => Err(FieldError::type_mismatch(field, "float", other.type_name())),
        }
    }

    /// Gets a Decimal field value.
    pub fn get_decimal(&self, field: &str) -> Result<Option<Decimal>, FieldError> {
        match self.fields.get(field) {
            None => Err(FieldError::missing(field)),
            Some(Value::Null) => Ok(None),
            Some(Value::Decimal(d)) => Ok(Some(*d)),
            Some(other) => Err(FieldError::type_mismatch(
                field,
                "decimal",
                other.type_name(),
            )),
        }
    }

    /// Gets a UUID field value.
    pub fn get_guid(&self, field: &str) -> Result<Option<Uuid>, FieldError> {
        match self.fields.get(field) {
            None => Err(FieldError::missing(field)),
            Some(Value::Null) => Ok(None),
            Some(Value::Guid(g)) => Ok(Some(*g)),
            Some(other) => Err(FieldError::type_mismatch(field, "guid", other.type_name())),
        }
    }

    /// Gets a DateTime field value.
    pub fn get_datetime(&self, field: &str) -> Result<Option<DateTime<Utc>>, FieldError> {
        match self.fields.get(field) {
            None => Err(FieldError::missing(field)),
            Some(Value::Null) => Ok(None),
            Some(Value::DateTime(dt)) => Ok(Some(*dt)),
            Some(other) => Err(FieldError::type_mismatch(
                field,
                "datetime",
                other.type_name(),
            )),
        }
    }

    /// Gets a Money field value.
    pub fn get_money(&self, field: &str) -> Result<Option<Money>, FieldError> {
        match self.fields.get(field) {
            None => Err(FieldError::missing(field)),
            Some(Value::Null) => Ok(None),
            Some(Value::Money(m)) => Ok(Some(*m)),
            Some(other) => Err(FieldError::type_mismatch(field, "money", other.type_name())),
        }
    }

    /// Gets an EntityReference field value.
    pub fn get_entity_reference(
        &self,
        field: &str,
    ) -> Result<Option<&EntityReference>, FieldError> {
        match self.fields.get(field) {
            None => Err(FieldError::missing(field)),
            Some(Value::Null) => Ok(None),
            Some(Value::EntityReference(r)) => Ok(Some(r)),
            Some(other) => Err(FieldError::type_mismatch(
                field,
                "entity_reference",
                other.type_name(),
            )),
        }
    }

    /// Gets an EntityBinding field value.
    pub fn get_entity_binding(&self, field: &str) -> Result<Option<&EntityBinding>, FieldError> {
        match self.fields.get(field) {
            None => Err(FieldError::missing(field)),
            Some(Value::Null) => Ok(None),
            Some(Value::EntityBinding(b)) => Ok(Some(b)),
            Some(other) => Err(FieldError::type_mismatch(
                field,
                "entity_binding",
                other.type_name(),
            )),
        }
    }

    /// Gets an OptionSetValue field value.
    pub fn get_option_set(&self, field: &str) -> Result<Option<&OptionSetValue>, FieldError> {
        match self.fields.get(field) {
            None => Err(FieldError::missing(field)),
            Some(Value::Null) => Ok(None),
            Some(Value::OptionSet(o)) => Ok(Some(o)),
            Some(other) => Err(FieldError::type_mismatch(
                field,
                "option_set",
                other.type_name(),
            )),
        }
    }

    /// Gets a MultiSelectOptionSetValue field value.
    pub fn get_multi_option_set(
        &self,
        field: &str,
    ) -> Result<Option<&MultiSelectOptionSetValue>, FieldError> {
        match self.fields.get(field) {
            None => Err(FieldError::missing(field)),
            Some(Value::Null) => Ok(None),
            Some(Value::MultiOptionSet(o)) => Ok(Some(o)),
            Some(other) => Err(FieldError::type_mismatch(
                field,
                "multi_option_set",
                other.type_name(),
            )),
        }
    }

    /// Gets a FileReference field value.
    pub fn get_file(&self, field: &str) -> Result<Option<&FileReference>, FieldError> {
        match self.fields.get(field) {
            None => Err(FieldError::missing(field)),
            Some(Value::Null) => Ok(None),
            Some(Value::File(f)) => Ok(Some(f)),
            Some(other) => Err(FieldError::type_mismatch(field, "file", other.type_name())),
        }
    }

    /// Gets an ImageReference field value.
    pub fn get_image(&self, field: &str) -> Result<Option<&ImageReference>, FieldError> {
        match self.fields.get(field) {
            None => Err(FieldError::missing(field)),
            Some(Value::Null) => Ok(None),
            Some(Value::Image(i)) => Ok(Some(i)),
            Some(other) => Err(FieldError::type_mismatch(field, "image", other.type_name())),
        }
    }

    /// Gets a nested Record field value (from expanded navigation property).
    pub fn get_record(&self, field: &str) -> Result<Option<&Record>, FieldError> {
        match self.fields.get(field) {
            None => Err(FieldError::missing(field)),
            Some(Value::Null) => Ok(None),
            Some(Value::Record(r)) => Ok(Some(r.as_ref())),
            Some(other) => Err(FieldError::type_mismatch(
                field,
                "record",
                other.type_name(),
            )),
        }
    }

    /// Gets a collection of Records (from expanded collection navigation property).
    pub fn get_records(&self, field: &str) -> Result<Option<&Vec<Record>>, FieldError> {
        match self.fields.get(field) {
            None => Err(FieldError::missing(field)),
            Some(Value::Null) => Ok(None),
            Some(Value::Records(r)) => Ok(Some(r)),
            Some(other) => Err(FieldError::type_mismatch(
                field,
                "records",
                other.type_name(),
            )),
        }
    }
}

impl Default for Record {
    fn default() -> Self {
        Self::new("")
    }
}
