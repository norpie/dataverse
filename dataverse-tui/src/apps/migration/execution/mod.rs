//! Execution module — converts comparison results into Dataverse batch operations.
//!
//! Generates operations per sub-phase:
//! 1. **Create** — scalar fields only (lookups deferred to Update)
//! 2. **Activate** — reactivate inactive target records before Update
//! 3. **Update** — lookup fields on created records + diffs on existing
//! 4. **Associate** — N:N junction associations
//! 5. **Disassociate** — orphan junction associations
//! 6. **Deactivate** — set inactive state on records and orphans
//! 7. **Delete** — orphan record deletion

use std::collections::HashMap;

use dataverse_lib::api::Batch;
use dataverse_lib::api::Op;
use dataverse_lib::model::metadata::ExecutionMetadata;
use dataverse_lib::model::metadata::ManyToManyRelationship;
use dataverse_lib::model::types::EntityBinding;
use dataverse_lib::model::types::OptionSetValue;
use dataverse_lib::model::Entity;
use dataverse_lib::model::Record;
use dataverse_lib::model::Value;
use uuid::Uuid;

use super::comparison::MappingComparison;
use super::comparison::OperationType;
use super::comparison::OrphanRecord;
use super::comparison::RecordComparison;

/// Batch size for grouping operations.
const BATCH_SIZE: usize = 50;

// =============================================================================
// Sub-Phase Enum
// =============================================================================

/// The sequential sub-phases of execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubPhase {
    Create,
    Activate,
    Update,
    Associate,
    Disassociate,
    Deactivate,
    Delete,
}

impl SubPhase {
    /// All sub-phases in execution order.
    pub const ALL: &'static [SubPhase] = &[
        SubPhase::Create,
        SubPhase::Activate,
        SubPhase::Update,
        SubPhase::Associate,
        SubPhase::Disassociate,
        SubPhase::Deactivate,
        SubPhase::Delete,
    ];

    /// Display label for this sub-phase.
    pub fn label(&self) -> &'static str {
        match self {
            SubPhase::Create => "Create",
            SubPhase::Activate => "Activate",
            SubPhase::Update => "Update",
            SubPhase::Associate => "Associate",
            SubPhase::Disassociate => "Disassociate",
            SubPhase::Deactivate => "Deactivate",
            SubPhase::Delete => "Delete",
        }
    }
}

// =============================================================================
// Output Types
// =============================================================================

/// Batches for a single entity within a sub-phase.
#[derive(Debug)]
pub struct EntityBatches {
    /// Target entity logical name.
    pub entity: String,
    /// Batches ready for queue submission.
    pub batches: Vec<Batch>,
    /// Total number of individual operations across all batches.
    pub operation_count: usize,
}

/// A record that was created and needs lookup fields set in the Update pass.
#[derive(Debug, Clone)]
pub struct PendingLookupUpdate {
    /// The content_id set on the Create operation (source UUID string).
    pub content_id: String,
    /// Target entity logical name.
    pub entity: String,
    /// Target entity set name.
    pub entity_set: String,
    /// The target record ID, if the primary key was in `transformed` (known upfront).
    pub known_target_id: Option<Uuid>,
    /// Lookup fields to set (field_name → EntityReference value).
    pub lookup_fields: HashMap<String, Value>,
}

/// Result of generating the Create pass.
pub struct CreatePassResult {
    /// Batches per entity.
    pub entity_batches: Vec<EntityBatches>,
    /// Records that need lookup fields set in the Update pass.
    pub pending_lookups: Vec<PendingLookupUpdate>,
}

// =============================================================================
// Create Pass
// =============================================================================

