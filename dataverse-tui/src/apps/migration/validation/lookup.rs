//! Lookup validation — detect and null out references to non-existent target records.
//!
//! After transforms produce `EntityReference` values for lookup fields, we validate
//! that the referenced records actually exist in the target environment. Missing
//! references are nulled out to avoid runtime errors during Create/Update execution.
//!
//! ## Flow
//!
//! 1. **Pre-fetch** (in `ODataFetchModal`): For each entity type that is a lookup
//!    target, fetch all primary IDs with `$select={primary_key}`.
//! 2. **Build cache**: Convert fetched records into `HashMap<String, HashSet<Uuid>>`.
//! 3. **Validate**: After transformation, scan `RecordResult.fields` for
//!    `EntityReference` values and null out any that reference non-existent IDs.

use std::collections::HashMap;
use std::collections::HashSet;

use dataverse_lib::model::Record;
use dataverse_lib::model::Value;
use uuid::Uuid;

use crate::apps::migration::engine::record::RecordResult;

// =============================================================================
// Types
// =============================================================================

/// Describes a lookup target entity that needs its IDs fetched for validation.
#[derive(Debug, Clone)]
pub struct LookupValidationSpec {
    /// Entity logical name (e.g., "systemuser").
    pub entity: String,
    /// Primary key field name (e.g., "systemuserid").
    pub primary_key: String,
}

// =============================================================================
// Cache Building
// =============================================================================

/// Build a validation cache from fetched records.
///
/// Maps each entity name to the set of existing primary IDs.
/// The `records` and `specs` must be in the same order (matching by index).
pub fn build_validation_cache(
    records: &[Vec<Record>],
    specs: &[LookupValidationSpec],
) -> HashMap<String, HashSet<Uuid>> {
    let mut cache: HashMap<String, HashSet<Uuid>> = HashMap::new();

    for (spec, entity_records) in specs.iter().zip(records.iter()) {
        let ids: HashSet<Uuid> = entity_records.iter().filter_map(|r| r.id()).collect();

        log::debug!(
            "[lookup-validation] Cached {} IDs for entity '{}'",
            ids.len(),
            spec.entity,
        );

        cache.insert(spec.entity.clone(), ids);
    }

    cache
}

// =============================================================================
// Validation
// =============================================================================

/// Validate lookup fields in transformed records against the validation cache.
///
/// Scans all fields in each `RecordResult` for `Value::EntityReference` values.
/// If the referenced ID does not exist in the cache for that entity type,
/// the value is replaced with `Value::Null` and a warning is logged.
///
/// Returns the number of lookups that were nulled.
pub fn validate_lookups(
    record_results: &mut [RecordResult],
    validation_cache: &HashMap<String, HashSet<Uuid>>,
) -> usize {
    if validation_cache.is_empty() {
        return 0;
    }

    let mut nulled_count = 0;

    for record in record_results.iter_mut() {
        for (field_name, value) in record.fields.iter_mut() {
            let (entity_name, id) = match value {
                Value::EntityReference(er) => (er.entity.name().to_string(), er.id),
                _ => continue,
            };

            // If we don't have a cache for this entity, we can't validate — skip
            let Some(existing_ids) = validation_cache.get(&entity_name) else {
                continue;
            };

            if !existing_ids.contains(&id) {
                log::warn!(
                    "Nulling lookup: field='{}', target_entity='{}', missing_id='{}'",
                    field_name,
                    entity_name,
                    id,
                );
                *value = Value::Null;
                nulled_count += 1;
            }
        }
    }

    if nulled_count > 0 {
        log::info!(
            "[lookup-validation] Nulled {} lookup(s) referencing non-existent records",
            nulled_count,
        );
    }

    nulled_count
}
