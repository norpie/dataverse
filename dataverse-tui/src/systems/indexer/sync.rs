//! Sync task types and execution helpers for the metadata indexer.

use std::time::Duration;

use chrono::Utc;
use rafter::prelude::GlobalContext;

use super::repository::{IndexerRepository, SyncStatus};
use crate::systems::client_management::{
    AuthenticatedEnvironment, ClientManagement, GetAuthenticatedEnvironments, GetClient,
};

/// Default check interval in seconds.
pub const DEFAULT_CHECK_INTERVAL_SECS: u64 = 60;

/// Default refresh threshold as percentage (0-100).
pub const DEFAULT_REFRESH_THRESHOLD_PCT: u64 = 80;

// =============================================================================
// Sync Tasks
// =============================================================================

/// A single unit of sync work.
#[derive(Clone, Debug)]
pub enum SyncTask {
    /// Check if an environment needs syncing, add follow-up tasks if so.
    CheckEnvironment {
        env_id: i64,
        account_id: i64,
        env_name: String,
    },
    /// Fetch the list of all entities for an environment.
    FetchAllEntities {
        env_id: i64,
        account_id: i64,
        env_name: String,
    },
    /// Fetch full metadata for a single entity.
    FetchEntityMetadata {
        env_id: i64,
        account_id: i64,
        env_name: String,
        entity_name: String,
    },
    /// Fetch all global option sets for an environment.
    FetchAllOptionSets {
        env_id: i64,
        account_id: i64,
        env_name: String,
        /// Number of entities that were fetched.
        entities_count: u32,
    },
}

impl SyncTask {
    /// Get the environment ID for this task.
    pub fn env_id(&self) -> i64 {
        match self {
            Self::CheckEnvironment { env_id, .. }
            | Self::FetchAllEntities { env_id, .. }
            | Self::FetchEntityMetadata { env_id, .. }
            | Self::FetchAllOptionSets { env_id, .. } => *env_id,
        }
    }

    /// Get the environment name for this task.
    pub fn env_name(&self) -> &str {
        match self {
            Self::CheckEnvironment { env_name, .. }
            | Self::FetchAllEntities { env_name, .. }
            | Self::FetchEntityMetadata { env_name, .. }
            | Self::FetchAllOptionSets { env_name, .. } => env_name,
        }
    }

}

// =============================================================================
// Task Execution
// =============================================================================

/// Execute a single sync task, returning follow-up tasks to add to the queue.
pub async fn execute_task(
    task: &SyncTask,
    repository: &IndexerRepository,
    gx: &GlobalContext,
    refresh_threshold: f64,
) -> Result<Vec<SyncTask>, SyncError> {
    match task {
        SyncTask::CheckEnvironment {
            env_id,
            account_id,
            env_name,
        } => {
            execute_check_environment(*env_id, *account_id, env_name, gx, refresh_threshold).await
        }
        SyncTask::FetchAllEntities {
            env_id,
            account_id,
            env_name,
        } => execute_fetch_all_entities(*env_id, *account_id, env_name, repository, gx, refresh_threshold).await,
        SyncTask::FetchEntityMetadata {
            env_id,
            account_id,
            entity_name,
            ..
        } => execute_fetch_entity_metadata(*env_id, *account_id, entity_name, gx).await,
        SyncTask::FetchAllOptionSets {
            env_id,
            account_id,
            env_name,
            entities_count,
        } => {
            execute_fetch_all_optionsets(
                *env_id,
                *account_id,
                env_name,
                *entities_count,
                repository,
                gx,
            )
            .await
        }
    }
}