/// Generate Create pass operations.
///
/// For each `RecordComparison` with `OperationType::Create`:
/// - Scalar fields go into the Create operation
/// - Lookup fields (EntityReference) are deferred to the Update pass
/// - State fields (statecode/statuscode) are skipped — handled by the Deactivate pass
/// - Primary key field is included if present in `transformed` (preserves source GUID)
/// - `content_id` is set to the source record UUID for correlation
///
/// For junction entities (is_intersect), records are skipped — they go in Associate pass.
pub fn generate_create_pass(
    comparisons: &[MappingComparison],
    metadata: &HashMap<String, ExecutionMetadata>,
) -> CreatePassResult {
    let mut all_entity_batches = Vec::new();
    let mut all_pending = Vec::new();

    for mapping in comparisons {
        let Some(meta) = metadata.get(&mapping.target_entity) else {
            continue;
        };

        // Junction entities don't get created — they go through Associate pass
        if meta.is_intersect {
            continue;
        }

        let mut operations = Vec::new();

        for record in &mapping.records {
            if record.operation != OperationType::Create {
                continue;
            }

            let source_id = match record.source_id {
                Some(id) => id,
                None => continue,
            };

            let content_id = source_id.to_string();
            let mut create_record = Record::new(Entity::set(&meta.entity_set_name));
            let mut lookup_fields = HashMap::new();
            let mut known_target_id = None;

            for (field, value) in &record.transformed {
                // Primary key: include in create so Dataverse uses our GUID
                if field == &meta.primary_key {
                    if let Value::Guid(id) = value {
                        known_target_id = Some(*id);
                        create_record.insert(field.clone(), value.clone());
                    }
                    continue;
                }

                // State fields: skip — always create as active.
                // The Deactivate pass will set the desired state afterwards.
                if field == "statecode" || field == "statuscode" {
                    continue;
                }

                // Lookup fields: defer to Update pass
                if is_lookup_value(value, field, meta) {
                    lookup_fields.insert(field.clone(), value.clone());
                    continue;
                }

                // Scalar field: include in Create
                create_record.insert(field.clone(), value.clone());
            }

            let op = Op::create(Entity::set(&meta.entity_set_name), create_record)
                .content_id(&content_id)
                .bypass_plugins()
                .bypass_flows()
                .bypass_sync_logic()
                .suppress_duplicate_detection()
                .build();

            operations.push(op);

            // Track pending lookup updates if there are deferred lookup fields
            if !lookup_fields.is_empty() {
                all_pending.push(PendingLookupUpdate {
                    content_id,
                    entity: mapping.target_entity.clone(),
                    entity_set: meta.entity_set_name.clone(),
                    known_target_id,
                    lookup_fields,
                });
            }
        }

        if !operations.is_empty() {
            all_entity_batches.push(EntityBatches {
                entity: mapping.target_entity.clone(),
                operation_count: operations.len(),
                batches: build_batches(operations),
            });
        }
    }

    CreatePassResult {
        entity_batches: all_entity_batches,
        pending_lookups: all_pending,
    }
}

// =============================================================================
// Activate Pass
// =============================================================================

/// Generate Activate pass operations.
///
/// For each `RecordComparison` with `OperationType::Update` where the target
/// record is currently inactive (`target_statecode` != 0):
/// - Build an Update operation setting `statecode=0` and `statuscode=1` (Active)
///
/// This must run before the Update pass because Dataverse rejects PATCH on
/// inactive records. After updates are applied, the Deactivate pass will
/// restore the desired inactive state if needed.
///
/// Junction entities are skipped (they cannot be deactivated/activated).
pub fn generate_activate_pass(
    comparisons: &[MappingComparison],
    metadata: &HashMap<String, ExecutionMetadata>,
) -> Vec<EntityBatches> {
    let mut all_entity_batches = Vec::new();

    for mapping in comparisons {
        let Some(meta) = metadata.get(&mapping.target_entity) else {
            continue;
        };

        // Junction entities can't be activated/deactivated
        if meta.is_intersect {
            continue;
        }

        let mut operations = Vec::new();

        for record in &mapping.records {
            if record.operation != OperationType::Update {
                continue;
            }

            let Some(target_id) = record.target_id else {
                continue;
            };

            // Check if the target record is currently inactive
            let is_inactive = record
                .target_statecode
                .as_ref()
                .map(|v| !is_active_statecode(v))
                .unwrap_or(false);

            if !is_inactive {
                continue;
            }

            // Build an Update to reactivate using metadata-derived statuscode
            let mut activate_record = Record::new(Entity::set(&meta.entity_set_name));
            activate_record.insert("statecode", OptionSetValue::new(0));
            activate_record.insert(
                "statuscode",
                OptionSetValue::new(meta.default_active_statuscode),
            );

            let op = Op::update(
                Entity::set(&meta.entity_set_name),
                target_id,
                activate_record,
            )
            .bypass_plugins()
            .bypass_flows()
            .bypass_sync_logic()
            .build();

            operations.push(op);
        }

        if !operations.is_empty() {
            all_entity_batches.push(EntityBatches {
                entity: mapping.target_entity.clone(),
                operation_count: operations.len(),
                batches: build_batches(operations),
            });
        }
    }

    all_entity_batches
}

