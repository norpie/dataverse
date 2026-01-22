//! Queue item execution logic.

use chrono::Utc;
use dataverse_lib::api::BatchItemResult;
use dataverse_lib::error::Error as DataverseError;
use rafter::GlobalContext;

use crate::systems::client_management::ClientManagement;
use crate::systems::client_management::GetClient;

use super::api::QueueItemCompleted;
use super::repository::NewExecutionRecord;
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
    let result = client.execute(operation).await;
    let completed_at = Utc::now();
    let duration_ms = (completed_at - started_at).num_milliseconds();

    match result {
        Ok(_) => ExecutionResult {
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
        },
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
        },
    }
}

async fn execute_batch(
    client: &dataverse_lib::DataverseClient,
    item: &QueueItem,
    batch: dataverse_lib::api::Batch,
    started_at: chrono::DateTime<Utc>,
) -> ExecutionResult {
    let result = client.execute_batch(batch).await;
    let completed_at = Utc::now();
    let duration_ms = (completed_at - started_at).num_milliseconds();

    match result {
        Ok(results) => {
            // Count successes and failures
            let mut success_count = 0i32;
            let mut failure_count = 0i32;
            let mut errors = Vec::new();

            for item_result in results.iter() {
                match item_result {
                    BatchItemResult::Operation(Ok(_)) => success_count += 1,
                    BatchItemResult::Operation(Err(e)) => {
                        failure_count += 1;
                        errors.push(e.to_string());
                    }
                    BatchItemResult::Changeset(Ok(ops)) => {
                        success_count += ops.len() as i32;
                    }
                    BatchItemResult::Changeset(Err(e)) => {
                        // Entire changeset failed
                        failure_count += 1;
                        errors.push(e.to_string());
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

    if let Err(e) = repo.insert_execution(result.record).await {
        log::error!(
            "Failed to save execution record for item {}: {}",
            item.id,
            e
        );
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
