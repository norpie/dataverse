//! Create, Read, Update, Delete operations
//!
//! This module provides the core CRUD operations for Dataverse records.
//! Operations can be executed standalone or combined into batches.
//!
//! # Example
//!
//! ```ignore
//! use dataverse_lib::{DataverseClient, Entity, Record};
//! use dataverse_lib::api::{Op, Operation};
//!
//! // Standalone execution
//! let id = client.create(Entity::set("accounts"), record).await?;
//!
//! // Or build an operation for batching
//! let op = Op::create(Entity::set("accounts"), record)
//!     .bypass_plugins()
//!     .build();
//! client.execute(op).await?;
//! ```

use uuid::Uuid;

use crate::model::Entity;
use crate::model::Record;

// =============================================================================
// Operation Options
// =============================================================================

/// Options that can be applied to CRUD operations.
///
/// These options control behavior like plugin execution, duplicate detection,
/// concurrency handling, and response format.
#[derive(Debug, Clone, Default)]
pub struct OperationOptions {
    /// Content-ID for batch operations (allows referencing results).
    pub content_id: Option<String>,
    /// Request the created/updated record in the response.
    pub return_record: bool,
    /// Fields to include in the returned record.
    pub select: Vec<String>,
    /// Skip custom plugin execution.
    pub bypass_plugins: bool,
    /// Skip Power Automate flows.
    pub bypass_flows: bool,
    /// Skip synchronous business logic.
    pub bypass_sync_logic: bool,
    /// Skip duplicate detection rules.
    pub suppress_duplicate_detection: bool,
    /// ETag for optimistic concurrency (If-Match header).
    pub if_match: Option<String>,
    /// Only succeed if record doesn't exist (If-None-Match: * header).
    pub if_none_match: bool,
}

impl OperationOptions {
    /// Creates new default options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the bypass headers for this operation.
    pub(crate) fn bypass_headers(&self) -> Vec<(&'static str, &'static str)> {
        let mut headers = Vec::new();
        if self.bypass_plugins {
            headers.push(("MSCRM.BypassCustomPluginExecution", "true"));
        }
        if self.bypass_flows {
            headers.push(("MSCRM.BypassPowerAutomateFlows", "true"));
        }
        if self.bypass_sync_logic {
            headers.push(("MSCRM.BypassBusinessLogicExecution", "true"));
        }
        if self.suppress_duplicate_detection {
            headers.push(("MSCRM.SuppressDuplicateDetection", "true"));
        }
        headers
    }
}

// =============================================================================
// Expand configuration for Retrieve
// =============================================================================

/// Configuration for expanding a navigation property in a retrieve operation.
#[derive(Debug, Clone)]
pub struct Expand {
    /// The navigation property to expand.
    pub nav_property: String,
    /// Fields to select from the expanded entity.
    pub select: Vec<String>,
    /// Nested expands.
    pub expand: Vec<Expand>,
    /// Filter for collection-valued navigation properties.
    pub filter: Option<String>,
    /// Order by for collection-valued navigation properties.
    pub order_by: Option<String>,
    /// Top N for collection-valued navigation properties.
    pub top: Option<usize>,
}

impl Expand {
    /// Creates a new expand for a navigation property.
    pub fn new(nav_property: impl Into<String>) -> Self {
        Self {
            nav_property: nav_property.into(),
            select: Vec::new(),
            expand: Vec::new(),
            filter: None,
            order_by: None,
            top: None,
        }
    }

    /// Sets the fields to select.
    pub fn select(mut self, fields: &[&str]) -> Self {
        self.select = fields.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Adds a nested expand.
    pub fn expand(mut self, expand: Expand) -> Self {
        self.expand.push(expand);
        self
    }

    /// Sets a filter (for collection-valued navigation properties).
    pub fn filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = Some(filter.into());
        self
    }

    /// Sets order by (for collection-valued navigation properties).
    pub fn order_by(mut self, order_by: impl Into<String>) -> Self {
        self.order_by = Some(order_by.into());
        self
    }

    /// Sets top N (for collection-valued navigation properties).
    pub fn top(mut self, n: usize) -> Self {
        self.top = Some(n);
        self
    }