// =============================================================================
// Update Pass
// =============================================================================

/// Generate Update pass operations.
///
/// Two sources:
/// 1. Lookup fields deferred from Create pass (using known or captured target IDs)
/// 2. Diff-based updates on existing records (`OperationType::Update`)
///
/// State fields (statecode/statuscode) are excluded from diffs — they are
/// handled by the Activate and Deactivate passes.
pub fn generate_update_pass(
    comparisons: &[MappingComparison],
    metadata: &HashMap<String, ExecutionMetadata>,
    pending_lookups: &[PendingLookupUpdate],
    captured_ids: &HashMap<String, Uuid>,
) -> Result<Vec<EntityBatches>, String> {
    let mut all_entity_batches = Vec::new();

    // Group pending lookups by entity for batching
    let mut pending_by_entity: HashMap<&str, Vec<&PendingLookupUpdate>> = HashMap::new();
    for pending in pending_lookups {
        pending_by_entity
            .entry(&pending.entity)
            .or_default()
            .push(pending);
    }

    for mapping in comparisons {
        let Some(meta) = metadata.get(&mapping.target_entity) else {
            continue;
        };

        // Junction entities don't get updated
        if meta.is_intersect {
            continue;
        }

        let mut operations = Vec::new();

        // 1. Lookup-setting on created records
        if let Some(pendings) = pending_by_entity.get(mapping.target_entity.as_str()) {
            for pending in pendings {
                let target_id = pending
                    .known_target_id
                    .or_else(|| captured_ids.get(&pending.content_id).copied());

                let Some(target_id) = target_id else {
                    log::warn!(
                        "No target ID for created record {} — skipping lookup update",
                        pending.content_id
                    );
                    continue;
                };

                let mut update_record = Record::new(Entity::set(&pending.entity_set));
                for (field, value) in &pending.lookup_fields {
                    let bound = to_binding(value, field, meta, metadata)?;
                    let odata_name = lookup_odata_name(field, meta)?;
                    update_record.insert(odata_name.to_string(), bound);
                }

                let op = Op::update(Entity::set(&pending.entity_set), target_id, update_record)
                    .bypass_plugins()
                    .bypass_flows()
                    .bypass_sync_logic()
                    .suppress_duplicate_detection()
                    .build();

                operations.push(op);
            }
        }

        // 2. Diff-based updates on existing records
        for record in &mapping.records {
            if record.operation != OperationType::Update {
                continue;
            }

            let Some(target_id) = record.target_id else {
                continue;
            };

            if record.diffs.is_empty() {
                continue;
            }

            let mut update_record = Record::new(Entity::set(&meta.entity_set_name));
            for diff in &record.diffs {
                // State fields are handled by Activate/Deactivate passes
                if diff.field == "statecode" || diff.field == "statuscode" {
                    continue;
                }

                // Lookup fields need conversion to EntityBinding for @odata.bind format
                if is_lookup_value(&diff.new_value, &diff.field, meta) {
                    let bound = to_binding(&diff.new_value, &diff.field, meta, metadata)?;
                    let odata_name = lookup_odata_name(&diff.field, meta)?;
                    update_record.insert(odata_name.to_string(), bound);
                    continue;
                }

                update_record.insert(diff.field.clone(), diff.new_value.clone());
            }

            // Skip if all diffs were state fields (nothing left to update)
            if update_record.fields().is_empty() {
                continue;
            }

            let op = Op::update(Entity::set(&meta.entity_set_name), target_id, update_record)
                .bypass_plugins()
                .bypass_flows()
                .bypass_sync_logic()
                .suppress_duplicate_detection()
                .build();

            operations.push(op);
        }

        if !operations.is_empty() {
            all_entity_batches.push(EntityBatches {
                entity: mapping.target_entity.clone(),
                operation_count: operations.len(),
                batches: build_batches(operations),
            });
        }
    }

    Ok(all_entity_batches)
}

