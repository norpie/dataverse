//! Domain structures mirroring database tables.

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

use crate::widgets::filter_builder::FilterNode;

use super::condition::Condition;
use super::enums::*;
use super::transform::TransformData;

// =============================================================================
// Migration
// =============================================================================

/// A migration configuration.
#[derive(Debug, Clone)]
pub struct Migration {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub source_environment_id: i64,
    pub target_environment_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Summary information for migration list view.
#[derive(Debug, Clone)]
pub struct MigrationSummary {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub source_environment_id: i64,
    pub target_environment_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// =============================================================================
// Phase
// =============================================================================

/// A phase within a migration.
#[derive(Debug, Clone)]
pub struct Phase {
    pub id: i64,
    pub migration_id: i64,
    pub order: i32,
    pub name: String,
    pub mode: Mode,
    pub lua_script: Option<String>,
}

// =============================================================================
// Entity Mapping
// =============================================================================

/// Configuration for mapping a source entity to a target entity.
#[derive(Debug, Clone)]
pub struct EntityMapping {
    pub id: i64,
    pub phase_id: i64,
    pub order: i32,
    pub name: String,
    pub source_entity: String,
    pub target_entity: String,
    pub mode: Mode,
    pub lua_script: Option<String>,
    pub match_strategy: MatchStrategy,
    pub match_find_config: Option<FindConfig>,
    pub match_lua_script: Option<String>,
    pub no_match_fallback: NoMatchFallback,
    pub orphan_strategy: OrphanStrategy,
    pub create_pass_enabled: bool,
    pub activate_pass_enabled: bool,
    pub update_pass_enabled: bool,
    pub delete_pass_enabled: bool,
    pub deactivate_pass_enabled: bool,
    pub associate_pass_enabled: bool,
    pub disassociate_pass_enabled: bool,
    pub source_filter: Option<FilterNode>,
    pub target_filter: Option<FilterNode>,
    pub test_guids: Option<Vec<String>>,
}

/// Configuration for the find-based match strategy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FindConfig {
    /// The find expression used for matching.
    /// This will be represented by a transform chain in the DB.
    pub transform_chain_root_id: Option<i64>,
}

// =============================================================================
// Variable
// =============================================================================

/// A computed variable scoped to an entity mapping.
#[derive(Debug, Clone)]
pub struct Variable {
    pub id: i64,
    pub entity_mapping_id: i64,
    pub order: i32,
    pub name: String,
    /// The declared output type of this variable.
    pub declared_type: dataverse_lib::model::ValueType,
}

// =============================================================================
// Field Mapping
// =============================================================================

/// Mapping for a single target field.
#[derive(Debug, Clone)]
pub struct FieldMapping {
    pub id: i64,
    pub entity_mapping_id: i64,
    pub order: i32,
    pub target_field: String,
}

// =============================================================================
// Transform
// =============================================================================

/// A transform operation.
#[derive(Debug, Clone)]
pub struct Transform {
    pub id: i64,
    pub entity_mapping_id: i64,
    pub parent_type: ParentType,
    pub parent_id: i64,
    pub order: i32,
    pub data: TransformData,
}

// =============================================================================
// Match Branch
// =============================================================================

/// A branch within a match transform.
#[derive(Debug, Clone)]
pub struct MatchBranch {
    pub id: i64,
    pub transform_id: i64,
    pub order: i32,
    pub condition: Condition,
}

// =============================================================================
// Coalesce Chain
// =============================================================================

/// A fallback chain within a coalesce transform.
#[derive(Debug, Clone)]
pub struct CoalesceChain {
    pub id: i64,
    pub transform_id: i64,
    pub order: i32,
}

// =============================================================================
// Find Condition
// =============================================================================

/// A condition within a find transform (where-clause mode).
#[derive(Debug, Clone)]
pub struct FindCondition {
    pub id: i64,
    pub transform_id: i64,
    pub target_field: String,
    pub order: i32,
}

// =============================================================================
// Match Condition
// =============================================================================

/// A condition within a match config (find mode).
/// Specifies a target field to match on, with a transform chain
/// (stored as transforms with ParentType::MatchCondition) to compute the value.
#[derive(Debug, Clone)]
pub struct MatchCondition {
    pub id: i64,
    pub entity_mapping_id: i64,
    pub target_field: String,
    pub order: i32,
}

// =============================================================================
// Phase Run
// =============================================================================

/// Execution history for a phase.
#[derive(Debug, Clone)]
pub struct PhaseRun {
    pub id: i64,
    pub phase_id: i64,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: PhaseRunStatus,
    pub queue_item_ids: Option<Vec<i64>>,
    pub error: Option<String>,
}
