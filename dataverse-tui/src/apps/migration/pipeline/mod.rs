//! Data pipeline for migration execution.
//!
//! This module provides the building blocks for fetching source/target data,
//! building caches, and executing transforms against real Dataverse records.
//!
//! The pipeline operates at the **phase** level to enable cross-mapping
//! deduplication of find cache fetches.

pub mod analysis;
pub mod fetch;

use std::collections::HashSet;

// =============================================================================
// Fetch Plan Types
// =============================================================================

/// Analysis output for one entity mapping — describes what data needs to be fetched.
#[derive(Debug, Clone)]
pub struct FetchPlan {
    /// Source entity fetch specification.
    pub source: SourceFetchSpec,
    /// Target entity fetch specification (needed for match config).
    pub target: Option<TargetFetchSpec>,
    /// Find cache specifications — one per find entity referenced.
    pub find_caches: Vec<FindCacheSpec>,
}

/// Describes what fields to fetch from the source entity.
#[derive(Debug, Clone)]
pub struct SourceFetchSpec {
    /// Source entity logical name.
    pub entity: String,
    /// Fields to include in `$select`.
    pub select: HashSet<String>,
    /// Navigation properties to `$expand` (for multi-segment paths).
    pub expands: Vec<ExpandSpec>,
}

/// Describes what fields to fetch from the target entity.
#[derive(Debug, Clone)]
pub struct TargetFetchSpec {
    /// Target entity logical name.
    pub entity: String,
    /// Fields to include in `$select`.
    pub select: HashSet<String>,
    /// Navigation properties to `$expand`.
    pub expands: Vec<ExpandSpec>,
}

/// Describes what fields to fetch for a find cache entity.
#[derive(Debug, Clone)]
pub struct FindCacheSpec {
    /// Entity logical name (e.g., "capacity").
    pub entity: String,
    /// Fields to include in `$select`.
    pub select: HashSet<String>,
}

/// A navigation property expansion with nested select/expand.
#[derive(Debug, Clone)]
pub struct ExpandSpec {
    /// Navigation property name (e.g., "parentaccountid").
    pub nav_property: String,
    /// Fields to select within this expansion.
    pub select: HashSet<String>,
    /// Nested expansions (for 3+ level paths).
    pub nested: Vec<ExpandSpec>,
}