    /// Builds the OData $expand query string for this expand.
    pub(crate) fn to_odata_string(&self) -> String {
        let mut parts = Vec::new();

        if !self.select.is_empty() {
            parts.push(format!("$select={}", self.select.join(",")));
        }

        if let Some(ref filter) = self.filter {
            parts.push(format!("$filter={}", filter));
        }

        if let Some(ref order_by) = self.order_by {
            parts.push(format!("$orderby={}", order_by));
        }

        if let Some(top) = self.top {
            parts.push(format!("$top={}", top));
        }

        if !self.expand.is_empty() {
            let nested: Vec<String> = self.expand.iter().map(|e| e.to_odata_string()).collect();
            parts.push(format!("$expand={}", nested.join(",")));
        }

        if parts.is_empty() {
            self.nav_property.clone()
        } else {
            format!("{}({})", self.nav_property, parts.join(";"))
        }
    }
}

// =============================================================================
// Operation enum
// =============================================================================

/// A CRUD operation that can be executed standalone or in a batch.
///
/// Operations are the central type for representing Dataverse actions.
/// They can be built using the [`Op`] helper or converted from client builders.
#[derive(Debug, Clone)]
pub enum Operation {
    /// Create a new record.
    Create {
        entity: Entity,
        record: Record,
        options: OperationOptions,
    },

    /// Retrieve a record by ID.
    Retrieve {
        entity: Entity,
        id: Uuid,
        select: Vec<String>,
        expand: Vec<Expand>,
        options: OperationOptions,
    },

    /// Update an existing record.
    Update {
        entity: Entity,
        id: Uuid,
        record: Record,
        options: OperationOptions,
    },

    /// Delete a record.
    Delete {
        entity: Entity,
        id: Uuid,
        options: OperationOptions,
    },

    /// Upsert (create or update) a record.
    Upsert {
        entity: Entity,
        id: Uuid,
        record: Record,
        options: OperationOptions,
    },

    /// Associate two records via N:N relationship.
    Associate {
        entity: Entity,
        id: Uuid,
        relationship: String,
        target_entity: Entity,
        target_id: Uuid,
        options: OperationOptions,
    },

    /// Disassociate two records from N:N relationship.
    Disassociate {
        entity: Entity,
        id: Uuid,
        relationship: String,
        target_id: Uuid,
        options: OperationOptions,
    },

    /// Set a single-valued navigation property (lookup).
    SetLookup {
        entity: Entity,
        id: Uuid,
        nav_property: String,
        target_entity: Entity,
        target_id: Uuid,
        options: OperationOptions,
    },

    /// Clear a single-valued navigation property (lookup).
    ClearLookup {
        entity: Entity,
        id: Uuid,
        nav_property: String,
        options: OperationOptions,
    },
}

// =============================================================================
// Op helper for building operations
// =============================================================================

/// Helper for building [`Operation`]s.
///
/// # Example
///
/// ```ignore
/// use dataverse_lib::api::Op;
/// use dataverse_lib::Entity;
///
/// let op = Op::create(Entity::set("accounts"), record)
///     .bypass_plugins()
///     .content_id("acc1")
///     .build();
/// ```
pub struct Op;

impl Op {
    /// Creates a new Create operation builder.
    pub fn create(entity: Entity, record: Record) -> CreateBuilder {
        CreateBuilder {
            entity,
            record,
            options: OperationOptions::default(),
        }
    }

    /// Creates a new Retrieve operation builder.
    pub fn retrieve(entity: Entity, id: Uuid) -> RetrieveBuilder {
        RetrieveBuilder {
            entity,
            id,
            select: Vec::new(),
            expand: Vec::new(),
            options: OperationOptions::default(),
        }
    }

    /// Creates a new Update operation builder.
    pub fn update(entity: Entity, id: Uuid, record: Record) -> UpdateBuilder {
        UpdateBuilder {
            entity,
            id,
            record,
            options: OperationOptions::default(),
        }
    }

    /// Creates a new Delete operation builder.
    pub fn delete(entity: Entity, id: Uuid) -> DeleteBuilder {
        DeleteBuilder {
            entity,
            id,
            options: OperationOptions::default(),
        }
    }

    /// Creates a new Upsert operation builder.
    pub fn upsert(entity: Entity, id: Uuid, record: Record) -> UpsertBuilder {
        UpsertBuilder {
            entity,
            id,
            record,
            options: OperationOptions::default(),
        }
    }

