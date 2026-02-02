//! Queue item execution logic.

use chrono::Utc;
use dataverse_lib::api::BatchItem;
use dataverse_lib::api::BatchItemResult;
use dataverse_lib::api::BatchOperationResult;
use dataverse_lib::api::OperationResult;
use dataverse_lib::error::Error as DataverseError;
use rafter::GlobalContext;

use crate::systems::client_management::ClientManagement;
use crate::systems::client_management::GetClient;

use super::api::QueueItemCompleted;
use super::repository::NewExecutionRecord;
use super::repository::NewOperationResult;
use super::repository::QueueRepository;
use super::types::ExecutionStatus;
use super::types::ItemStatus;
use super::types::QueueItem;
use super::types::QueuePayload;

/// Result of executing a queue item.
pub struct ExecutionResult {
    /// Final status for the queue item.
    pub status: ItemStatus,
    /// Execution record to persist.
    pub record: NewExecutionRecord,
    /// Per-operation results (for batches). execution_id will be filled in later.
    pub operation_results: Vec<NewOperationResult>,
}

/// Error that occurred during execution.
#[derive(Debug)]
pub enum ExecutionError {
    /// Failed to get client for the environment.
    ClientError(String),
    /// Dataverse API error.
    ApiError(DataverseError),
}

impl std::fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ClientError(msg) => write!(f, "Client error: {}", msg),
            Self::ApiError(e) => write!(f, "API error: {}", e),
        }
    }
}

/// Execute a queue item.
///
/// This is a stateless function that:
/// 1. Requests a client from ClientManagement
/// 2. Executes the operation(s)
/// 3. Returns an ExecutionResult with status and record
pub async fn execute_item(item: &QueueItem, gx: &GlobalContext) -> ExecutionResult {
    let started_at = Utc::now();

    // Get client for this environment
    let client_result = gx
        .request_system::<ClientManagement, GetClient>(GetClient {
            account_id: item.account_id,
            env_id: item.env_id,
        })
        .await;

    let client_info = match client_result {
        Ok(Ok(info)) => info,
        Ok(Err(e)) => {
            return make_error_result(item, started_at, ExecutionError::ClientError(e.to_string()));
        }
        Err(e) => {
            return make_error_result(
                item,
                started_at,
                ExecutionError::ClientError(format!("Request failed: {}", e)),
            );
        }
    };

    // Execute based on payload type
    match &item.payload {
        QueuePayload::Single(operation) => {
            execute_single(&client_info.client, item, operation.clone(), started_at).await
        }
        QueuePayload::Batch(batch) => {
            execute_batch(&client_info.client, item, batch.clone(), started_at).await
        }
    }
}

async fn execute_single(
    client: &dataverse_lib::DataverseClient,
    item: &QueueItem,
    operation: dataverse_lib::api::Operation,
    started_at: chrono::DateTime<Utc>,
) -> ExecutionResult {
    let content_id = operation.content_id().map(|s| s.to_string());
    let result = client.execute(operation).await;
    let completed_at = Utc::now();
    let duration_ms = (completed_at - started_at).num_milliseconds();

    match result {
        Ok(op_result) => {
            let (operation_type, result_data) = single_operation_result_info(&op_result);
            ExecutionResult {
                status: ItemStatus::Done,
                record: NewExecutionRecord {
                    item_id: item.id,
                    started_at,
                    completed_at,
                    duration_ms,
                    status: ExecutionStatus::Success,
                    error: None,
                    success_count: 1,
                    failure_count: 0,
                },
                operation_results: vec![NewOperationResult {
                    execution_id: 0, // filled in later
                    op_index: 0,
                    content_id,
                    success: true,
                    operation_type: Some(operation_type.to_string()),
                    result_data,
                    error_status: None,
                    error_code: None,
                    error_message: None,
                }],
            }
        }
        Err(e) => ExecutionResult {
            status: ItemStatus::Failed,
            record: NewExecutionRecord {
                item_id: item.id,
                started_at,
                completed_at,
                duration_ms,
                status: ExecutionStatus::Failed,
                error: Some(e.to_string()),
                success_count: 0,
                failure_count: 1,
            },
            operation_results: vec![NewOperationResult {
                execution_id: 0,
                op_index: 0,
                content_id,
                success: false,
                operation_type: None,
                result_data: None,
                error_status: None,
                error_code: None,
                error_message: Some(e.to_string()),
            }],
        },
    }
}