// =============================================================================
// Associate Pass
// =============================================================================

/// Generate Associate pass operations for junction entities.
///
/// For each `RecordComparison` with `OperationType::Associate`:
/// - Extract entity1 and entity2 IDs from the transformed fields
/// - Build an Associate operation using the N:N relationship metadata
pub fn generate_associate_pass(
    comparisons: &[MappingComparison],
    metadata: &HashMap<String, ExecutionMetadata>,
) -> Vec<EntityBatches> {
    let mut all_entity_batches = Vec::new();

    for mapping in comparisons {
        let Some(meta) = metadata.get(&mapping.target_entity) else {
            continue;
        };

        if !meta.is_intersect {
            continue;
        }

        let Some(rel) = &meta.junction_relationship else {
            log::warn!(
                "Junction entity {} has no relationship metadata — skipping Associate pass",
                mapping.target_entity
            );
            continue;
        };

        let mut operations = Vec::new();

        for record in &mapping.records {
            if record.operation != OperationType::Associate {
                continue;
            }

            if let Some(op) = build_associate_op(record, rel, metadata) {
                operations.push(op);
            }
        }

        if !operations.is_empty() {
            all_entity_batches.push(EntityBatches {
                entity: mapping.target_entity.clone(),
                operation_count: operations.len(),
                batches: build_batches(operations),
            });
        }
    }

    all_entity_batches
}

// =============================================================================
// Disassociate Pass
// =============================================================================

/// Generate Disassociate pass operations for orphan junction records.
///
/// For each `OrphanRecord` with `OperationType::Disassociate`:
/// - We don't have the FK values from the orphan (only record_id)
/// - We'd need the junction record's fields to know which records to disassociate
/// - For now, this falls back to Delete on the junction entity
pub fn generate_disassociate_pass(
    comparisons: &[MappingComparison],
    metadata: &HashMap<String, ExecutionMetadata>,
) -> Vec<EntityBatches> {
    let mut all_entity_batches = Vec::new();

    for mapping in comparisons {
        let Some(meta) = metadata.get(&mapping.target_entity) else {
            continue;
        };

        if !meta.is_intersect {
            continue;
        }

        let Some(rel) = &meta.junction_relationship else {
            continue;
        };

        let mut operations = Vec::new();

        for orphan in &mapping.orphans {
            if orphan.operation != OperationType::Disassociate {
                continue;
            }

            if let Some(op) = build_disassociate_op(orphan, rel, metadata) {
                operations.push(op);
            }
        }

        if !operations.is_empty() {
            all_entity_batches.push(EntityBatches {
                entity: mapping.target_entity.clone(),
                operation_count: operations.len(),
                batches: build_batches(operations),
            });
        }
    }

    all_entity_batches
}

// =============================================================================
// Deactivate Pass
// =============================================================================