    /// Creates a new Associate operation builder.
    pub fn associate(
        entity: Entity,
        id: Uuid,
        relationship: impl Into<String>,
        target_entity: Entity,
        target_id: Uuid,
    ) -> AssociateBuilder {
        AssociateBuilder {
            entity,
            id,
            relationship: relationship.into(),
            target_entity,
            target_id,
            options: OperationOptions::default(),
        }
    }

    /// Creates a new Disassociate operation builder.
    pub fn disassociate(
        entity: Entity,
        id: Uuid,
        relationship: impl Into<String>,
        target_id: Uuid,
    ) -> DisassociateBuilder {
        DisassociateBuilder {
            entity,
            id,
            relationship: relationship.into(),
            target_id,
            options: OperationOptions::default(),
        }
    }

    /// Creates a new SetLookup operation builder.
    pub fn set_lookup(
        entity: Entity,
        id: Uuid,
        nav_property: impl Into<String>,
        target_entity: Entity,
        target_id: Uuid,
    ) -> SetLookupBuilder {
        SetLookupBuilder {
            entity,
            id,
            nav_property: nav_property.into(),
            target_entity,
            target_id,
            options: OperationOptions::default(),
        }
    }

    /// Creates a new ClearLookup operation builder.
    pub fn clear_lookup(
        entity: Entity,
        id: Uuid,
        nav_property: impl Into<String>,
    ) -> ClearLookupBuilder {
        ClearLookupBuilder {
            entity,
            id,
            nav_property: nav_property.into(),
            options: OperationOptions::default(),
        }
    }
}

// =============================================================================
// Operation Builders
// =============================================================================

/// Builder for Create operations.
#[derive(Debug, Clone)]
pub struct CreateBuilder {
    entity: Entity,
    record: Record,
    options: OperationOptions,
}

impl CreateBuilder {
    /// Sets the content ID for batch referencing.
    pub fn content_id(mut self, id: impl Into<String>) -> Self {
        self.options.content_id = Some(id.into());
        self
    }

    /// Request the created record in the response.
    pub fn return_record(mut self) -> Self {
        self.options.return_record = true;
        self
    }

    /// Sets the fields to include in the returned record.
    pub fn select(mut self, fields: &[&str]) -> Self {
        self.options.select = fields.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Skip custom plugin execution.
    pub fn bypass_plugins(mut self) -> Self {
        self.options.bypass_plugins = true;
        self
    }

    /// Skip Power Automate flows.
    pub fn bypass_flows(mut self) -> Self {
        self.options.bypass_flows = true;
        self
    }

    /// Skip synchronous business logic.
    pub fn bypass_sync_logic(mut self) -> Self {
        self.options.bypass_sync_logic = true;
        self
    }

    /// Skip duplicate detection.
    pub fn suppress_duplicate_detection(mut self) -> Self {
        self.options.suppress_duplicate_detection = true;
        self
    }

    /// Builds the operation.
    pub fn build(self) -> Operation {
        Operation::Create {
            entity: self.entity,
            record: self.record,
            options: self.options,
        }
    }
}

impl From<CreateBuilder> for Operation {
    fn from(builder: CreateBuilder) -> Self {
        builder.build()
    }
}

/// Builder for Retrieve operations.
#[derive(Debug, Clone)]
pub struct RetrieveBuilder {
    entity: Entity,
    id: Uuid,
    select: Vec<String>,
    expand: Vec<Expand>,
    options: OperationOptions,
}

impl RetrieveBuilder {
    /// Sets the fields to retrieve.
    pub fn select(mut self, fields: &[&str]) -> Self {
        self.select = fields.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Adds a navigation property to expand.
    pub fn expand(
        mut self,
        nav_property: impl Into<String>,
        configure: impl FnOnce(Expand) -> Expand,
    ) -> Self {
        let expand = configure(Expand::new(nav_property));
        self.expand.push(expand);
        self
    }

    /// Bypasses custom plugins for this operation.
    pub fn bypass_plugins(mut self) -> Self {
        self.options.bypass_plugins = true;
        self
    }

    /// Bypasses Power Automate flows for this operation.
    pub fn bypass_flows(mut self) -> Self {
        self.options.bypass_flows = true;
        self
    }

    /// Builds the operation.
    pub fn build(self) -> Operation {
        Operation::Retrieve {
            entity: self.entity,
            id: self.id,
            select: self.select,
            expand: self.expand,
            options: self.options,
        }
    }
}

impl From<RetrieveBuilder> for Operation {
    fn from(builder: RetrieveBuilder) -> Self {
        builder.build()
    }
}

/// Builder for Update operations.
#[derive(Debug, Clone)]
pub struct UpdateBuilder {
    entity: Entity,
    id: Uuid,
    record: Record,
    options: OperationOptions,
}

impl UpdateBuilder {
    /// Sets the content ID for batch referencing.
    pub fn content_id(mut self, id: impl Into<String>) -> Self {
        self.options.content_id = Some(id.into());
        self
    }

