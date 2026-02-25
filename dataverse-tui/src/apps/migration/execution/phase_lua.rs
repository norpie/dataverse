//! Phase-level Lua batch building — converts parsed Lua operations into
//! `EntityBatches` per sub-phase, ready for queue submission.
//!
//! This bypasses the comparison-based `generate_*_pass` functions entirely.
//! The Lua script controls exactly what operations are performed.

use std::collections::HashMap;

use dataverse_lib::api::Op;
use dataverse_lib::model::metadata::ExecutionMetadata;
use dataverse_lib::model::types::OptionSetValue;
use dataverse_lib::model::Entity;
use dataverse_lib::model::Record;
use dataverse_lib::model::Value;
use uuid::Uuid;

use super::build_batches;
use super::is_lookup_value;
use super::lookup_odata_name;
use super::to_binding;
use super::EntityBatches;
use super::SubPhase;
use crate::apps::migration::comparison::OperationTypeCounts;

// =============================================================================
// Types
// =============================================================================

/// A single operation parsed from a phase-level Lua script.
#[derive(Debug, Clone)]
pub enum PhaseLuaOperation {
    Create {
        entity: String,
        id: Uuid,
        fields: HashMap<String, Value>,
    },
    Update {
        entity: String,
        id: Uuid,
        fields: HashMap<String, Value>,
    },
    Activate {
        entity: String,
        id: Uuid,
    },
    Deactivate {
        entity: String,
        id: Uuid,
    },
    Delete {
        entity: String,
        id: Uuid,
    },
    Associate {
        entity1: String,
        id1: Uuid,
        entity2: String,
        id2: Uuid,
        relationship: String,
    },
    Disassociate {
        entity1: String,
        id1: Uuid,
        entity2: String,
        id2: Uuid,
        relationship: String,
    },
}

impl PhaseLuaOperation {
    /// Which sub-phase this operation belongs to.
    pub fn sub_phase(&self) -> SubPhase {
        match self {
            PhaseLuaOperation::Create { .. } => SubPhase::Create,
            PhaseLuaOperation::Activate { .. } => SubPhase::Activate,
            PhaseLuaOperation::Update { .. } => SubPhase::Update,
            PhaseLuaOperation::Associate { .. } => SubPhase::Associate,
            PhaseLuaOperation::Disassociate { .. } => SubPhase::Disassociate,
            PhaseLuaOperation::Deactivate { .. } => SubPhase::Deactivate,
            PhaseLuaOperation::Delete { .. } => SubPhase::Delete,
        }
    }

    /// The primary entity logical name for this operation.
    pub fn entity(&self) -> &str {
        match self {
            PhaseLuaOperation::Create { entity, .. }
            | PhaseLuaOperation::Update { entity, .. }
            | PhaseLuaOperation::Activate { entity, .. }
            | PhaseLuaOperation::Deactivate { entity, .. }
            | PhaseLuaOperation::Delete { entity, .. } => entity,
            PhaseLuaOperation::Associate { entity1, .. }
            | PhaseLuaOperation::Disassociate { entity1, .. } => entity1,
        }
    }
}

/// Resolved N:N relationship metadata for associate/disassociate operations.
#[derive(Debug, Clone)]
pub struct ResolvedRelationship {
    /// Entity set name for entity1 (e.g., "accounts").
    pub entity1_set: String,
    /// Entity set name for entity2 (e.g., "contacts").
    pub entity2_set: String,
    /// Navigation property on entity1 pointing to entity2 (e.g., "account_contacts").
    pub nav_property: String,
}

/// Result of building phase-level Lua batches.
pub struct PhaseLuaResult {
    /// Pre-built batches per sub-phase, ready for queue submission.
    pub batches: HashMap<SubPhase, Vec<EntityBatches>>,
    /// Operation counts per type (for the confirmation modal).
    pub counts: OperationTypeCounts,
}

// =============================================================================
// Public API
// =============================================================================