/// Generate Deactivate pass operations.
///
/// Two sources of deactivation:
///
/// **Source A: Orphan records** — `OrphanRecord` with `OperationType::Deactivate`.
/// Sets `statecode=1, statuscode=2` (standard Inactive).
///
/// **Source B: State restoration** — records that were created or updated but need
/// to end up inactive. For each `RecordComparison` with `OperationType::Create` or
/// `OperationType::Update` whose transformed `statecode` is not active:
/// - For Create: uses `known_target_id` from pending_lookups or `captured_ids`
/// - For Update: uses `record.target_id`
/// Sets `statecode` and `statuscode` to the values from the transformed map.
///
/// Junction entities are skipped.
pub fn generate_deactivate_pass(
    comparisons: &[MappingComparison],
    metadata: &HashMap<String, ExecutionMetadata>,
    pending_lookups: &[PendingLookupUpdate],
    captured_ids: &HashMap<String, Uuid>,
) -> Vec<EntityBatches> {
    let mut all_entity_batches = Vec::new();

    for mapping in comparisons {
        let Some(meta) = metadata.get(&mapping.target_entity) else {
            continue;
        };

        // Junction entities can't be deactivated
        if meta.is_intersect {
            continue;
        }

        let mut operations = Vec::new();

        // Source A: Orphan deactivation
        for orphan in &mapping.orphans {
            if orphan.operation != OperationType::Deactivate {
                continue;
            }

            let Some(record_id) = orphan.record_id else {
                continue;
            };

            let mut deactivate_record = Record::new(Entity::set(&meta.entity_set_name));
            deactivate_record.insert("statecode", OptionSetValue::new(1));
            deactivate_record.insert(
                "statuscode",
                OptionSetValue::new(meta.default_inactive_statuscode),
            );

            let op = Op::update(
                Entity::set(&meta.entity_set_name),
                record_id,
                deactivate_record,
            )
            .bypass_plugins()
            .bypass_flows()
            .bypass_sync_logic()
            .build();

            operations.push(op);
        }

        // Source B: State restoration for created/updated records that should be inactive
        for record in &mapping.records {
            let target_id = match record.operation {
                OperationType::Create => {
                    // Find the target ID from pending_lookups or captured_ids
                    let source_id = match record.source_id {
                        Some(id) => id,
                        None => continue,
                    };
                    let content_id = source_id.to_string();

                    // Check pending_lookups for a known_target_id first
                    let from_pending = pending_lookups
                        .iter()
                        .find(|p| p.content_id == content_id)
                        .and_then(|p| p.known_target_id);

                    match from_pending.or_else(|| captured_ids.get(&content_id).copied()) {
                        Some(id) => id,
                        None => {
                            log::warn!(
                                "No target ID for created record {} — skipping deactivation",
                                content_id
                            );
                            continue;
                        }
                    }
                }
                OperationType::Update => match record.target_id {
                    Some(id) => id,
                    None => continue,
                },
                _ => continue,
            };

            // Check if the transformed statecode is inactive
            let transformed_statecode = record.transformed.get("statecode");
            let needs_deactivation = transformed_statecode
                .map(|v| !is_active_statecode(v))
                .unwrap_or(false);

            if !needs_deactivation {
                continue;
            }

            // Use transformed statecode value; fall back to standard inactive
            let statecode_value = transformed_statecode
                .cloned()
                .unwrap_or_else(|| Value::OptionSet(OptionSetValue::new(1)));

            // Use transformed statuscode if present; fall back to metadata-derived inactive
            let statuscode_value = record
                .transformed
                .get("statuscode")
                .cloned()
                .unwrap_or_else(|| {
                    Value::OptionSet(OptionSetValue::new(meta.default_inactive_statuscode))
                });

            let mut deactivate_record = Record::new(Entity::set(&meta.entity_set_name));
            deactivate_record.insert("statecode", statecode_value);
            deactivate_record.insert("statuscode", statuscode_value);

            let op = Op::update(
                Entity::set(&meta.entity_set_name),
                target_id,
                deactivate_record,
            )
            .bypass_plugins()
            .bypass_flows()
            .bypass_sync_logic()
            .build();

            operations.push(op);
        }

        if !operations.is_empty() {
            all_entity_batches.push(EntityBatches {
                entity: mapping.target_entity.clone(),
                operation_count: operations.len(),
                batches: build_batches(operations),
            });
        }
    }

    all_entity_batches
}

// =============================================================================
// Delete Pass
// =============================================================================

/// Generate Delete pass operations.
///
/// For each `OrphanRecord` with `OperationType::Delete`:
/// - Build a Delete operation using the orphan's record_id
pub fn generate_delete_pass(
    comparisons: &[MappingComparison],
    metadata: &HashMap<String, ExecutionMetadata>,
) -> Vec<EntityBatches> {
    let mut all_entity_batches = Vec::new();

    for mapping in comparisons {
        let Some(meta) = metadata.get(&mapping.target_entity) else {
            continue;
        };

        // Junction entity orphans go through Disassociate, not Delete
        if meta.is_intersect {
            continue;
        }

        let mut operations = Vec::new();

        for orphan in &mapping.orphans {
            if orphan.operation != OperationType::Delete {
                continue;
            }

            let Some(record_id) = orphan.record_id else {
                continue;
            };

            let op = Op::delete(Entity::set(&meta.entity_set_name), record_id)
                .bypass_plugins()
                .bypass_flows()
                .bypass_sync_logic()
                .build();

            operations.push(op);
        }

        if !operations.is_empty() {
            all_entity_batches.push(EntityBatches {
                entity: mapping.target_entity.clone(),
                operation_count: operations.len(),
                batches: build_batches(operations),
            });
        }
    }

    all_entity_batches
}