    /// Request the updated record in the response.
    pub fn return_record(mut self) -> Self {
        self.options.return_record = true;
        self
    }

    /// Sets the fields to include in the returned record.
    pub fn select(mut self, fields: &[&str]) -> Self {
        self.options.select = fields.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Sets the ETag for optimistic concurrency.
    pub fn if_match(mut self, etag: impl Into<String>) -> Self {
        self.options.if_match = Some(etag.into());
        self
    }

    /// Skip custom plugin execution.
    pub fn bypass_plugins(mut self) -> Self {
        self.options.bypass_plugins = true;
        self
    }

    /// Skip Power Automate flows.
    pub fn bypass_flows(mut self) -> Self {
        self.options.bypass_flows = true;
        self
    }

    /// Skip synchronous business logic.
    pub fn bypass_sync_logic(mut self) -> Self {
        self.options.bypass_sync_logic = true;
        self
    }

    /// Skip duplicate detection.
    pub fn suppress_duplicate_detection(mut self) -> Self {
        self.options.suppress_duplicate_detection = true;
        self
    }

    /// Builds the operation.
    pub fn build(self) -> Operation {
        Operation::Update {
            entity: self.entity,
            id: self.id,
            record: self.record,
            options: self.options,
        }
    }
}

impl From<UpdateBuilder> for Operation {
    fn from(builder: UpdateBuilder) -> Self {
        builder.build()
    }
}

/// Builder for Delete operations.
#[derive(Debug, Clone)]
pub struct DeleteBuilder {
    entity: Entity,
    id: Uuid,
    options: OperationOptions,
}

impl DeleteBuilder {
    /// Sets the content ID for batch referencing.
    pub fn content_id(mut self, id: impl Into<String>) -> Self {
        self.options.content_id = Some(id.into());
        self
    }

    /// Sets the ETag for optimistic concurrency.
    pub fn if_match(mut self, etag: impl Into<String>) -> Self {
        self.options.if_match = Some(etag.into());
        self
    }

    /// Skip custom plugin execution.
    pub fn bypass_plugins(mut self) -> Self {
        self.options.bypass_plugins = true;
        self
    }

    /// Skip Power Automate flows.
    pub fn bypass_flows(mut self) -> Self {
        self.options.bypass_flows = true;
        self
    }

    /// Skip synchronous business logic.
    pub fn bypass_sync_logic(mut self) -> Self {
        self.options.bypass_sync_logic = true;
        self
    }

    /// Builds the operation.
    pub fn build(self) -> Operation {
        Operation::Delete {
            entity: self.entity,
            id: self.id,
            options: self.options,
        }
    }
}

impl From<DeleteBuilder> for Operation {
    fn from(builder: DeleteBuilder) -> Self {
        builder.build()
    }
}

/// Builder for Upsert operations.
#[derive(Debug, Clone)]
pub struct UpsertBuilder {
    entity: Entity,
    id: Uuid,
    record: Record,
    options: OperationOptions,
}

impl UpsertBuilder {
    /// Sets the content ID for batch referencing.
    pub fn content_id(mut self, id: impl Into<String>) -> Self {
        self.options.content_id = Some(id.into());
        self
    }

    /// Request the created/updated record in the response.
    pub fn return_record(mut self) -> Self {
        self.options.return_record = true;
        self
    }