/// Convert parsed Lua operations into `EntityBatches` per sub-phase.
///
/// Each operation is converted to a Dataverse `Operation` using the appropriate
/// `Op` builder. Operations are grouped by `(sub_phase, entity)` and batched
/// into groups of 50.
///
/// For create/update: lookup fields (`EntityBinding`) are included directly —
/// no deferral to the Update pass.
///
/// For associate/disassociate: relationship metadata must be pre-resolved and
/// provided via `relationships`.
pub fn build_phase_lua_batches(
    operations: Vec<PhaseLuaOperation>,
    metadata: &HashMap<String, ExecutionMetadata>,
    relationships: &HashMap<String, ResolvedRelationship>,
) -> Result<PhaseLuaResult, String> {
    let mut counts = OperationTypeCounts::default();

    // Group operations by (sub_phase, entity)
    let mut grouped: HashMap<(SubPhase, String), Vec<dataverse_lib::api::Operation>> =
        HashMap::new();

    for lua_op in operations {
        let sub_phase = lua_op.sub_phase();
        let entity_name = lua_op.entity().to_string();

        // Count
        match &lua_op {
            PhaseLuaOperation::Create { .. } => counts.create += 1,
            PhaseLuaOperation::Update { .. } => counts.update += 1,
            PhaseLuaOperation::Activate { .. } => counts.update += 1, // activate is an update op
            PhaseLuaOperation::Deactivate { .. } => counts.deactivate += 1,
            PhaseLuaOperation::Delete { .. } => counts.delete += 1,
            PhaseLuaOperation::Associate { .. } => counts.associate += 1,
            PhaseLuaOperation::Disassociate { .. } => counts.disassociate += 1,
        }

        let op = build_operation(lua_op, metadata, relationships)?;

        grouped
            .entry((sub_phase, entity_name))
            .or_default()
            .push(op);
    }

    // Build EntityBatches per sub-phase
    let mut batches_by_phase: HashMap<SubPhase, Vec<EntityBatches>> = HashMap::new();

    for ((sub_phase, entity), ops) in grouped {
        let operation_count = ops.len();
        let batches = build_batches(ops);

        batches_by_phase
            .entry(sub_phase)
            .or_default()
            .push(EntityBatches {
                entity,
                batches,
                operation_count,
            });
    }

    log::info!(
        "[phase_lua] Built batches: {} creates, {} updates, {} activates (counted as updates), \
         {} deletes, {} deactivates, {} associates, {} disassociates",
        counts.create,
        counts.update,
        0, // activates are rolled into update count
        counts.delete,
        counts.deactivate,
        counts.associate,
        counts.disassociate,
    );

    Ok(PhaseLuaResult {
        batches: batches_by_phase,
        counts,
    })
}

/// Collect all unique entity logical names referenced by the operations.
///
/// This includes `entity`, `entity1`, and `entity2` fields from all operations.
/// Used to determine which entities need metadata resolution.
pub fn collect_referenced_entities(operations: &[PhaseLuaOperation]) -> Vec<String> {
    let mut entities = std::collections::HashSet::new();

    for op in operations {
        match op {
            PhaseLuaOperation::Create { entity, .. }
            | PhaseLuaOperation::Update { entity, .. }
            | PhaseLuaOperation::Activate { entity, .. }
            | PhaseLuaOperation::Deactivate { entity, .. }
            | PhaseLuaOperation::Delete { entity, .. } => {
                entities.insert(entity.clone());
            }
            PhaseLuaOperation::Associate {
                entity1, entity2, ..
            }
            | PhaseLuaOperation::Disassociate {
                entity1, entity2, ..
            } => {
                entities.insert(entity1.clone());
                entities.insert(entity2.clone());
            }
        }
    }

    entities.into_iter().collect()
}