async fn execute_batch(
    client: &dataverse_lib::DataverseClient,
    item: &QueueItem,
    batch: dataverse_lib::api::Batch,
    started_at: chrono::DateTime<Utc>,
) -> ExecutionResult {
    // Extract content_ids from original operations for correlation
    let original_content_ids: Vec<Option<String>> = batch
        .items()
        .iter()
        .flat_map(|batch_item| match batch_item {
            BatchItem::Operation(op) => vec![op.content_id().map(|s| s.to_string())],
            BatchItem::Changeset(cs) => cs
                .operations()
                .iter()
                .map(|op| op.content_id().map(|s| s.to_string()))
                .collect(),
        })
        .collect();

    let result = client.execute_batch(batch).await;
    let completed_at = Utc::now();
    let duration_ms = (completed_at - started_at).num_milliseconds();

    match result {
        Ok(results) => {
            // Count successes and failures, and collect per-operation results
            let mut success_count = 0i32;
            let mut failure_count = 0i32;
            let mut errors = Vec::new();
            let mut operation_results = Vec::new();
            let mut op_index = 0i32;

            for item_result in results.iter() {
                match item_result {
                    BatchItemResult::Operation(Ok(op_result)) => {
                        success_count += 1;
                        let (operation_type, result_data) = batch_operation_result_info(op_result);
                        operation_results.push(NewOperationResult {
                            execution_id: 0,
                            op_index,
                            content_id: original_content_ids
                                .get(op_index as usize)
                                .cloned()
                                .flatten(),
                            success: true,
                            operation_type: Some(operation_type.to_string()),
                            result_data,
                            error_status: None,
                            error_code: None,
                            error_message: None,
                        });
                        op_index += 1;
                    }
                    BatchItemResult::Operation(Err(e)) => {
                        failure_count += 1;
                        errors.push(e.to_string());
                        operation_results.push(NewOperationResult {
                            execution_id: 0,
                            op_index,
                            content_id: e.content_id.clone().or_else(|| {
                                original_content_ids
                                    .get(op_index as usize)
                                    .cloned()
                                    .flatten()
                            }),
                            success: false,
                            operation_type: None,
                            result_data: None,
                            error_status: Some(e.status as i32),
                            error_code: e.error_code.clone(),
                            error_message: Some(e.message.clone()),
                        });
                        op_index += 1;
                    }
                    BatchItemResult::Changeset(Ok(ops)) => {
                        for op_result in ops {
                            success_count += 1;
                            let (operation_type, result_data) =
                                batch_operation_result_info(op_result);
                            operation_results.push(NewOperationResult {
                                execution_id: 0,
                                op_index,
                                content_id: original_content_ids
                                    .get(op_index as usize)
                                    .cloned()
                                    .flatten(),
                                success: true,
                                operation_type: Some(operation_type.to_string()),
                                result_data,
                                error_status: None,
                                error_code: None,
                                error_message: None,
                            });
                            op_index += 1;
                        }
                    }
                    BatchItemResult::Changeset(Err(e)) => {
                        // Entire changeset failed - we don't know how many ops were in it
                        // so we record one failure with the changeset error
                        failure_count += 1;
                        errors.push(e.to_string());
                        operation_results.push(NewOperationResult {
                            execution_id: 0,
                            op_index,
                            content_id: e.content_id.clone(),
                            success: false,
                            operation_type: None,
                            result_data: None,
                            error_status: Some(e.status as i32),
                            error_code: e.error_code.clone(),
                            error_message: Some(e.message.clone()),
                        });
                        op_index += 1;
                    }
                }
            }

            let (status, exec_status) = if failure_count == 0 {
                (ItemStatus::Done, ExecutionStatus::Success)
            } else if success_count == 0 {
                (ItemStatus::Failed, ExecutionStatus::Failed)
            } else {
                (ItemStatus::PartiallyFailed, ExecutionStatus::PartialSuccess)
            };

            let error = if errors.is_empty() {
                None
            } else {
                Some(errors.join("\n---\n"))
            };

            ExecutionResult {
                status,
                record: NewExecutionRecord {
                    item_id: item.id,
                    started_at,
                    completed_at,
                    duration_ms,
                    status: exec_status,
                    error,
                    success_count,
                    failure_count,
                },
                operation_results,
            }
        }
        Err(e) => {
            // Entire batch request failed
            ExecutionResult {
                status: ItemStatus::Failed,
                record: NewExecutionRecord {
                    item_id: item.id,
                    started_at,
                    completed_at,
                    duration_ms,
                    status: ExecutionStatus::Failed,
                    error: Some(e.to_string()),
                    success_count: 0,
                    failure_count: 1,
                },
                operation_results: vec![],
            }
        }
    }
}