    /// Sets the fields to include in the returned record.
    pub fn select(mut self, fields: &[&str]) -> Self {
        self.options.select = fields.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Only create if doesn't exist (If-None-Match: *).
    pub fn if_none_match(mut self) -> Self {
        self.options.if_none_match = true;
        self
    }

    /// Only update if exists (If-Match: *).
    pub fn if_match_any(mut self) -> Self {
        self.options.if_match = Some("*".to_string());
        self
    }

    /// Skip custom plugin execution.
    pub fn bypass_plugins(mut self) -> Self {
        self.options.bypass_plugins = true;
        self
    }

    /// Skip Power Automate flows.
    pub fn bypass_flows(mut self) -> Self {
        self.options.bypass_flows = true;
        self
    }

    /// Skip synchronous business logic.
    pub fn bypass_sync_logic(mut self) -> Self {
        self.options.bypass_sync_logic = true;
        self
    }

    /// Skip duplicate detection.
    pub fn suppress_duplicate_detection(mut self) -> Self {
        self.options.suppress_duplicate_detection = true;
        self
    }

    /// Builds the operation.
    pub fn build(self) -> Operation {
        Operation::Upsert {
            entity: self.entity,
            id: self.id,
            record: self.record,
            options: self.options,
        }
    }
}

impl From<UpsertBuilder> for Operation {
    fn from(builder: UpsertBuilder) -> Self {
        builder.build()
    }
}

/// Builder for Associate operations.
#[derive(Debug, Clone)]
pub struct AssociateBuilder {
    entity: Entity,
    id: Uuid,
    relationship: String,
    target_entity: Entity,
    target_id: Uuid,
    options: OperationOptions,
}

impl AssociateBuilder {
    /// Sets the content ID for batch referencing.
    pub fn content_id(mut self, id: impl Into<String>) -> Self {
        self.options.content_id = Some(id.into());
        self
    }

    /// Skip custom plugin execution.
    pub fn bypass_plugins(mut self) -> Self {
        self.options.bypass_plugins = true;
        self
    }

    /// Skip Power Automate flows.
    pub fn bypass_flows(mut self) -> Self {
        self.options.bypass_flows = true;
        self
    }

    /// Builds the operation.
    pub fn build(self) -> Operation {
        Operation::Associate {
            entity: self.entity,
            id: self.id,
            relationship: self.relationship,
            target_entity: self.target_entity,
            target_id: self.target_id,
            options: self.options,
        }
    }
}

impl From<AssociateBuilder> for Operation {
    fn from(builder: AssociateBuilder) -> Self {
        builder.build()
    }
}

/// Builder for Disassociate operations.
#[derive(Debug, Clone)]
pub struct DisassociateBuilder {
    entity: Entity,
    id: Uuid,
    relationship: String,
    target_id: Uuid,
    options: OperationOptions,
}

impl DisassociateBuilder {
    /// Sets the content ID for batch referencing.
    pub fn content_id(mut self, id: impl Into<String>) -> Self {
        self.options.content_id = Some(id.into());
        self
    }

    /// Skip custom plugin execution.
    pub fn bypass_plugins(mut self) -> Self {
        self.options.bypass_plugins = true;
        self
    }

    /// Skip Power Automate flows.
    pub fn bypass_flows(mut self) -> Self {
        self.options.bypass_flows = true;
        self
    }

    /// Builds the operation.
    pub fn build(self) -> Operation {
        Operation::Disassociate {
            entity: self.entity,
            id: self.id,
            relationship: self.relationship,
            target_id: self.target_id,
            options: self.options,
        }
    }
}

impl From<DisassociateBuilder> for Operation {
    fn from(builder: DisassociateBuilder) -> Self {
        builder.build()
    }
}

/// Builder for SetLookup operations.
#[derive(Debug, Clone)]
pub struct SetLookupBuilder {
    entity: Entity,
    id: Uuid,
    nav_property: String,
    target_entity: Entity,
    target_id: Uuid,
    options: OperationOptions,
}

impl SetLookupBuilder {
    /// Sets the content ID for batch referencing.
    pub fn content_id(mut self, id: impl Into<String>) -> Self {
        self.options.content_id = Some(id.into());
        self
    }