/// Collect all unique relationship schema names from associate/disassociate operations.
///
/// Returns `(entity1_logical_name, relationship_schema_name)` pairs, which are
/// used to resolve `ManyToManyRelationship` metadata.
pub fn collect_referenced_relationships(operations: &[PhaseLuaOperation]) -> Vec<(String, String)> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();

    for op in operations {
        match op {
            PhaseLuaOperation::Associate {
                entity1,
                relationship,
                ..
            }
            | PhaseLuaOperation::Disassociate {
                entity1,
                relationship,
                ..
            } => {
                let key = (entity1.clone(), relationship.clone());
                if seen.insert(key.clone()) {
                    result.push(key);
                }
            }
            _ => {}
        }
    }

    result
}

// =============================================================================
// Operation building
// =============================================================================

/// Convert a single `PhaseLuaOperation` into a Dataverse `Operation`.
fn build_operation(
    lua_op: PhaseLuaOperation,
    metadata: &HashMap<String, ExecutionMetadata>,
    relationships: &HashMap<String, ResolvedRelationship>,
) -> Result<dataverse_lib::api::Operation, String> {
    match lua_op {
        PhaseLuaOperation::Create { entity, id, fields } => {
            build_create_op(&entity, id, fields, metadata)
        }
        PhaseLuaOperation::Update { entity, id, fields } => {
            build_update_op(&entity, id, fields, metadata)
        }
        PhaseLuaOperation::Activate { entity, id } => build_activate_op(&entity, id, metadata),
        PhaseLuaOperation::Deactivate { entity, id } => build_deactivate_op(&entity, id, metadata),
        PhaseLuaOperation::Delete { entity, id } => build_delete_op(&entity, id, metadata),
        PhaseLuaOperation::Associate {
            entity1,
            id1,
            entity2,
            id2,
            relationship,
        } => build_associate_op(&entity1, id1, &entity2, id2, &relationship, relationships),
        PhaseLuaOperation::Disassociate {
            entity1,
            id1,
            entity2: _,
            id2,
            relationship,
        } => build_disassociate_op(&entity1, id1, id2, &relationship, relationships),
    }
}

/// Build a Create operation with all fields (including lookups) directly.
fn build_create_op(
    entity: &str,
    id: Uuid,
    fields: HashMap<String, Value>,
    metadata: &HashMap<String, ExecutionMetadata>,
) -> Result<dataverse_lib::api::Operation, String> {
    let meta = resolve_meta(entity, metadata)?;
    let content_id = id.to_string();
    let mut record = Record::new(Entity::set(&meta.entity_set_name));

    // Include the primary key so Dataverse uses our preset UUID
    record.insert(meta.primary_key.clone(), Value::Guid(id));

    for (field, value) in &fields {
        // Skip statecode/statuscode — Activate/Deactivate pass handles these
        if field == "statecode" || field == "statuscode" {
            continue;
        }

        if is_lookup_value(value, field, meta) {
            let bound = to_binding(value, field, meta, metadata)?;
            let odata_name = lookup_odata_name(field, meta)?;
            record.insert(odata_name.to_string(), bound);
        } else {
            record.insert(field.clone(), value.clone());
        }
    }

    Ok(Op::create(Entity::set(&meta.entity_set_name), record)
        .content_id(&content_id)
        .bypass_plugins()
        .bypass_flows()
        .bypass_sync_logic()
        .suppress_duplicate_detection()
        .build())
}

/// Build an Update operation with only the specified fields (partial update).
fn build_update_op(
    entity: &str,
    id: Uuid,
    fields: HashMap<String, Value>,
    metadata: &HashMap<String, ExecutionMetadata>,
) -> Result<dataverse_lib::api::Operation, String> {
    let meta = resolve_meta(entity, metadata)?;
    let mut record = Record::new(Entity::set(&meta.entity_set_name));

    for (field, value) in &fields {
        if is_lookup_value(value, field, meta) {
            let bound = to_binding(value, field, meta, metadata)?;
            let odata_name = lookup_odata_name(field, meta)?;
            record.insert(odata_name.to_string(), bound);
        } else {
            record.insert(field.clone(), value.clone());
        }
    }

    Ok(Op::update(Entity::set(&meta.entity_set_name), id, record)
        .bypass_plugins()
        .bypass_flows()
        .bypass_sync_logic()
        .suppress_duplicate_detection()
        .build())
}