fn make_error_result(
    item: &QueueItem,
    started_at: chrono::DateTime<Utc>,
    error: ExecutionError,
) -> ExecutionResult {
    let completed_at = Utc::now();
    let duration_ms = (completed_at - started_at).num_milliseconds();

    ExecutionResult {
        status: ItemStatus::Failed,
        record: NewExecutionRecord {
            item_id: item.id,
            started_at,
            completed_at,
            duration_ms,
            status: ExecutionStatus::Failed,
            error: Some(error.to_string()),
            success_count: 0,
            failure_count: 1,
        },
        operation_results: vec![],
    }
}

/// Extract operation type and result data from a successful batch operation result.
fn batch_operation_result_info(result: &BatchOperationResult) -> (&'static str, Option<String>) {
    match result {
        BatchOperationResult::Created { id, .. } => {
            ("create", Some(format!(r#"{{"id":"{}"}}"#, id)))
        }
        BatchOperationResult::Retrieved(_) => ("retrieve", None),
        BatchOperationResult::Updated { .. } => ("update", None),
        BatchOperationResult::Deleted => ("delete", None),
        BatchOperationResult::Upserted { created, id, .. } => {
            let op_type = if *created {
                "upsert_create"
            } else {
                "upsert_update"
            };
            (op_type, Some(format!(r#"{{"id":"{}"}}"#, id)))
        }
        BatchOperationResult::Associated => ("associate", None),
        BatchOperationResult::Disassociated => ("disassociate", None),
        BatchOperationResult::LookupSet => ("set_lookup", None),
        BatchOperationResult::LookupCleared => ("clear_lookup", None),
    }
}

/// Extract operation type and result data from a successful single operation result.
fn single_operation_result_info(result: &OperationResult) -> (&'static str, Option<String>) {
    match result {
        OperationResult::Create(create_result) => {
            let id = create_result.id().ok();
            let result_data = id.map(|id| format!(r#"{{"id":"{}"}}"#, id));
            ("create", result_data)
        }
        OperationResult::Retrieve(_) => ("retrieve", None),
        OperationResult::Update(_) => ("update", None),
        OperationResult::Delete => ("delete", None),
        OperationResult::Upsert(upsert_result) => {
            use dataverse_lib::api::UpsertResult;
            match upsert_result {
                UpsertResult::Created(create_result) => {
                    let id = create_result.id().ok();
                    let result_data = id.map(|id| format!(r#"{{"id":"{}"}}"#, id));
                    ("upsert_create", result_data)
                }
                UpsertResult::Updated { id, .. } => {
                    ("upsert_update", Some(format!(r#"{{"id":"{}"}}"#, id)))
                }
            }
        }
        OperationResult::Associate => ("associate", None),
        OperationResult::Disassociate => ("disassociate", None),
        OperationResult::SetLookup => ("set_lookup", None),
        OperationResult::ClearLookup => ("clear_lookup", None),
    }
}

/// Execute a queue item and persist results.
///
/// This handles the full lifecycle: execute, update status, save execution record,
/// and publish the completion event. Designed to be spawned as a tokio task.
pub async fn execute_and_complete(item: QueueItem, repo: QueueRepository, gx: GlobalContext) {
    log::info!("Executing queue item {}: {}", item.id, item.description);

    let result = execute_item(&item, &gx).await;

    if let Err(e) = repo.update_status(item.id, result.status).await {
        log::error!("Failed to update item {} status: {}", item.id, e);
    }

    let error = result.record.error.clone();
    let mut operation_results = result.operation_results;

    match repo.insert_execution(result.record).await {
        Ok(execution_id) => {
            // Fill in execution_id for operation results and insert them
            if !operation_results.is_empty() {
                for op_result in &mut operation_results {
                    op_result.execution_id = execution_id;
                }
                if let Err(e) = repo.insert_operation_results(operation_results).await {
                    log::error!(
                        "Failed to save operation results for item {}: {}",
                        item.id,
                        e
                    );
                }
            }
        }
        Err(e) => {
            log::error!(
                "Failed to save execution record for item {}: {}",
                item.id,
                e
            );
        }
    }

    gx.publish(QueueItemCompleted {
        item_id: item.id,
        status: result.status,
        error,
    });

    log::info!(
        "Queue item {} completed with status {:?}",
        item.id,
        result.status
    );
}