/// Check if an environment's cache needs refresh.
async fn execute_check_environment(
    env_id: i64,
    account_id: i64,
    env_name: &str,
    gx: &GlobalContext,
    refresh_threshold: f64,
) -> Result<Vec<SyncTask>, SyncError> {
    log::debug!("[Indexer] Checking cache for environment: {}", env_name);

    let client_info = gx
        .request_system::<ClientManagement, GetClient>(GetClient { account_id, env_id })
        .await
        .map_err(|e| SyncError::Request(format!("Failed to request client: {}", e)))?
        .map_err(|e| SyncError::Client(format!("Failed to get client: {}", e)))?;

    let client = &client_info.client;

    let Some(cache) = client.cache() else {
        log::warn!("[Indexer] No cache provider for environment {}", env_name);
        return Ok(vec![]);
    };

    let cache_config = client.cache_config();
    let entries = cache.get_all().await;
    let now = Utc::now();

    // Check if any cache entry needs refresh
    // - all_entities list
    // - all_global_optionsets list  
    // - any individual entity metadata (entity_full:*, entity_core:*)
    let mut needs_entities = false;
    let mut needs_optionsets = false;
    let mut has_entity_list = false;
    let mut has_optionset_list = false;

    for entry in &entries {
        let elapsed_ratio = if entry.expires_at > now {
            let ttl = get_ttl_for_key(&entry.key, cache_config);
            let time_until_expiry = (entry.expires_at - now).num_seconds().max(0) as f64;
            let ttl_secs = ttl.as_secs_f64();
            if ttl_secs > 0.0 {
                1.0 - (time_until_expiry / ttl_secs)
            } else {
                1.0
            }
        } else {
            1.0 // Expired
        };

        let is_stale = elapsed_ratio >= refresh_threshold;

        if entry.key == "all_entities" {
            has_entity_list = true;
            if is_stale {
                needs_entities = true;
            }
        } else if entry.key == "all_global_optionsets" {
            has_optionset_list = true;
            if is_stale {
                needs_optionsets = true;
            }
        } else if entry.key.starts_with("entity_full:") || entry.key.starts_with("entity_core:") {
            // Any stale entity metadata triggers a full refresh
            if is_stale {
                needs_entities = true;
            }
        }
    }

    // If we don't have the lists cached at all, we need to fetch them
    if !has_entity_list {
        needs_entities = true;
    }
    if !has_optionset_list {
        needs_optionsets = true;
    }

    if !needs_entities && !needs_optionsets {
        log::debug!("[Indexer] Cache fresh for environment: {}", env_name);
        return Ok(vec![]);
    }

    log::info!(
        "[Indexer] Cache stale for {} (entities: {}, optionsets: {})",
        env_name,
        needs_entities,
        needs_optionsets
    );

    // Add FetchAllEntities task (which will chain to entity metadata and optionsets)
    Ok(vec![SyncTask::FetchAllEntities {
        env_id,
        account_id,
        env_name: env_name.to_string(),
    }])
}

/// Fetch all entities for an environment and create follow-up tasks.
/// Only creates tasks for entities that are missing or stale in the cache.
async fn execute_fetch_all_entities(
    env_id: i64,
    account_id: i64,
    env_name: &str,
    repository: &IndexerRepository,
    gx: &GlobalContext,
    refresh_threshold: f64,
) -> Result<Vec<SyncTask>, SyncError> {
    log::debug!("[Indexer] Fetching all entities for {}", env_name);

    // Update repository status to syncing
    let _ = repository
        .upsert_env_sync(env_id, SyncStatus::Syncing, None, None, 0, 0, 0)
        .await;

    let client_info = gx
        .request_system::<ClientManagement, GetClient>(GetClient { account_id, env_id })
        .await
        .map_err(|e| SyncError::Request(format!("Failed to request client: {}", e)))?
        .map_err(|e| SyncError::Client(format!("Failed to get client: {}", e)))?;

    let client = &client_info.client;

    // Use cache if available - only fetches from API if cache is stale/missing
    let entities = client
        .metadata()
        .all_entities()
        .await
        .map_err(|e| SyncError::Api(format!("Failed to fetch all entities: {}", e)))?;

    let total = entities.len();
    log::debug!("[Indexer] Fetched {} entities for {}", total, env_name);

    // Check cache to find which entities need refresh
    let cache = client.cache();
    let cache_config = client.cache_config();
    let now = Utc::now();

    let mut stale_entities = Vec::new();
    
    if let Some(cache) = cache {
        let cache_entries = cache.get_all().await;
        
        for entity in &entities {
            let cache_key = format!("entity_full:{}", entity.logical_name);
            let entry = cache_entries.iter().find(|e| e.key == cache_key);
            
            let needs_refresh = match entry {
                None => true, // Not in cache
                Some(entry) => {
                    if entry.expires_at <= now {
                        true // Expired
                    } else {
                        // Check if past threshold
                        let ttl = cache_config.entity_metadata_ttl;
                        let time_until_expiry = (entry.expires_at - now).num_seconds().max(0) as f64;
                        let ttl_secs = ttl.as_secs_f64();
                        let elapsed_ratio = if ttl_secs > 0.0 {
                            1.0 - (time_until_expiry / ttl_secs)
                        } else {
                            1.0
                        };
                        elapsed_ratio >= refresh_threshold
                    }
                }
            };
            
            if needs_refresh {
                stale_entities.push(entity.logical_name.clone());
            }
        }
    } else {
        // No cache, fetch all
        stale_entities = entities.iter().map(|e| e.logical_name.clone()).collect();
    }

    let stale_count = stale_entities.len();
    log::info!(
        "[Indexer] {} of {} entities need refresh for {}",
        stale_count,
        total,
        env_name
    );

    // Create tasks only for stale entities
    let mut tasks: Vec<SyncTask> = stale_entities
        .into_iter()
        .map(|entity_name| SyncTask::FetchEntityMetadata {
            env_id,
            account_id,
            env_name: env_name.to_string(),
            entity_name,
        })
        .collect();

    // Add the optionsets task at the end (always refresh with entity list)
    tasks.push(SyncTask::FetchAllOptionSets {
        env_id,
        account_id,
        env_name: env_name.to_string(),
        entities_count: total as u32,
    });

    Ok(tasks)
}