    /// Skip custom plugin execution.
    pub fn bypass_plugins(mut self) -> Self {
        self.options.bypass_plugins = true;
        self
    }

    /// Skip Power Automate flows.
    pub fn bypass_flows(mut self) -> Self {
        self.options.bypass_flows = true;
        self
    }

    /// Builds the operation.
    pub fn build(self) -> Operation {
        Operation::SetLookup {
            entity: self.entity,
            id: self.id,
            nav_property: self.nav_property,
            target_entity: self.target_entity,
            target_id: self.target_id,
            options: self.options,
        }
    }
}

impl From<SetLookupBuilder> for Operation {
    fn from(builder: SetLookupBuilder) -> Self {
        builder.build()
    }
}

/// Builder for ClearLookup operations.
#[derive(Debug, Clone)]
pub struct ClearLookupBuilder {
    entity: Entity,
    id: Uuid,
    nav_property: String,
    options: OperationOptions,
}

impl ClearLookupBuilder {
    /// Sets the content ID for batch referencing.
    pub fn content_id(mut self, id: impl Into<String>) -> Self {
        self.options.content_id = Some(id.into());
        self
    }

    /// Skip custom plugin execution.
    pub fn bypass_plugins(mut self) -> Self {
        self.options.bypass_plugins = true;
        self
    }

    /// Skip Power Automate flows.
    pub fn bypass_flows(mut self) -> Self {
        self.options.bypass_flows = true;
        self
    }

    /// Builds the operation.
    pub fn build(self) -> Operation {
        Operation::ClearLookup {
            entity: self.entity,
            id: self.id,
            nav_property: self.nav_property,
            options: self.options,
        }
    }
}

impl From<ClearLookupBuilder> for Operation {
    fn from(builder: ClearLookupBuilder) -> Self {
        builder.build()
    }
}

// =============================================================================
// Operation Results
// =============================================================================

/// Result of a create operation.
#[derive(Debug, Clone)]
pub enum CreateResult {
    /// Only the ID was returned (default).
    Id(Uuid),
    /// The full record was returned (with return_record()).
    Record(Record),
}

impl CreateResult {
    /// Returns the ID of the created record.
    ///
    /// # Errors
    ///
    /// Returns an error if the record variant doesn't contain an ID.
    pub fn id(&self) -> Result<Uuid, crate::error::Error> {
        match self {
            CreateResult::Id(id) => Ok(*id),
            CreateResult::Record(record) => record.id().ok_or_else(|| {
                crate::error::Error::InvalidOperation(
                    "Created record does not contain an ID".to_string(),
                )
            }),
        }
    }

    /// Returns the record if it was requested, None otherwise.
    pub fn record(&self) -> Option<&Record> {
        match self {
            CreateResult::Id(_) => None,
            CreateResult::Record(record) => Some(record),
        }
    }

    /// Consumes self and returns the record if it was requested.
    pub fn into_record(self) -> Option<Record> {
        match self {
            CreateResult::Id(_) => None,
            CreateResult::Record(record) => Some(record),
        }
    }
}

/// Result of an upsert operation.
#[derive(Debug, Clone)]
pub enum UpsertResult {
    /// A new record was created.
    Created(CreateResult),
    /// An existing record was updated.
    Updated {
        /// The ID of the updated record.
        id: Uuid,
        /// The record, if `return_record()` was set.
        record: Option<Record>,
    },
}

impl UpsertResult {
    /// Returns true if a new record was created.
    pub fn is_created(&self) -> bool {
        matches!(self, UpsertResult::Created(_))
    }

    /// Returns true if an existing record was updated.
    pub fn is_updated(&self) -> bool {
        matches!(self, UpsertResult::Updated { .. })
    }

    /// Returns the ID of the created/updated record.
    ///
    /// # Errors
    ///
    /// Returns an error if a created record doesn't contain an ID.
    pub fn id(&self) -> Result<Uuid, crate::error::Error> {
        match self {
            UpsertResult::Created(result) => result.id(),
            UpsertResult::Updated { id, .. } => Ok(*id),
        }
    }

    /// Returns the record if it was requested.
    pub fn record(&self) -> Option<&Record> {
        match self {
            UpsertResult::Created(result) => result.record(),
            UpsertResult::Updated { record, .. } => record.as_ref(),
        }
    }
}