/// Build an Activate operation (set statecode=0, statuscode=default_active).
fn build_activate_op(
    entity: &str,
    id: Uuid,
    metadata: &HashMap<String, ExecutionMetadata>,
) -> Result<dataverse_lib::api::Operation, String> {
    let meta = resolve_meta(entity, metadata)?;
    let mut record = Record::new(Entity::set(&meta.entity_set_name));
    record.insert("statecode", OptionSetValue::new(0));
    record.insert(
        "statuscode",
        OptionSetValue::new(meta.default_active_statuscode),
    );

    Ok(Op::update(Entity::set(&meta.entity_set_name), id, record)
        .bypass_plugins()
        .bypass_flows()
        .bypass_sync_logic()
        .build())
}

/// Build a Deactivate operation (set statecode=1, statuscode=default_inactive).
fn build_deactivate_op(
    entity: &str,
    id: Uuid,
    metadata: &HashMap<String, ExecutionMetadata>,
) -> Result<dataverse_lib::api::Operation, String> {
    let meta = resolve_meta(entity, metadata)?;
    let mut record = Record::new(Entity::set(&meta.entity_set_name));
    record.insert("statecode", OptionSetValue::new(1));
    record.insert(
        "statuscode",
        OptionSetValue::new(meta.default_inactive_statuscode),
    );

    Ok(Op::update(Entity::set(&meta.entity_set_name), id, record)
        .bypass_plugins()
        .bypass_flows()
        .bypass_sync_logic()
        .build())
}

/// Build a Delete operation.
fn build_delete_op(
    entity: &str,
    id: Uuid,
    metadata: &HashMap<String, ExecutionMetadata>,
) -> Result<dataverse_lib::api::Operation, String> {
    let meta = resolve_meta(entity, metadata)?;

    Ok(Op::delete(Entity::set(&meta.entity_set_name), id)
        .bypass_plugins()
        .bypass_flows()
        .bypass_sync_logic()
        .build())
}

/// Build an Associate operation using pre-resolved relationship metadata.
fn build_associate_op(
    entity1: &str,
    id1: Uuid,
    entity2: &str,
    id2: Uuid,
    relationship: &str,
    relationships: &HashMap<String, ResolvedRelationship>,
) -> Result<dataverse_lib::api::Operation, String> {
    let rel = relationships.get(relationship).ok_or_else(|| {
        format!(
            "Relationship '{}' not resolved — cannot build associate for {} -> {}",
            relationship, entity1, entity2,
        )
    })?;

    Ok(Op::associate(
        Entity::set(&rel.entity1_set),
        id1,
        &rel.nav_property,
        Entity::set(&rel.entity2_set),
        id2,
    )
    .bypass_plugins()
    .bypass_flows()
    .bypass_sync_logic()
    .build())
}

/// Build a Disassociate operation using pre-resolved relationship metadata.
fn build_disassociate_op(
    entity1: &str,
    id1: Uuid,
    id2: Uuid,
    relationship: &str,
    relationships: &HashMap<String, ResolvedRelationship>,
) -> Result<dataverse_lib::api::Operation, String> {
    let rel = relationships.get(relationship).ok_or_else(|| {
        format!(
            "Relationship '{}' not resolved — cannot build disassociate for {}",
            relationship, entity1,
        )
    })?;

    Ok(
        Op::disassociate(Entity::set(&rel.entity1_set), id1, &rel.nav_property, id2)
            .bypass_plugins()
            .bypass_flows()
            .bypass_sync_logic()
            .build(),
    )
}

// =============================================================================
// Helpers
// =============================================================================

/// Look up `ExecutionMetadata` for an entity, returning a helpful error if missing.
fn resolve_meta<'a>(
    entity: &str,
    metadata: &'a HashMap<String, ExecutionMetadata>,
) -> Result<&'a ExecutionMetadata, String> {
    metadata.get(entity).ok_or_else(|| {
        format!(
            "No metadata for entity '{}' — was it included in M.declare() or \
             referenced by an operation?",
            entity,
        )
    })
}