// =============================================================================
// Helpers
// =============================================================================

/// Check if a value is a lookup field (EntityReference or null lookup).
fn is_lookup_value(value: &Value, field: &str, meta: &ExecutionMetadata) -> bool {
    let result = match value {
        Value::EntityReference { .. } => true,
        Value::EntityBinding(_) => meta.lookup_attributes.contains(field),
        Value::Null => meta.lookup_attributes.contains(field),
        _ => false,
    };
    log::debug!(
        "is_lookup_value check: field={}, value_type={:?}, is_lookup_attr={}, result={}",
        field,
        std::mem::discriminant(value),
        meta.lookup_attributes.contains(field),
        result
    );
    result
}

/// Convert a lookup value to the write format (`EntityBinding`) that serializes
/// as `"field@odata.bind"`.
///
/// - `EntityReference` → `EntityBinding` (resolves logical entity name → set name via metadata)
/// - `Null` → `EntityBinding::null(set_name)` (serializes as `"field@odata.bind": null`)
/// - Already an `EntityBinding` → returned as-is
///
/// `field` and `entity_meta` are used to resolve the target entity set name for
/// null lookups (which don't carry type information).
///
/// Returns `None` if the entity reference target is not in the metadata map
/// (unknown entity — can't resolve set name).
fn to_binding(
    value: &Value,
    field: &str,
    entity_meta: &ExecutionMetadata,
    metadata: &HashMap<String, ExecutionMetadata>,
) -> Result<Value, String> {
    match value {
        Value::EntityReference(er) => {
            let logical_name = er.entity.name();
            let target_meta = metadata.get(logical_name).ok_or_else(|| {
                format!(
                    "Cannot resolve entity set name for lookup target '{}' on field '{}' — \
                     entity not in metadata",
                    logical_name, field,
                )
            })?;
            Ok(Value::EntityBinding(er.bind(&target_meta.entity_set_name)))
        }
        Value::Null => {
            let set_name = entity_meta
                .lookup_targets
                .get(field)
                .and_then(|targets| targets.first())
                .and_then(|logical| metadata.get(logical))
                .map(|m| m.entity_set_name.as_str())
                .ok_or_else(|| {
                    format!(
                        "Cannot resolve entity set name for null lookup field '{}' on entity '{}'",
                        field, entity_meta.logical_name,
                    )
                })?;
            Ok(Value::EntityBinding(EntityBinding::null(set_name)))
        }
        Value::EntityBinding(_) => Ok(value.clone()),
        other => Err(format!(
            "to_binding called with non-lookup value type '{}' for field '{}'",
            other.type_name(),
            field,
        )),
    }
}

/// Resolve the OData field name for a lookup attribute.
///
/// Lookup fields must use the navigation property name (e.g., `nrq_CountryId`)
/// for `@odata.bind` annotations, not the logical attribute name (`nrq_countryid`).
///
/// Returns an error if the navigation property mapping is not found.
fn lookup_odata_name<'a>(field: &str, meta: &'a ExecutionMetadata) -> Result<&'a str, String> {
    meta.lookup_nav_properties
        .get(field)
        .map(|s| s.as_str())
        .ok_or_else(|| {
            format!(
                "No navigation property found for lookup '{}' on entity '{}' — \
                 cannot build @odata.bind annotation",
                field, meta.logical_name,
            )
        })
}

/// Check if a statecode value represents active (0).
fn is_active_statecode(value: &Value) -> bool {
    match value {
        Value::OptionSet(v) => v.value == 0,
        Value::Int(v) => *v == 0,
        _ => true, // Default to "active" if we can't determine
    }
}

