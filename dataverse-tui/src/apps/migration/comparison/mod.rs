//! Comparison engine — match source records to targets and determine operations.
//!
//! Orchestrates matching, diffing, and operation determination for an entire
//! entity mapping's records.

pub mod diff;
pub mod matching;

use std::collections::HashMap;
use std::collections::HashSet;

use dataverse_lib::model::Record;
use dataverse_lib::model::Value;
use uuid::Uuid;

use self::diff::FieldDiff;
use self::diff::diff_fields;
use self::matching::MatchInput;
use self::matching::MatchResult;
use self::matching::TargetIndexError;
use self::matching::build_target_index;
use self::matching::match_target;

use super::engine::ChainItem;
use super::engine::FindCache;
use super::engine::TransformError;
use super::engine::record::RecordResult;
use super::types::MatchStrategy;
use super::types::NoMatchFallback;
use super::types::OrphanStrategy;

// =============================================================================
// Types
// =============================================================================

/// The determined operation for a record.
#[derive(Debug, Clone, PartialEq)]
pub enum OperationType {
    /// Record should be created in the target.
    Create,
    /// Record should be updated in the target.
    Update,
    /// Record is identical — no action needed.
    Skip,
    /// Orphaned target record should be deleted.
    Delete,
    /// Orphaned target record should be deactivated.
    Deactivate,
    /// Association should be created (junction entity target).
    Associate,
    /// Association should be removed (junction entity orphan).
    Disassociate,
    /// Source record ignored — no target match (per config).
    IgnoreSource,
    /// Orphaned target record ignored (per config).
    IgnoreTarget,
    /// An error occurred during processing.
    Error(String),
}

/// Comparison result for a single source record.
#[derive(Debug, Clone)]
pub struct RecordComparison {
    /// The determined operation.
    pub operation: OperationType,
    /// Source record ID.
    pub source_id: Option<Uuid>,
    /// Matched target record ID, if any.
    pub target_id: Option<Uuid>,
    /// Transformed field values (moved from RecordResult, not cloned).
    pub transformed: HashMap<String, Value>,
    /// Field-level diffs (only for Update operations).
    pub diffs: Vec<FieldDiff>,
    /// Transform errors from the engine.
    pub errors: Vec<(String, TransformError)>,
    /// The target record's current statecode (if matched).
    pub target_statecode: Option<Value>,
    /// The target record's current statuscode (if matched).
    pub target_statuscode: Option<Value>,
}

/// An orphaned target record (not matched by any source record).
#[derive(Debug, Clone)]
pub struct OrphanRecord {
    /// The determined operation (Delete, Deactivate, IgnoreTarget, or Error).
    pub operation: OperationType,
    /// The orphaned target record ID.
    pub record_id: Option<Uuid>,
    /// Target record field values (for display in detail view).
    pub fields: HashMap<String, Value>,
}

/// Comparison results for an entire entity mapping.
#[derive(Debug, Clone, Default)]
pub struct MappingComparison {
    /// Source entity logical name.
    pub source_entity: String,
    /// Target entity logical name.
    pub target_entity: String,
    /// Per-source-record comparison results.
    pub records: Vec<RecordComparison>,
    /// Orphaned target records.
    pub orphans: Vec<OrphanRecord>,
}

impl MappingComparison {
    /// Count records by operation type.
    pub fn count_operations(&self) -> OperationTypeCounts {
        let mut counts = OperationTypeCounts::default();
        for r in &self.records {
            match &r.operation {
                OperationType::Create => counts.create += 1,
                OperationType::Update => counts.update += 1,
                OperationType::Skip => counts.skip += 1,
                OperationType::Associate => counts.associate += 1,
                OperationType::IgnoreSource => counts.ignore_source += 1,
                OperationType::Error(_) => counts.error += 1,
                _ => {}
            }
        }
        for o in &self.orphans {
            match &o.operation {
                OperationType::Delete => counts.delete += 1,
                OperationType::Deactivate => counts.deactivate += 1,
                OperationType::Disassociate => counts.disassociate += 1,
                OperationType::IgnoreTarget => counts.ignore_target += 1,
                OperationType::Error(_) => counts.error += 1,
                _ => {}
            }
        }
        counts
    }
}

