//! Batch/changeset operations
//!
//! This module provides batch request support for Dataverse, allowing multiple
//! operations in a single HTTP request with support for transactions (changesets),
//! content-id references, and per-operation error handling.
//!
//! # Example
//!
//! ```ignore
//! let results = client.batch()
//!     .bypass_plugins()           // batch-level setting
//!     .continue_on_error()        // don't stop on first failure
//!     .add(Op::create(entity, record1))
//!     .add(Op::create(entity, record2))
//!     .add(Op::delete(entity, id))
//!     .execute()
//!     .await?;
//! ```

pub mod multipart;
pub mod response;

pub use response::BatchItemResult;
pub use response::BatchOperationError;
pub use response::BatchResults;
pub use response::OperationResult as BatchOperationResult;

use crate::api::crud::Operation;
use crate::error::Error;

// =============================================================================
// Batch Options
// =============================================================================

/// Batch-level options applied to all operations unless overridden.
#[derive(Debug, Clone, Default)]
pub struct BatchOptions {
    /// Continue processing operations after a failure.
    pub continue_on_error: bool,
    /// Skip custom plugin execution.
    pub bypass_plugins: bool,
    /// Skip Power Automate flows.
    pub bypass_flows: bool,
    /// Skip synchronous business logic.
    pub bypass_sync_logic: bool,
    /// Skip duplicate detection rules.
    pub suppress_duplicate_detection: bool,
    /// Request the record in responses for create/update operations.
    pub return_record: bool,
}

impl BatchOptions {
    /// Returns the bypass headers for batch-level settings.
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
// Content-ID Reference
// =============================================================================

/// Opaque reference to a Content-ID for referencing operation results.
///
/// Used within changesets to reference the result of an earlier operation
/// (e.g., binding a contact to a newly created account).
#[derive(Debug, Clone)]
pub struct ContentIdRef(pub(crate) String);

impl ContentIdRef {
    /// Returns the reference string (e.g., "$1").
    pub fn as_ref_string(&self) -> &str {
        &self.0
    }
}

// =============================================================================
// Changeset
// =============================================================================

/// A transactional group of operations.
///
/// All operations in a changeset either succeed together or fail together
/// (rollback). Changesets cannot be nested.
#[derive(Debug, Clone)]
pub struct Changeset {
    pub(crate) operations: Vec<Operation>,
}

// =============================================================================
// Batch Item
// =============================================================================

/// A single item in a batch.
#[derive(Debug, Clone)]
pub enum BatchItem {
    /// A standalone operation (not transactional).
    Operation(Operation),
    /// A transactional group of operations.
    Changeset(Changeset),
}

// =============================================================================
// Batch
// =============================================================================

/// A batch of operations to execute in a single HTTP request.
///
/// Use [`DataverseClient::batch()`] to create a batch builder bound to a client.
#[derive(Debug, Clone)]
pub struct Batch {
    pub(crate) items: Vec<BatchItem>,
    pub(crate) options: BatchOptions,
}

impl Default for Batch {
    fn default() -> Self {
        Self::new()
    }
}

impl Batch {
    /// Creates a new empty batch.
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            options: BatchOptions::default(),
        }
    }

    // -------------------------------------------------------------------------
    // Batch-level settings
    // -------------------------------------------------------------------------

    /// Continue processing operations after a failure.
    ///
    /// By default, processing stops at the first error. With this enabled,
    /// all operations are attempted and individual errors are returned.
    pub fn continue_on_error(mut self) -> Self {
        self.options.continue_on_error = true;
        self
    }

    /// Skip custom plugin execution for all operations.
    pub fn bypass_plugins(mut self) -> Self {
        self.options.bypass_plugins = true;
        self
    }

    /// Skip Power Automate flows for all operations.
    pub fn bypass_flows(mut self) -> Self {
        self.options.bypass_flows = true;
        self
    }

    /// Skip synchronous business logic for all operations.
    pub fn bypass_sync_logic(mut self) -> Self {
        self.options.bypass_sync_logic = true;
        self
    }

    /// Skip duplicate detection for all operations.
    pub fn suppress_duplicate_detection(mut self) -> Self {
        self.options.suppress_duplicate_detection = true;
        self
    }

    /// Request the record in responses for create/update operations.
    pub fn return_record(mut self) -> Self {
        self.options.return_record = true;
        self
    }

    // -------------------------------------------------------------------------
    // Adding items
    // -------------------------------------------------------------------------

    /// Adds a standalone operation to the batch.
    pub fn add(mut self, op: impl Into<Operation>) -> Self {
        self.items.push(BatchItem::Operation(op.into()));
        self
    }

    /// Adds a transactional changeset to the batch.
    ///
    /// All operations in the changeset succeed or fail together.
    ///
    /// # Example
    ///
    /// ```ignore
    /// client.batch()
    ///     .changeset(|cs| {
    ///         let account_ref = cs.add(Op::create(&accounts, account));
    ///         cs.add(Op::create(&contacts, contact
    ///             .bind_ref("parentcustomerid", "accounts", &account_ref)));
    ///     })
    ///     .execute()
    ///     .await?;
    /// ```
    pub fn changeset<F>(mut self, build: F) -> Self
    where
        F: FnOnce(&mut ChangesetBuilder),
    {
        let mut builder = ChangesetBuilder::new();
        build(&mut builder);
        self.items.push(BatchItem::Changeset(builder.build()));
        self
    }

    // -------------------------------------------------------------------------
    // Validation
    // -------------------------------------------------------------------------

    /// Validates the batch.
    ///
    /// Returns an error if the batch exceeds the maximum of 1000 operations.
    pub fn validate(&self) -> Result<(), Error> {
        let count = self.operation_count();
        if count > 1000 {
            return Err(Error::BatchSizeExceeded { count, max: 1000 });
        }
        Ok(())
    }

    /// Counts the total number of operations (including within changesets).
    pub fn operation_count(&self) -> usize {
        self.items
            .iter()
            .map(|item| match item {
                BatchItem::Operation(_) => 1,
                BatchItem::Changeset(cs) => cs.operations.len(),
            })
            .sum()
    }

    /// Returns true if the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns the number of items (operations + changesets) in the batch.
    pub fn len(&self) -> usize {
        self.items.len()
    }
}

// =============================================================================
// Changeset Builder
// =============================================================================

/// Builder for changesets - only allows adding Operations (not nested changesets).
pub struct ChangesetBuilder {
    operations: Vec<Operation>,
    next_content_id: u32,
}

impl ChangesetBuilder {
    pub(crate) fn new() -> Self {
        Self {
            operations: Vec::new(),
            next_content_id: 1,
        }
    }

    /// Adds an operation and returns a [`ContentIdRef`] for referencing its result.
    ///
    /// The returned reference can be used with [`Record::bind_ref()`] to bind
    /// a lookup field to the result of this operation.
    pub fn add(&mut self, op: impl Into<Operation>) -> ContentIdRef {
        let content_id = self.next_content_id;
        self.next_content_id += 1;

        let mut operation = op.into();
        operation.set_content_id(content_id.to_string());

        let reference = ContentIdRef(format!("${}", content_id));
        self.operations.push(operation);
        reference
    }

    pub(crate) fn build(self) -> Changeset {
        Changeset {
            operations: self.operations,
        }
    }
}