/// Build an Associate operation from a junction record's transformed fields.
fn build_associate_op(
    record: &RecordComparison,
    rel: &ManyToManyRelationship,
    metadata: &HashMap<String, ExecutionMetadata>,
) -> Option<dataverse_lib::api::Operation> {
    // Extract entity1 and entity2 IDs from the junction's FK fields
    let e1_attr = rel.entity1_intersect_attribute.as_deref()?;
    let e2_attr = rel.entity2_intersect_attribute.as_deref()?;

    let e1_id = extract_uuid_from_value(record.transformed.get(e1_attr)?)?;
    let e2_id = extract_uuid_from_value(record.transformed.get(e2_attr)?)?;

    // Get entity set names for entity1 and entity2
    let e1_meta = metadata.get(&rel.entity1_logical_name)?;
    let e2_meta = metadata.get(&rel.entity2_logical_name)?;

    // Associate from entity1's perspective
    let nav_property = rel.entity1_navigation_property_name.as_deref()?;

    Some(
        Op::associate(
            Entity::set(&e1_meta.entity_set_name),
            e1_id,
            nav_property,
            Entity::set(&e2_meta.entity_set_name),
            e2_id,
        )
        .bypass_plugins()
        .bypass_flows()
        .bypass_sync_logic()
        .build(),
    )
}

/// Build a Disassociate operation from an orphan junction record.
///
/// Orphan records only have `record_id` (the junction row's PK), which is not
/// enough for a proper Disassociate call (we need both FK values). Since junction
/// rows ARE regular records, we fall back to deleting the junction row directly.
fn build_disassociate_op(
    orphan: &OrphanRecord,
    rel: &ManyToManyRelationship,
    _metadata: &HashMap<String, ExecutionMetadata>,
) -> Option<dataverse_lib::api::Operation> {
    let record_id = orphan.record_id?;

    // Junction rows can be deleted directly — we don't need to call Disassociate.
    // The intersect entity is the junction table itself.
    Some(
        Op::delete(Entity::set(&rel.intersect_entity_name), record_id)
            .bypass_plugins()
            .bypass_flows()
            .bypass_sync_logic()
            .build(),
    )
}

/// Extract a UUID from a Value (handles Guid, EntityReference, String).
fn extract_uuid_from_value(value: &Value) -> Option<Uuid> {
    match value {
        Value::Guid(id) => Some(*id),
        Value::EntityReference(er) => Some(er.id),
        Value::String(s) => Uuid::parse_str(s).ok(),
        _ => None,
    }
}

/// Group operations into batches of BATCH_SIZE with bypass headers.
fn build_batches(operations: Vec<dataverse_lib::api::Operation>) -> Vec<Batch> {
    operations
        .chunks(BATCH_SIZE)
        .map(|chunk| {
            let mut batch = Batch::new()
                .continue_on_error()
                .bypass_plugins()
                .bypass_flows()
                .bypass_sync_logic()
                .suppress_duplicate_detection();
            for op in chunk {
                batch = batch.add(op.clone());
            }
            batch
        })
        .collect()
}

// =============================================================================
// Convenience: Count total operations across entity batches
// =============================================================================

/// Sum up all operations across a list of entity batches.
pub fn total_operations(entity_batches: &[EntityBatches]) -> usize {
    entity_batches.iter().map(|eb| eb.operation_count).sum()
}

/// Sum up all batches across a list of entity batches.
pub fn total_batches(entity_batches: &[EntityBatches]) -> usize {
    entity_batches.iter().map(|eb| eb.batches.len()).sum()
}

// =============================================================================
// Execution State Types (used by the Execute page)
// =============================================================================

/// Overall status of the execution state machine.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ExecutionStatus {
    /// Not yet started.
    #[default]
    Idle,
    /// Currently executing sub-phases.
    Running,
    /// All sub-phases completed successfully.
    Complete,
    /// Execution failed (some sub-phase had unrecoverable errors).
    Failed,
    /// User cancelled the execution.
    Cancelled,
}