/// Aggregate operation counts.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct OperationTypeCounts {
    pub create: usize,
    pub update: usize,
    pub skip: usize,
    pub delete: usize,
    pub deactivate: usize,
    pub associate: usize,
    pub disassociate: usize,
    pub ignore_source: usize,
    pub ignore_target: usize,
    pub error: usize,
}

impl std::ops::Add for OperationTypeCounts {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self {
            create: self.create + rhs.create,
            update: self.update + rhs.update,
            skip: self.skip + rhs.skip,
            delete: self.delete + rhs.delete,
            deactivate: self.deactivate + rhs.deactivate,
            associate: self.associate + rhs.associate,
            disassociate: self.disassociate + rhs.disassociate,
            ignore_source: self.ignore_source + rhs.ignore_source,
            ignore_target: self.ignore_target + rhs.ignore_target,
            error: self.error + rhs.error,
        }
    }
}

// =============================================================================
// Input
// =============================================================================

/// Input for comparing an entire entity mapping.
pub struct CompareInput<'a> {
    /// Source records (borrowed for matching, IDs extracted).
    pub source_records: &'a [Record],
    /// Transform results per source record (owned — fields are moved into comparisons).
    pub record_results: Vec<RecordResult>,
    /// Target records fetched from the target environment.
    pub target_records: &'a [Record],
    /// Match strategy.
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

// =============================================================================
// Orchestration
// =============================================================================

/// Compare an entire entity mapping's records.
///
/// For each source record + transform result:
/// 1. Match against target records
/// 2. Diff transformed fields against matched target
/// 3. Determine operation (Create, Update, Skip, Error)
///
/// Then detect orphaned target records and apply orphan strategy.
pub fn compare_mapping(input: CompareInput<'_>) -> Result<MappingComparison, TargetIndexError> {
    let mut records = Vec::with_capacity(input.source_records.len());
    let mut matched_target_ids: HashSet<Uuid> = HashSet::new();

    // Build target index once for O(1) SameId lookups
    let target_index = build_target_index(input.target_records, input.target_primary_key)?;

    for (source_record, record_result) in input
        .source_records
        .iter()
        .zip(input.record_results.into_iter())
    {
        let match_input = MatchInput {
            source_record,
            strategy: input.strategy,
            source_primary_key: input.source_primary_key,
            target_primary_key: input.target_primary_key,
            match_conditions: input.match_conditions,
            source_entity: input.source_entity,
            target_entity: input.target_entity,
            find_cache: input.find_cache,
        };

        let has_errors = !record_result.errors.is_empty();
        let match_result = match_target(&match_input, input.target_records, &target_index);

        let (operation, target_id, diffs, target_statecode, target_statuscode) = match match_result
        {
            MatchResult::Found(target_idx) => {
                let target = &input.target_records[target_idx];
                let tid = target.id();
                if let Some(tid) = tid {
                    matched_target_ids.insert(tid);
                }

                let tsc = target.get("statecode").cloned();
                let tssc = target.get("statuscode").cloned();

                if has_errors {
                    (
                        OperationType::Error("Transform errors".into()),
                        tid,
                        vec![],
                        tsc,
                        tssc,
                    )
                } else {
                    let diffs = diff_fields(&record_result.fields, target);
                    let op = if diffs.is_empty() {
                        OperationType::Skip
                    } else {
                        OperationType::Update
                    };
                    (op, tid, diffs, tsc, tssc)
                }
            }
            MatchResult::NotFound => {
                if has_errors {
                    (
                        OperationType::Error("Transform errors".into()),
                        None,
                        vec![],
                        None,
                        None,
                    )
                } else {
                    let op = match input.no_match_fallback {
                        NoMatchFallback::Error => {
                            OperationType::Error("No target match found".into())
                        }
                        NoMatchFallback::Create => OperationType::Create,
                        NoMatchFallback::Ignore => OperationType::IgnoreSource,
                    };
                    (op, None, vec![], None, None)
                }
            }
            MatchResult::Multiple(n) => (
                OperationType::Error(format!("Multiple target matches ({})", n)),
                None,
                vec![],
                None,
                None,
            ),
            MatchResult::Error(msg) => (OperationType::Error(msg), None, vec![], None, None),
        };

        // Move fields and errors out of record_result — no cloning
        records.push(RecordComparison {
            operation,
            source_id: source_record.id(),
            target_id,
            transformed: record_result.fields,
            diffs,
            errors: record_result.errors,
            target_statecode,
            target_statuscode,
        });
    }

    // Orphan detection
    let orphans = detect_orphans(
        input.target_records,
        &matched_target_ids,
        input.orphan_strategy,
    );

    Ok(MappingComparison {
        source_entity: input.source_entity.to_string(),
        target_entity: input.target_entity.to_string(),
        records,
        orphans,
    })
}