/// Fetch full metadata for a single entity.
async fn execute_fetch_entity_metadata(
    env_id: i64,
    account_id: i64,
    entity_name: &str,
    gx: &GlobalContext,
) -> Result<Vec<SyncTask>, SyncError> {
    log::trace!("[Indexer] Fetching metadata for entity: {}", entity_name);

    let client_info = gx
        .request_system::<ClientManagement, GetClient>(GetClient { account_id, env_id })
        .await
        .map_err(|e| SyncError::Request(format!("Failed to request client: {}", e)))?
        .map_err(|e| SyncError::Client(format!("Failed to get client: {}", e)))?;

    // Fetch and cache - errors are logged but don't fail the whole sync
    match client_info
        .client
        .metadata()
        .entity(entity_name)
        .bypass_cache()
        .await
    {
        Ok(_) => {
            log::trace!("[Indexer] Cached metadata for entity: {}", entity_name);
        }
        Err(e) => {
            log::warn!(
                "[Indexer] Failed to fetch metadata for entity {}: {}",
                entity_name,
                e
            );
        }
    }

    // No follow-up tasks
    Ok(vec![])
}

/// Fetch all global option sets for an environment.
async fn execute_fetch_all_optionsets(
    env_id: i64,
    account_id: i64,
    env_name: &str,
    entities_count: u32,
    repository: &IndexerRepository,
    gx: &GlobalContext,
) -> Result<Vec<SyncTask>, SyncError> {
    log::debug!("[Indexer] Fetching all global option sets for {}", env_name);

    let client_info = gx
        .request_system::<ClientManagement, GetClient>(GetClient { account_id, env_id })
        .await
        .map_err(|e| SyncError::Request(format!("Failed to request client: {}", e)))?
        .map_err(|e| SyncError::Client(format!("Failed to get client: {}", e)))?;

    let optionsets = client_info
        .client
        .metadata()
        .all_global_option_sets()
        .bypass_cache()
        .await
        .map_err(|e| SyncError::Api(format!("Failed to fetch global option sets: {}", e)))?;

    let optionsets_count = optionsets.len() as i64;
    log::debug!(
        "[Indexer] Fetched {} global option sets for {}",
        optionsets_count,
        env_name
    );

    // Update repository with success
    let completed_at = Utc::now();
    let _ = repository
        .upsert_env_sync(
            env_id,
            SyncStatus::Idle,
            Some(completed_at),
            None,
            entities_count as i64,
            optionsets_count,
            0, // total_attributes_count - we don't track this per-entity anymore
        )
        .await;

    // Add sync log entry
    let _ = repository
        .add_sync_log(
            env_id,
            completed_at, // Use completed_at as started_at approximation
            Some(completed_at),
            "success".to_string(),
            None,
            entities_count as i64,
            optionsets_count,
        )
        .await;

    log::info!(
        "[Indexer] Completed sync for {} ({} entities, {} optionsets)",
        env_name,
        entities_count,
        optionsets_count
    );

    // No follow-up tasks - this environment is done
    Ok(vec![])
}

// =============================================================================
// Helpers
// =============================================================================

/// Get the TTL for a cache key based on its prefix.
fn get_ttl_for_key(key: &str, config: &dataverse_lib::cache::CacheConfig) -> Duration {
    if key == "all_entities" {
        config.entity_list_ttl
    } else if key == "all_global_optionsets" {
        config.global_optionset_ttl
    } else if key.starts_with("entity_full:") || key.starts_with("entity_core:") {
        config.entity_metadata_ttl
    } else if key.starts_with("attributes:") || key.starts_with("attribute:") {
        config.attribute_metadata_ttl
    } else if key.starts_with("relationship:") {
        config.relationship_ttl
    } else if key.starts_with("global_optionset:") {
        config.global_optionset_ttl
    } else {
        // Default to entity list TTL for unknown keys
        config.entity_list_ttl
    }
}

/// Get all authenticated environments as CheckEnvironment tasks.
pub async fn get_check_tasks(gx: &GlobalContext) -> Vec<SyncTask> {
    let environments: Vec<AuthenticatedEnvironment> = gx
        .request_system::<ClientManagement, GetAuthenticatedEnvironments>(
            GetAuthenticatedEnvironments,
        )
        .await
        .unwrap_or_default();

    environments
        .into_iter()
        .map(|env| SyncTask::CheckEnvironment {
            env_id: env.env_id,
            account_id: env.account_id,
            env_name: env.environment_name,
        })
        .collect()
}

// =============================================================================
// Errors
// =============================================================================

/// Errors that can occur during sync.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("request error: {0}")]
    Request(String),

    #[error("client error: {0}")]
    Client(String),

    #[error("API error: {0}")]
    Api(String),

    #[error("repository error: {0}")]
    Repository(#[from] super::repository::RepositoryError),
}