/// Progress tracking for a single entity within a sub-phase.
#[derive(Debug, Clone)]
pub struct EntityProgress {
    /// Entity logical name.
    pub entity: String,
    /// Total number of operations submitted.
    pub total: usize,
    /// Number of operations completed (success + failure).
    pub completed: usize,
    /// Number of operations that failed.
    pub failed: usize,
}

/// Progress tracking for a sub-phase.
#[derive(Debug, Clone)]
pub struct SubPhaseProgress {
    /// Which sub-phase this tracks.
    pub sub_phase: SubPhase,
    /// Per-entity progress within this sub-phase.
    pub entities: Vec<EntityProgress>,
    /// Status of this sub-phase.
    pub status: SubPhaseStatus,
}

/// Status of a single sub-phase.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SubPhaseStatus {
    /// Waiting for earlier sub-phases to complete.
    #[default]
    Waiting,
    /// Currently submitting/processing batches.
    Running,
    /// All operations completed (possibly with errors).
    Complete,
    /// Skipped (no operations or disabled).
    Skipped,
}

/// An error that occurred during execution.
#[derive(Debug, Clone)]
pub struct ExecutionError {
    /// Sub-phase where the error occurred.
    pub sub_phase: SubPhase,
    /// Entity the error pertains to.
    pub entity: String,
    /// Source record ID (if available).
    pub record_id: Option<String>,
    /// Error message.
    pub message: String,
}

// =============================================================================
// Execution Tree Node (for the Execute page tree widget)
// =============================================================================

/// Tree node for the execution progress display.
#[derive(Debug, Clone)]
pub enum ExecutionTreeNode {
    /// A sub-phase header (Create, Update, etc.)
    SubPhase {
        sub_phase: SubPhase,
        status: SubPhaseStatus,
    },
    /// An entity within a sub-phase.
    Entity {
        sub_phase: SubPhase,
        entity: String,
        total: usize,
        completed: usize,
        failed: usize,
        parent_status: SubPhaseStatus,
    },
}

impl rafter::widgets::TreeItem for ExecutionTreeNode {
    type Key = String;

    fn key(&self) -> String {
        match self {
            Self::SubPhase { sub_phase, .. } => format!("sp-{:?}", sub_phase),
            Self::Entity {
                sub_phase, entity, ..
            } => format!("sp-{:?}-{}", sub_phase, entity),
        }
    }

    fn render(&self) -> tuidom::Element {
        use crate::widgets::BrailleSpinner;
        use rafter::element;
        use rafter::widgets::Text;
        use tuidom::Color;

        match self {
            Self::SubPhase { sub_phase, status } => {
                let label_color = Color::var(match status {
                    SubPhaseStatus::Running => "primary",
                    SubPhaseStatus::Complete => "success",
                    SubPhaseStatus::Skipped | SubPhaseStatus::Waiting => "muted",
                });

                let status_el = match status {
                    SubPhaseStatus::Waiting => {
                        element! { text (content: "waiting") style (fg: muted) }
                    }
                    SubPhaseStatus::Running => BrailleSpinner::new()
                        .id(format!("sp-{:?}", sub_phase))
                        .build_standalone(),
                    SubPhaseStatus::Complete => {
                        element! { text (content: "done") style (fg: success) }
                    }
                    SubPhaseStatus::Skipped => {
                        element! { text (content: "skipped") style (fg: muted) }
                    }
                };

                element! {
                    row (gap: 2) {
                        text (content: {sub_phase.label()}) style (bold, fg: {label_color})
                        { status_el }
                    }
                }
            }
            Self::Entity {
                entity,
                total,
                completed,
                failed,
                parent_status,
                ..
            } => {
                let show_counts = *parent_status == SubPhaseStatus::Running
                    || *parent_status == SubPhaseStatus::Complete;
                let count_color = Color::var(if *failed > 0 {
                    "error"
                } else if completed == total {
                    "success"
                } else {
                    "primary"
                });

                element! {
                    row (gap: 2) {
                        text (content: {entity.clone()}) style (fg: primary)
                        if show_counts {
                            text (content: {format!("{}/{}", completed, total)}) style (fg: {count_color})
                        }
                        if *failed > 0 {
                            text (content: {format!("{} failed", failed)}) style (fg: error)
                        }
                    }
                }
            }
        }
    }
}