/// Detect orphaned target records and apply the orphan strategy.
fn detect_orphans(
    target_records: &[Record],
    matched_ids: &HashSet<Uuid>,
    strategy: OrphanStrategy,
) -> Vec<OrphanRecord> {
    let mut orphans = Vec::new();

    for target in target_records {
        let target_id = match target.id() {
            Some(id) => id,
            None => continue, // Can't track records without IDs
        };

        if !matched_ids.contains(&target_id) {
            let operation = match strategy {
                OrphanStrategy::Delete => OperationType::Delete,
                OrphanStrategy::Deactivate => OperationType::Deactivate,
                OrphanStrategy::Ignore => OperationType::IgnoreTarget,
                OrphanStrategy::Error => OperationType::Error("Orphaned target record".into()),
            };

            let fields = target
                .fields()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();

            orphans.push(OrphanRecord {
                operation,
                record_id: Some(target_id),
                fields,
            });
        }
    }

    orphans
}

// =============================================================================
// Junction Entity Post-Processing
// =============================================================================

/// Remap operation types for a junction entity target.
///
/// Junction entities represent N:N associations — records are either associated
/// or not. This remaps standard CRUD operations to junction-appropriate ones:
/// - `Create` → `Associate` (create the association)
/// - `Update` → `Skip` (associations are binary, no partial updates)
/// - `Delete` → `Disassociate` (remove the association)
/// - `Deactivate` → `Disassociate` (junctions can't be deactivated)
/// - All other operations pass through unchanged.
pub fn remap_junction_operations(comparison: &mut MappingComparison) {
    for record in &mut comparison.records {
        record.operation = match std::mem::replace(&mut record.operation, OperationType::Skip) {
            OperationType::Create => OperationType::Associate,
            OperationType::Update => {
                // Associations are binary — if the FK pair matches, it's a skip.
                // Clear diffs since there's nothing to update.
                record.diffs.clear();
                OperationType::Skip
            }
            other => other,
        };
    }
    for orphan in &mut comparison.orphans {
        orphan.operation = match std::mem::replace(&mut orphan.operation, OperationType::Skip) {
            OperationType::Delete | OperationType::Deactivate => OperationType::Disassociate,
            other => other,
        };
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apps::migration::engine::StubFindCache;
    use crate::apps::migration::types::TransformData;
    use dataverse_lib::model::Entity;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn make_record(entity: &str, uuid: Uuid, fields: Vec<(&str, Value)>) -> Record {
        let mut record = Record::with_id(Entity::logical(entity), uuid);
        for (field, value) in fields {
            record = record.set(field, value);
        }
        record
    }

    fn make_result(fields: Vec<(&str, Value)>) -> RecordResult {
        RecordResult {
            fields: fields
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
            errors: vec![],
        }
    }

    fn make_error_result(
        fields: Vec<(&str, Value)>,
        errors: Vec<(&str, TransformError)>,
    ) -> RecordResult {
        RecordResult {
            fields: fields
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
            errors: errors
                .into_iter()
                .map(|(k, e)| (k.to_string(), e))
                .collect(),
        }
    }

    static STUB: StubFindCache = StubFindCache;

    fn default_compare_input<'a>(
        source_records: &'a [Record],
        record_results: Vec<RecordResult>,
        target_records: &'a [Record],
    ) -> CompareInput<'a> {
        CompareInput {
            source_records,
            record_results,
            target_records,
            strategy: MatchStrategy::SameId,
            source_primary_key: "accountid",
            target_primary_key: "accountid",
            match_conditions: &[],
            source_entity: "account",
            target_entity: "account",
            find_cache: &STUB,
            no_match_fallback: NoMatchFallback::Create,
            orphan_strategy: OrphanStrategy::Ignore,
        }
    }

    // ---- Full flow tests ----

    #[test]
    fn matched_record_no_changes_is_skip() {
        let sources = vec![make_record(
            "account",
            id(1),
            vec![("name", Value::from("Acme"))],
        )];
        let results = vec![make_result(vec![("name", Value::from("Acme"))])];
        let targets = vec![make_record(
            "account",
            id(1),
            vec![("name", Value::from("Acme"))],
        )];

        let input = default_compare_input(&sources, results, &targets);
        let comparison = compare_mapping(input).unwrap();

        assert_eq!(comparison.records.len(), 1);
        assert_eq!(comparison.records[0].operation, OperationType::Skip);
        assert!(comparison.records[0].diffs.is_empty());
        assert!(comparison.records[0].target_id.is_some());
    }

    #[test]
    fn matched_record_with_changes_is_update() {
        let sources = vec![make_record(
            "account",
            id(1),
            vec![("name", Value::from("Acme New"))],
        )];
        let results = vec![make_result(vec![("name", Value::from("Acme New"))])];
        let targets = vec![make_record(
            "account",
            id(1),
            vec![("name", Value::from("Acme Old"))],
        )];

        let input = default_compare_input(&sources, results, &targets);
        let comparison = compare_mapping(input).unwrap();

        assert_eq!(comparison.records.len(), 1);
        assert_eq!(comparison.records[0].operation, OperationType::Update);
        assert_eq!(comparison.records[0].diffs.len(), 1);
        assert_eq!(comparison.records[0].diffs[0].field, "name");
    }

    #[test]
    fn no_match_with_create_fallback() {
        let sources = vec![make_record(
            "account",
            id(1),
            vec![("name", Value::from("New"))],
        )];
        let results = vec![make_result(vec![("name", Value::from("New"))])];
        let targets = vec![];

        let mut input = default_compare_input(&sources, results, &targets);
        input.no_match_fallback = NoMatchFallback::Create;
        let comparison = compare_mapping(input).unwrap();

        assert_eq!(comparison.records[0].operation, OperationType::Create);
        assert!(comparison.records[0].target_id.is_none());
    }

    #[test]
    fn no_match_with_error_fallback() {
        let sources = vec![make_record("account", id(1), vec![])];
        let results = vec![make_result(vec![])];
        let targets = vec![];

        let mut input = default_compare_input(&sources, results, &targets);
        input.no_match_fallback = NoMatchFallback::Error;
        let comparison = compare_mapping(input).unwrap();

        assert!(matches!(
            comparison.records[0].operation,
            OperationType::Error(_)
        ));
    }

    #[test]
    fn no_match_with_ignore_fallback() {
        let sources = vec![make_record("account", id(1), vec![])];
        let results = vec![make_result(vec![])];
        let targets = vec![];

        let mut input = default_compare_input(&sources, results, &targets);
        input.no_match_fallback = NoMatchFallback::Ignore;
        let comparison = compare_mapping(input).unwrap();

        assert_eq!(comparison.records[0].operation, OperationType::IgnoreSource);
    }

    #[test]
    fn transform_errors_produce_error_operation() {
        let sources = vec![make_record(
            "account",
            id(1),
            vec![("name", Value::from("Acme"))],
        )];
        let results = vec![make_error_result(
            vec![("name", Value::from("Acme"))],
            vec![(
                "bad_field",
                TransformError::NullInPath {
                    segment: "bad".into(),
                },
            )],
        )];
        let targets = vec![make_record(
            "account",
            id(1),
            vec![("name", Value::from("Acme"))],
        )];

        let input = default_compare_input(&sources, results, &targets);
        let comparison = compare_mapping(input).unwrap();

        assert!(matches!(
            comparison.records[0].operation,
            OperationType::Error(_)
        ));
        // Still matched the target
        assert!(comparison.records[0].target_id.is_some());
    }

    // ---- Orphan tests ----

    #[test]
    fn orphan_strategy_delete() {
        let sources = vec![make_record("account", id(1), vec![])];
        let results = vec![make_result(vec![])];
        let targets = vec![
            make_record("account", id(1), vec![]),
            make_record("account", id(2), vec![]),
        ];

        let mut input = default_compare_input(&sources, results, &targets);
        input.orphan_strategy = OrphanStrategy::Delete;
        let comparison = compare_mapping(input).unwrap();

        assert_eq!(comparison.orphans.len(), 1);
        assert_eq!(comparison.orphans[0].operation, OperationType::Delete);
        assert_eq!(comparison.orphans[0].record_id, Some(id(2)));
    }

    #[test]
    fn orphan_strategy_deactivate() {
        let sources = vec![make_record("account", id(1), vec![])];
        let results = vec![make_result(vec![])];
        let targets = vec![
            make_record("account", id(1), vec![]),
            make_record("account", id(2), vec![]),
        ];

        let mut input = default_compare_input(&sources, results, &targets);
        input.orphan_strategy = OrphanStrategy::Deactivate;
        let comparison = compare_mapping(input).unwrap();

        assert_eq!(comparison.orphans.len(), 1);
        assert_eq!(comparison.orphans[0].operation, OperationType::Deactivate);
    }

    #[test]
    fn orphan_strategy_ignore() {
        let sources = vec![make_record("account", id(1), vec![])];
        let results = vec![make_result(vec![])];
        let targets = vec![
            make_record("account", id(1), vec![]),
            make_record("account", id(2), vec![]),
        ];

        let mut input = default_compare_input(&sources, results, &targets);
        input.orphan_strategy = OrphanStrategy::Ignore;
        let comparison = compare_mapping(input).unwrap();

        assert_eq!(comparison.orphans.len(), 1);
        assert_eq!(comparison.orphans[0].operation, OperationType::IgnoreTarget);
    }

    #[test]
    fn orphan_strategy_error() {
        let sources: Vec<Record> = vec![];
        let results: Vec<RecordResult> = vec![];
        let targets = vec![make_record("account", id(1), vec![])];

        let mut input = default_compare_input(&sources, results, &targets);
        input.orphan_strategy = OrphanStrategy::Error;
        let comparison = compare_mapping(input).unwrap();

        assert_eq!(comparison.orphans.len(), 1);
        assert!(matches!(
            comparison.orphans[0].operation,
            OperationType::Error(_)
        ));
    }

    // ---- Mixed operations ----

    #[test]
    fn mixed_operations_in_one_mapping() {
        let sources = vec![
            make_record("account", id(1), vec![("name", Value::from("Same"))]),
            make_record("account", id(2), vec![("name", Value::from("Changed"))]),
            make_record("account", id(3), vec![("name", Value::from("New"))]),
        ];
        let results = vec![
            make_result(vec![("name", Value::from("Same"))]),
            make_result(vec![("name", Value::from("Changed"))]),
            make_result(vec![("name", Value::from("New"))]),
        ];
        let targets = vec![
            make_record("account", id(1), vec![("name", Value::from("Same"))]),
            make_record("account", id(2), vec![("name", Value::from("Old"))]),
            make_record("account", id(99), vec![("name", Value::from("Orphan"))]),
        ];

        let mut input = default_compare_input(&sources, results, &targets);
        input.no_match_fallback = NoMatchFallback::Create;
        input.orphan_strategy = OrphanStrategy::Delete;
        let comparison = compare_mapping(input).unwrap();

        assert_eq!(comparison.records[0].operation, OperationType::Skip);
        assert_eq!(comparison.records[1].operation, OperationType::Update);
        assert_eq!(comparison.records[2].operation, OperationType::Create);
        assert_eq!(comparison.orphans.len(), 1);
        assert_eq!(comparison.orphans[0].operation, OperationType::Delete);

        let counts = comparison.count_operations();
        assert_eq!(
            counts,
            OperationTypeCounts {
                create: 1,
                update: 1,
                skip: 1,
                delete: 1,
                ..Default::default()
            }
        );
    }

    // ---- Find strategy test ----

    #[test]
    fn find_strategy_with_match_conditions() {
        let sources = vec![make_record(
            "account",
            id(1),
            vec![("name", Value::from("Acme"))],
        )];
        let results = vec![make_result(vec![("name", Value::from("Acme"))])];
        let targets = vec![make_record(
            "account",
            id(10),
            vec![("name", Value::from("Acme"))],
        )];

        let conditions = vec![(
            "name".to_string(),
            vec![ChainItem::new(TransformData::Copy {
                path: "name".to_string(),
            })],
        )];

        let mut input = default_compare_input(&sources, results, &targets);
        input.strategy = MatchStrategy::Find;
        input.match_conditions = &conditions;
        let comparison = compare_mapping(input).unwrap();

        assert_eq!(comparison.records[0].operation, OperationType::Skip);
        assert_eq!(comparison.records[0].target_id, Some(id(10)));
    }
}
