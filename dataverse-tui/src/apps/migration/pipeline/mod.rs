//! Data pipeline for migration execution.
//!
//! This module provides the building blocks for fetching source/target data,
//! building caches, and executing transforms against real Dataverse records.
//!
//! The pipeline operates at the **phase** level to enable cross-mapping
//! deduplication of find cache fetches.
//!
//! ## Flow
//!
//! The UI layer orchestrates — the pipeline provides sync building blocks:
//!
//! 1. **`analyze_phase`** — analyze all mappings, merge find caches
//! 2. **`build_fetch_tasks`** — convert plan into OData fetch tasks
//! 3. *(UI fetches data via `ODataFetchModal`)*
//! 4. **`build_find_cache`** — populate `LiveFindCache` from fetch results
//! 5. **`execute_mapping`** — run transforms per source record

pub mod analysis;
pub mod cache;
pub mod fetch;

use std::collections::HashSet;

use dataverse_lib::model::Entity;
use dataverse_lib::model::Record;
use dataverse_lib::DataverseClient;
use rayon::prelude::*;

use self::analysis::AnalysisInput;
use self::cache::LiveFindCache;
use self::fetch::build_find_cache_tasks;
use self::fetch::build_source_task;
use self::fetch::build_target_task;
use self::fetch::into_fetch_task;
use self::fetch::merge_find_cache_specs;
use self::fetch::BuildError;
use self::fetch::FetchTaskConfig;

use super::comparison::compare_mapping;
use super::comparison::CompareInput;
use super::comparison::MappingComparison;
use super::engine::record::execute_record;
use super::engine::record::RecordResult;
use super::engine::transforms::extract_placeholders;
use super::engine::ChainChildren;
use super::engine::ChainItem;
use super::engine::FindCache;
use super::engine::PathCache;
use super::engine::SystemVars;
use super::types::Condition;
use super::types::Expr;
use super::types::MatchStrategy;
use super::types::NoMatchFallback;
use super::types::OrphanStrategy;
use super::types::TransformData;

use crate::modals::odata_fetch::ODataFetchTask;
use crate::widgets::filter_builder::FilterNode;

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
    /// Navigation properties to `$expand` (for dotted target_field paths).
    pub expands: Vec<ExpandSpec>,
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

// =============================================================================
// Pipeline Input Types
// =============================================================================

/// Input for a single entity mapping within a phase.
pub struct MappingInput<'a> {
    /// Source entity logical name.
    pub source_entity: &'a str,
    /// Target entity logical name.
    pub target_entity: &'a str,
    /// Primary key field for the source entity (from metadata).
    pub source_primary_key: &'a str,
    /// Primary key field for the target entity (from metadata).
    pub target_primary_key: &'a str,
    /// Materialized field mapping chains: (target_field, chain).
    pub field_mappings: &'a [(String, Vec<ChainItem>)],
    /// Materialized variable chains: (variable_name, chain).
    pub variables: &'a [(String, Vec<ChainItem>)],
    /// Materialized match config chain (if match strategy is Find).
    pub match_config_chain: Option<&'a [ChainItem]>,
    /// Source filter from entity mapping config.
    pub source_filter: Option<&'a FilterNode>,
    /// Target filter from entity mapping config.
    pub target_filter: Option<&'a FilterNode>,
    /// Test GUIDs — if present, completely override source filter.
    pub test_guids: Option<&'a [String]>,
    /// Display name for the mapping (used in fetch task labels).
    pub mapping_name: &'a str,
}

// =============================================================================
// Phase-Level Analysis
// =============================================================================

/// Phase-level fetch plan — analysis output for all mappings in a phase.
pub struct PhaseFetchPlan {
    /// Per-mapping fetch plans (same order as input).
    pub mapping_plans: Vec<FetchPlan>,
    /// Merged find cache specs (deduplicated across all mappings).
    pub merged_find_caches: Vec<FindCacheSpec>,
}

/// Analyze all mappings in a phase and produce a unified fetch plan.
///
/// Runs `analyze_mapping` for each input, then merges find cache specs
/// across all mappings so each entity is only fetched once.
pub fn analyze_phase(inputs: &[MappingInput<'_>]) -> PhaseFetchPlan {
    let mapping_plans: Vec<FetchPlan> = inputs
        .iter()
        .map(|input| {
            analysis::analyze_mapping(&AnalysisInput {
                source_entity: input.source_entity,
                target_entity: input.target_entity,
                source_primary_key: input.source_primary_key,
                target_primary_key: input.target_primary_key,
                field_mappings: input.field_mappings,
                variables: input.variables,
                match_config_chain: input.match_config_chain,
            })
        })
        .collect();

    let all_find_caches: Vec<Vec<FindCacheSpec>> = mapping_plans
        .iter()
        .map(|plan| plan.find_caches.clone())
        .collect();
    let merged_find_caches = merge_find_cache_specs(all_find_caches);

    PhaseFetchPlan {
        mapping_plans,
        merged_find_caches,
    }
}

// =============================================================================
// Fetch Task Building
// =============================================================================

/// Built fetch tasks for a phase, ready for the `ODataFetchModal`.
pub struct PhaseFetchTasks {
    /// Source fetch tasks — one per mapping. Index matches `MappingInput` order.
    pub source_tasks: Vec<ODataFetchTask>,
    /// Target fetch tasks — one per mapping that has a target spec.
    /// Each entry is `(mapping_index, task)`.
    pub target_tasks: Vec<(usize, ODataFetchTask)>,
    /// Find cache fetch tasks — one per unique find entity (after merging).
    pub find_cache_tasks: Vec<ODataFetchTask>,
}

/// Build all fetch tasks for a phase.
///
/// Takes the phase plan and mapping inputs (for filters/GUIDs/clients),
/// and produces `ODataFetchTask`s ready for the fetch modal.
pub fn build_phase_fetch_tasks(
    phase_plan: &PhaseFetchPlan,
    inputs: &[MappingInput<'_>],
    source_client: &DataverseClient,
    target_client: &DataverseClient,
) -> Result<PhaseFetchTasks, BuildError> {
    let mut source_tasks = Vec::new();
    let mut target_tasks = Vec::new();

    for (i, (plan, input)) in phase_plan
        .mapping_plans
        .iter()
        .zip(inputs.iter())
        .enumerate()
    {
        let config = FetchTaskConfig {
            plan,
            source_primary_key: input.source_primary_key,
            target_primary_key: input.target_primary_key,
            source_filter: input.source_filter,
            target_filter: input.target_filter,
            test_guids: input.test_guids,
            mapping_name: input.mapping_name,
        };

        let source_query = build_source_task(&config)?;
        source_tasks.push(into_fetch_task(
            format!("Source: {}", input.mapping_name),
            source_query,
            source_client.clone(),
        ));

        if let Some(target_query) = build_target_task(&config)? {
            target_tasks.push((
                i,
                into_fetch_task(
                    format!("Target: {}", input.mapping_name),
                    target_query,
                    target_client.clone(),
                ),
            ));
        }
    }

    let find_cache_queries = build_find_cache_tasks(&phase_plan.merged_find_caches);
    let find_cache_tasks = find_cache_queries
        .into_iter()
        .zip(phase_plan.merged_find_caches.iter())
        .map(|(query, spec)| {
            into_fetch_task(
                format!("Find cache: {}", spec.entity),
                query,
                target_client.clone(),
            )
        })
        .collect();

    Ok(PhaseFetchTasks {
        source_tasks,
        target_tasks,
        find_cache_tasks,
    })
}

/// Collect all tasks from a `PhaseFetchTasks` into a single flat list
/// for the fetch modal. Returns the tasks and an index map to recover
/// which result belongs to which category.
///
/// Order: source tasks, then target tasks, then find cache tasks.
pub fn collect_all_tasks(tasks: PhaseFetchTasks) -> (Vec<ODataFetchTask>, FetchTaskIndex) {
    let source_count = tasks.source_tasks.len();
    let target_mapping_indices: Vec<usize> = tasks.target_tasks.iter().map(|(i, _)| *i).collect();
    let target_count = tasks.target_tasks.len();
    let find_cache_count = tasks.find_cache_tasks.len();

    let mut all_tasks = Vec::with_capacity(source_count + target_count + find_cache_count);
    all_tasks.extend(tasks.source_tasks);
    all_tasks.extend(tasks.target_tasks.into_iter().map(|(_, task)| task));
    all_tasks.extend(tasks.find_cache_tasks);

    let index = FetchTaskIndex {
        source_count,
        target_count,
        target_mapping_indices,
        find_cache_count,
    };

    (all_tasks, index)
}

/// Index map for recovering fetch results from a flat task list.
pub struct FetchTaskIndex {
    /// Number of source tasks (indices 0..source_count).
    pub source_count: usize,
    /// Number of target tasks.
    pub target_count: usize,
    /// For each target task, which mapping index it belongs to.
    pub target_mapping_indices: Vec<usize>,
    /// Number of find cache tasks.
    pub find_cache_count: usize,
}

/// Fetch results split by category.
#[derive(Default)]
pub struct PhaseFetchResults {
    /// Source records per mapping (same order as inputs).
    pub source_records: Vec<Vec<Record>>,
    /// Target records per mapping index that had a target spec.
    pub target_records: Vec<(usize, Vec<Record>)>,
    /// Find cache records per entity (same order as merged_find_caches).
    pub find_cache_records: Vec<Vec<Record>>,
}

/// Split flat fetch results back into categorized results using the index.
pub fn split_fetch_results(results: Vec<Vec<Record>>, index: &FetchTaskIndex) -> PhaseFetchResults {
    let mut iter = results.into_iter();

    let source_records: Vec<Vec<Record>> = iter.by_ref().take(index.source_count).collect();

    let target_records: Vec<(usize, Vec<Record>)> = iter
        .by_ref()
        .take(index.target_count)
        .zip(index.target_mapping_indices.iter())
        .map(|(records, &mapping_idx)| (mapping_idx, records))
        .collect();

    let find_cache_records: Vec<Vec<Record>> = iter.by_ref().take(index.find_cache_count).collect();

    PhaseFetchResults {
        source_records,
        target_records,
        find_cache_records,
    }
}

// =============================================================================
// Cache Building
// =============================================================================

/// Build a `LiveFindCache` from fetch results.
///
/// Maps each find cache entity's records into the cache.
pub fn build_find_cache(
    find_cache_records: Vec<Vec<Record>>,
    find_cache_specs: &[FindCacheSpec],
) -> LiveFindCache {
    let mut cache = LiveFindCache::new();

    for (records, spec) in find_cache_records.into_iter().zip(find_cache_specs.iter()) {
        cache.insert_records(&spec.entity, records);
    }

    cache
}

// =============================================================================
// Record Execution
// =============================================================================

/// Result of executing all records for a single entity mapping.
#[derive(Default)]
pub struct MappingResult {
    /// Per-record transform results (same order as source records).
    pub record_results: Vec<RecordResult>,
}

/// Execute transforms for all source records of a single entity mapping.
///
/// Calls `execute_record` for each source record with the shared
/// variables, field mappings, system vars, and find cache.
///
/// Builds a path cache once before iteration to avoid re-parsing path
/// strings for every record.
pub fn execute_mapping(
    source_records: &[Record],
    variables: &[(String, Vec<ChainItem>)],
    field_mappings: &[(String, Vec<ChainItem>)],
    source_entity: &str,
    target_entity: &str,
    find_cache: &dyn FindCache,
) -> MappingResult {
    // Build path cache once for all records in this mapping
    let path_cache = build_path_cache(variables, field_mappings);

    let record_results = source_records
        .par_iter()
        .enumerate()
        .map(|(index, source)| {
            let system_vars = SystemVars::new(
                Entity::logical(source_entity),
                Entity::logical(target_entity),
                index,
            );
            execute_record(
                source,
                variables,
                field_mappings,
                system_vars,
                find_cache,
                &path_cache,
            )
        })
        .collect();

    MappingResult { record_results }
}

/// Build a path cache from all chains in variables and field mappings.
///
/// Walks all chain items recursively, extracting path strings from Copy,
/// Format, Guard, Match conditions, and Find conditions. Pre-parses them
/// into `PathExpr` so each record doesn't re-parse the same strings.
fn build_path_cache(
    variables: &[(String, Vec<ChainItem>)],
    field_mappings: &[(String, Vec<ChainItem>)],
) -> PathCache {
    let mut cache = PathCache::new();

    for (_, chain) in variables {
        collect_paths_from_chain(chain, &mut cache);
    }
    for (_, chain) in field_mappings {
        collect_paths_from_chain(chain, &mut cache);
    }

    cache
}

/// Recursively collect and pre-parse all path strings from a chain.
fn collect_paths_from_chain(chain: &[ChainItem], cache: &mut PathCache) {
    for item in chain {
        collect_paths_from_data(&item.data, cache);
        collect_paths_from_children(&item.children, cache);
    }
}

/// Extract path strings from a single TransformData and pre-parse them.
fn collect_paths_from_data(data: &TransformData, cache: &mut PathCache) {
    match data {
        TransformData::Copy { path } => {
            // Handle coalesce syntax: "a ?? b ?? c"
            if path.contains("??") {
                for alt in path.split("??").map(|s| s.trim()) {
                    if !alt.is_empty() {
                        try_cache_path(alt, cache);
                    }
                }
            } else {
                try_cache_path(path, cache);
            }
        }
        TransformData::Format { template } => {
            // Extract placeholders from format template
            for placeholder in extract_placeholders(template) {
                // Handle coalesce within placeholders
                if placeholder.contains("??") {
                    for alt in placeholder.split("??").map(|s| s.trim()) {
                        if !alt.is_empty() {
                            try_cache_path(alt, cache);
                        }
                    }
                } else {
                    try_cache_path(&placeholder, cache);
                }
            }
        }
        TransformData::Guard { condition } => {
            collect_paths_from_condition(condition, cache);
        }
        _ => {}
    }
}

/// Extract path strings from conditions (Guard, Match branches).
fn collect_paths_from_condition(condition: &Condition, cache: &mut PathCache) {
    match condition {
        Condition::And(conditions) | Condition::Or(conditions) => {
            for c in conditions {
                collect_paths_from_condition(c, cache);
            }
        }
        Condition::Not(inner) => collect_paths_from_condition(inner, cache),
        Condition::Compare { left, right, .. } => {
            collect_paths_from_expr(left, cache);
            collect_paths_from_expr(right, cache);
        }
        Condition::IsNull(expr) | Condition::IsNotNull(expr) => {
            collect_paths_from_expr(expr, cache);
        }
        Condition::Contains { value, substring } => {
            collect_paths_from_expr(value, cache);
            collect_paths_from_expr(substring, cache);
        }
        Condition::StartsWith { value, prefix } => {
            collect_paths_from_expr(value, cache);
            collect_paths_from_expr(prefix, cache);
        }
        Condition::EndsWith { value, suffix } => {
            collect_paths_from_expr(value, cache);
            collect_paths_from_expr(suffix, cache);
        }
    }
}

/// Extract path strings from expressions.
fn collect_paths_from_expr(expr: &Expr, cache: &mut PathCache) {
    if let Expr::Path(path) = expr {
        try_cache_path(path, cache);
    }
}

/// Extract paths from child chains recursively.
fn collect_paths_from_children(children: &ChainChildren, cache: &mut PathCache) {
    match children {
        ChainChildren::None => {}
        ChainChildren::Fallback(chain) => collect_paths_from_chain(chain, cache),
        ChainChildren::Branches(branches, default) => {
            for branch in branches {
                collect_paths_from_condition(&branch.condition, cache);
                collect_paths_from_chain(&branch.chain, cache);
            }
            if let Some(default) = default {
                collect_paths_from_chain(default, cache);
            }
        }
        ChainChildren::Alternatives(alts) => {
            for alt in alts {
                collect_paths_from_chain(alt, cache);
            }
        }
        ChainChildren::FindConditions(conditions, default) => {
            for cond in conditions {
                collect_paths_from_chain(&cond.source_chain, cache);
            }
            if let Some(default) = default {
                collect_paths_from_chain(default, cache);
            }
        }
    }
}

/// Try to parse a path string and cache it. Silently skips parse errors
/// (they'll be caught at execution time with a proper error message).
fn try_cache_path(path: &str, cache: &mut PathCache) {
    if cache.contains_key(path) {
        return;
    }
    if let Ok(parsed) = crate::apps::migration::validation::parse_path(path) {
        cache.insert(path.to_string(), parsed);
    }
}

// =============================================================================
// Comparison
// =============================================================================

/// Input for comparing a mapping's results against target records.
pub struct ComparisonInput<'a> {
    /// Source records (same order as MappingResult.record_results).
    pub source_records: &'a [Record],
    /// Transform results from `execute_mapping` (owned — consumed by comparison).
    pub mapping_result: MappingResult,
    /// Target records fetched from the target environment.
    pub target_records: &'a [Record],
    /// Match strategy from entity mapping config.
    pub strategy: MatchStrategy,
    /// Source entity primary key field name.
    pub source_primary_key: &'a str,
    /// Target entity primary key field name.
    pub target_primary_key: &'a str,
    /// Materialized match conditions: (target_field, source_chain).
    pub match_conditions: &'a [(String, Vec<ChainItem>)],
    /// Source entity logical name.
    pub source_entity: &'a str,
    /// Target entity logical name.
    pub target_entity: &'a str,
    /// Find cache for resolving find() in match condition chains.
    pub find_cache: &'a dyn FindCache,
    /// What to do when no target match is found.
    pub no_match_fallback: NoMatchFallback,
    /// What to do with orphaned target records.
    pub orphan_strategy: OrphanStrategy,
}

/// Compare a mapping's transform results against target records.
///
/// Consumes the mapping result, moving field data into comparisons
/// instead of cloning.
pub fn compare_mapping_results(input: ComparisonInput<'_>) -> MappingComparison {
    compare_mapping(CompareInput {
        source_records: input.source_records,
        record_results: input.mapping_result.record_results,
        target_records: input.target_records,
        strategy: input.strategy,
        source_primary_key: input.source_primary_key,
        target_primary_key: input.target_primary_key,
        match_conditions: input.match_conditions,
        source_entity: input.source_entity,
        target_entity: input.target_entity,
        find_cache: input.find_cache,
        no_match_fallback: input.no_match_fallback,
        orphan_strategy: input.orphan_strategy,
    })
}

// =============================================================================
// Junction Entity Synthetic ID
// =============================================================================

/// Namespace UUID for junction entity synthetic IDs (UUID v5).
/// Generated once, arbitrary but fixed.
const JUNCTION_NAMESPACE: uuid::Uuid = uuid::Uuid::from_bytes([
    0x6a, 0x75, 0x6e, 0x63, 0x74, 0x69, 0x6f, 0x6e, // "junction"
    0x2d, 0x65, 0x6e, 0x74, 0x69, 0x74, 0x79, 0x2d, // "-entity-"
]);

/// Compute a deterministic synthetic ID for a junction entity record.
///
/// Takes the two FK GUID values from the record, sorts them alphabetically,
/// and produces a UUID v5. This makes `SameId` matching work for junction
/// entities — the same FK pair in different environments produces the same
/// synthetic ID, regardless of the meaningless junction primary key.
///
/// Returns `None` if either FK field is missing or not a GUID.
pub fn junction_synthetic_id(
    record: &Record,
    fk_attr1: &str,
    fk_attr2: &str,
) -> Option<uuid::Uuid> {
    let fk1 = match record.get(fk_attr1) {
        Some(dataverse_lib::model::Value::Guid(g)) => g.to_string(),
        _ => return None,
    };
    let fk2 = match record.get(fk_attr2) {
        Some(dataverse_lib::model::Value::Guid(g)) => g.to_string(),
        _ => return None,
    };

    // Sort alphabetically so (A, B) and (B, A) produce the same ID
    let combined = if fk1 <= fk2 {
        format!("{}_{}", fk1, fk2)
    } else {
        format!("{}_{}", fk2, fk1)
    };

    Some(uuid::Uuid::new_v5(&JUNCTION_NAMESPACE, combined.as_bytes()))
}

/// Apply synthetic IDs to all records in a junction entity result set.
///
/// Replaces each record's ID with a deterministic UUID derived from its
/// two FK values. Records where either FK is missing are left unchanged
/// (they'll fail to match, which is correct).
pub fn apply_junction_synthetic_ids(records: &mut [Record], fk_attr1: &str, fk_attr2: &str) {
    let mut applied = 0;
    for record in records.iter_mut() {
        if let Some(synthetic) = junction_synthetic_id(record, fk_attr1, fk_attr2) {
            record.set_id(synthetic);
            applied += 1;
        }
    }
    log::debug!(
        "junction: applied synthetic IDs to {}/{} records (fk1={}, fk2={})",
        applied,
        records.len(),
        fk_attr1,
        fk_attr2,
    );
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use dataverse_lib::model::Value;
    use uuid::Uuid;

    use crate::apps::migration::engine::ChainChildren;
    use crate::apps::migration::engine::StubFindCache;
    use crate::apps::migration::types::TransformData;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn make_record(entity: &str, id: Uuid, fields: Vec<(&str, Value)>) -> Record {
        let mut record = Record::with_id(Entity::logical(entity), id);
        for (field, value) in fields {
            record = record.set(field, value);
        }
        record
    }

    fn copy_chain(path: &str) -> Vec<ChainItem> {
        vec![ChainItem {
            data: TransformData::Copy {
                path: path.to_string(),
            },
            children: ChainChildren::None,
        }]
    }

    fn constant_chain(value: Value) -> Vec<ChainItem> {
        vec![ChainItem {
            data: TransformData::Constant { value },
            children: ChainChildren::None,
        }]
    }

    // ---- analyze_phase tests ----

    #[test]
    fn analyze_phase_single_mapping() {
        let field_mappings = vec![
            ("name".to_string(), copy_chain("name")),
            ("city".to_string(), copy_chain("address1_city")),
        ];
        let variables: Vec<(String, Vec<ChainItem>)> = vec![];

        let inputs = vec![MappingInput {
            source_entity: "account",
            target_entity: "account",
            source_primary_key: "accountid",
            target_primary_key: "accountid",
            field_mappings: &field_mappings,
            variables: &variables,
            match_config_chain: None,
            source_filter: None,
            target_filter: None,
            test_guids: None,
            mapping_name: "account → account",
        }];

        let plan = analyze_phase(&inputs);
        assert_eq!(plan.mapping_plans.len(), 1);
        assert!(plan.mapping_plans[0].source.select.contains("name"));
        assert!(plan.mapping_plans[0]
            .source
            .select
            .contains("address1_city"));
        assert!(plan.mapping_plans[0].source.select.contains("accountid"));
        assert!(plan.merged_find_caches.is_empty());
    }

    // ---- split_fetch_results tests ----

    #[test]
    fn split_fetch_results_correct_categories() {
        let results = vec![
            vec![make_record("account", id(1), vec![])],  // source 0
            vec![make_record("contact", id(2), vec![])],  // source 1
            vec![make_record("account", id(3), vec![])],  // target for mapping 0
            vec![make_record("capacity", id(4), vec![])], // find cache
        ];

        let index = FetchTaskIndex {
            source_count: 2,
            target_count: 1,
            target_mapping_indices: vec![0],
            find_cache_count: 1,
        };

        let split = split_fetch_results(results, &index);
        assert_eq!(split.source_records.len(), 2);
        assert_eq!(split.source_records[0][0].id(), Some(id(1)));
        assert_eq!(split.source_records[1][0].id(), Some(id(2)));
        assert_eq!(split.target_records.len(), 1);
        assert_eq!(split.target_records[0].0, 0); // mapping index
        assert_eq!(split.target_records[0].1[0].id(), Some(id(3)));
        assert_eq!(split.find_cache_records.len(), 1);
        assert_eq!(split.find_cache_records[0][0].id(), Some(id(4)));
    }

    #[test]
    fn split_fetch_results_no_target() {
        let results = vec![vec![make_record("account", id(1), vec![])]];

        let index = FetchTaskIndex {
            source_count: 1,
            target_count: 0,
            target_mapping_indices: vec![],
            find_cache_count: 0,
        };

        let split = split_fetch_results(results, &index);
        assert_eq!(split.source_records.len(), 1);
        assert!(split.target_records.is_empty());
        assert!(split.find_cache_records.is_empty());
    }

    // ---- build_find_cache tests ----

    #[test]
    fn build_find_cache_populates_entities() {
        let specs = vec![
            FindCacheSpec {
                entity: "contact".to_string(),
                select: HashSet::new(),
                expands: vec![],
            },
            FindCacheSpec {
                entity: "capacity".to_string(),
                select: HashSet::new(),
                expands: vec![],
            },
        ];

        let records = vec![
            vec![make_record(
                "contact",
                id(1),
                vec![("name", Value::from("Alice"))],
            )],
            vec![make_record(
                "capacity",
                id(2),
                vec![("name", Value::from("Cap1"))],
            )],
        ];

        let cache = build_find_cache(records, &specs);
        assert!(cache.get("contact", id(1)).is_some());
        assert!(cache.get("capacity", id(2)).is_some());
        assert!(cache.get("other", id(1)).is_none());
    }

    // ---- execute_mapping tests ----

    #[test]
    fn execute_mapping_transforms_all_records() {
        let field_mappings = vec![("target_name".to_string(), copy_chain("name"))];
        let variables: Vec<(String, Vec<ChainItem>)> = vec![];

        let source_records = vec![
            make_record("account", id(1), vec![("name", Value::from("Acme"))]),
            make_record("account", id(2), vec![("name", Value::from("Contoso"))]),
            make_record("account", id(3), vec![("name", Value::from("Fabrikam"))]),
        ];

        let cache = StubFindCache;
        let result = execute_mapping(
            &source_records,
            &variables,
            &field_mappings,
            "account",
            "account",
            &cache,
        );

        assert_eq!(result.record_results.len(), 3);
        assert!(result.record_results[0].is_ok());
        assert_eq!(
            result.record_results[0].fields.get("target_name"),
            Some(&Value::from("Acme"))
        );
        assert_eq!(
            result.record_results[1].fields.get("target_name"),
            Some(&Value::from("Contoso"))
        );
        assert_eq!(
            result.record_results[2].fields.get("target_name"),
            Some(&Value::from("Fabrikam"))
        );
    }

    #[test]
    fn execute_mapping_with_constants() {
        let field_mappings = vec![(
            "source_field".to_string(),
            constant_chain(Value::from("fixed")),
        )];
        let variables: Vec<(String, Vec<ChainItem>)> = vec![];

        let source_records = vec![
            make_record("account", id(1), vec![]),
            make_record("account", id(2), vec![]),
        ];

        let cache = StubFindCache;
        let result = execute_mapping(
            &source_records,
            &variables,
            &field_mappings,
            "account",
            "account",
            &cache,
        );

        assert_eq!(result.record_results.len(), 2);
        for rr in &result.record_results {
            assert!(rr.is_ok());
            assert_eq!(rr.fields.get("source_field"), Some(&Value::from("fixed")));
        }
    }

    #[test]
    fn execute_mapping_empty_source() {
        let field_mappings = vec![("x".to_string(), copy_chain("x"))];
        let variables: Vec<(String, Vec<ChainItem>)> = vec![];

        let cache = StubFindCache;
        let result = execute_mapping(&[], &variables, &field_mappings, "a", "a", &cache);

        assert!(result.record_results.is_empty());
    }

    #[test]
    fn execute_mapping_collects_per_field_errors() {
        let field_mappings = vec![
            ("good".to_string(), constant_chain(Value::from("ok"))),
            ("bad".to_string(), copy_chain("nonexistent_field")),
        ];
        let variables: Vec<(String, Vec<ChainItem>)> = vec![];

        let source_records = vec![make_record("account", id(1), vec![])];

        let cache = StubFindCache;
        let result = execute_mapping(
            &source_records,
            &variables,
            &field_mappings,
            "account",
            "account",
            &cache,
        );

        assert_eq!(result.record_results.len(), 1);
        let rr = &result.record_results[0];
        assert_eq!(rr.field_count(), 1);
        assert_eq!(rr.error_count(), 1);
        assert_eq!(rr.fields.get("good"), Some(&Value::from("ok")));
        assert_eq!(rr.errors[0].0, "bad");
    }

    // ---- compare_mapping_results tests ----

    #[test]
    fn compare_mapping_results_end_to_end() {
        // Execute transforms, then compare against targets
        let field_mappings = vec![("name".to_string(), copy_chain("name"))];
        let variables: Vec<(String, Vec<ChainItem>)> = vec![];

        let source_records = vec![
            make_record("account", id(1), vec![("name", Value::from("Same"))]),
            make_record("account", id(2), vec![("name", Value::from("Changed"))]),
            make_record("account", id(3), vec![("name", Value::from("New"))]),
        ];

        let target_records = vec![
            make_record("account", id(1), vec![("name", Value::from("Same"))]),
            make_record("account", id(2), vec![("name", Value::from("Old"))]),
            make_record("account", id(99), vec![("name", Value::from("Orphan"))]),
        ];

        let cache = StubFindCache;
        let mapping_result = execute_mapping(
            &source_records,
            &variables,
            &field_mappings,
            "account",
            "account",
            &cache,
        );

        let comparison_input = ComparisonInput {
            source_records: &source_records,
            mapping_result,
            target_records: &target_records,
            strategy: MatchStrategy::SameId,
            source_primary_key: "accountid",
            target_primary_key: "accountid",
            match_conditions: &[],
            source_entity: "account",
            target_entity: "account",
            find_cache: &cache,
            no_match_fallback: NoMatchFallback::Create,
            orphan_strategy: OrphanStrategy::Delete,
        };

        let comparison = compare_mapping_results(comparison_input);

        use crate::apps::migration::comparison::OperationType;

        assert_eq!(comparison.records.len(), 3);
        assert_eq!(comparison.records[0].operation, OperationType::Skip);
        assert_eq!(comparison.records[1].operation, OperationType::Update);
        assert_eq!(comparison.records[2].operation, OperationType::Create);
        assert_eq!(comparison.orphans.len(), 1);
        assert_eq!(comparison.orphans[0].operation, OperationType::Delete);
    }
}
