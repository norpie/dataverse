use std::collections::HashMap;
use std::collections::HashSet;

use dataverse_lib::DataverseClient;
use dataverse_lib::api::Batch;
use dataverse_lib::api::Op;
use dataverse_lib::api::Operation;
use dataverse_lib::model::Entity;
use dataverse_lib::model::Record;
use dataverse_lib::model::Value;
use dataverse_lib::model::metadata::ExecutionMetadata;
use dataverse_lib::model::types::EntityBinding;
use uuid::Uuid;

use crate::apps::migration::execution::EntityBatches;
use crate::apps::migration::execution::SubPhase;

use super::comparison::QuestionnaireComparison;
use super::comparison::QuestionnaireOperation;
use super::scope::QUESTIONNAIRE_ENTITIES;
use super::scope::QuestionnaireEntitySpec;
use super::scope::QuestionnaireFieldKind;
use super::scope::QuestionnaireFieldSpec;
use super::types::QuestionnaireExecutionPlan;

const BATCH_SIZE: usize = 50;

pub async fn fetch_execution_metadata(
    client: &DataverseClient,
) -> Result<HashMap<String, ExecutionMetadata>, String> {
    let mut metadata = HashMap::new();
    let mut logical_names = HashSet::new();

    for spec in QUESTIONNAIRE_ENTITIES {
        logical_names.insert(spec.logical_name);
        for field in spec.fields {
            if let QuestionnaireFieldKind::Lookup { target_entity } = field.kind {
                logical_names.insert(target_entity);
            }
        }
    }

    for logical_name in logical_names {
        let entity_metadata = client
            .metadata()
            .entity(Entity::logical(logical_name))
            .await
            .map_err(|e| format!("Failed to fetch metadata for {}: {}", logical_name, e))?;
        let execution_metadata = entity_metadata.execution_metadata().map_err(|e| {
            format!(
                "Failed to prepare execution metadata for {}: {}",
                logical_name, e
            )
        })?;
        metadata.insert(logical_name.to_string(), execution_metadata);
    }

    Ok(metadata)
}

pub fn build_execution_plan(
    comparison: &QuestionnaireComparison,
    metadata: &HashMap<String, ExecutionMetadata>,
) -> Result<QuestionnaireExecutionPlan, String> {
    let mut plan = QuestionnaireExecutionPlan::default();

    plan.push(
        SubPhase::Create,
        generate_create_pass(comparison, metadata)?,
    );
    plan.push(
        SubPhase::Activate,
        generate_activate_pass(comparison, metadata),
    );
    plan.push(
        SubPhase::Update,
        generate_update_pass(comparison, metadata)?,
    );
    plan.push(
        SubPhase::Associate,
        generate_associate_pass(comparison, metadata)?,
    );
    plan.push(
        SubPhase::Disassociate,
        generate_disassociate_pass(comparison, metadata)?,
    );
    plan.push(
        SubPhase::Deactivate,
        generate_deactivate_pass(comparison, metadata),
    );
    plan.push(SubPhase::Delete, generate_delete_pass(comparison, metadata));

    Ok(plan)
}

fn generate_create_pass(
    comparison: &QuestionnaireComparison,
    metadata: &HashMap<String, ExecutionMetadata>,
) -> Result<Vec<EntityBatches>, String> {
    let mut all_batches = Vec::new();

    for entity in &comparison.entities {
        let spec = entity_spec(&entity.entity)?;
        let meta = metadata_for(metadata, &entity.entity)?;
        let mut operations = Vec::new();

        for record in &entity.records {
            if record.operation != QuestionnaireOperation::Create {
                continue;
            }

            let Some(source_id) = record.source_id else {
                continue;
            };

            let mut create_record = Record::new(Entity::set(&meta.entity_set_name));
            create_record.insert(meta.primary_key.clone(), Value::Guid(source_id));

            for field in spec.fields {
                if matches!(field.kind, QuestionnaireFieldKind::Lookup { .. }) {
                    continue;
                }
                let Some(value) = record.source_record.get(field.field_name) else {
                    continue;
                };
                if value.is_null() {
                    continue;
                }
                insert_write_value(&mut create_record, field, value, meta, metadata)?;
            }

            operations.push(
                Op::create(Entity::set(&meta.entity_set_name), create_record)
                    .content_id(source_id.to_string())
                    .bypass_plugins()
                    .bypass_flows()
                    .bypass_sync_logic()
                    .suppress_duplicate_detection()
                    .build(),
            );
        }

        push_entity_batches(&mut all_batches, entity.entity.clone(), operations);
    }

    Ok(all_batches)
}

fn generate_activate_pass(
    comparison: &QuestionnaireComparison,
    metadata: &HashMap<String, ExecutionMetadata>,
) -> Vec<EntityBatches> {
    let mut all_batches = Vec::new();

    for entity in &comparison.entities {
        let Some(meta) = metadata.get(&entity.entity) else {
            continue;
        };
        let mut operations = Vec::new();

        for record in &entity.records {
            if record.operation != QuestionnaireOperation::Update || record.target_is_active {
                continue;
            }
            let Some(target_id) = record.target_id else {
                continue;
            };

            let mut update_record = Record::new(Entity::set(&meta.entity_set_name));
            update_record.insert("statecode", Value::Int(0));
            update_record.insert("statuscode", Value::Int(1));
            operations.push(update_op(meta, target_id, update_record));
        }

        push_entity_batches(&mut all_batches, entity.entity.clone(), operations);
    }

    all_batches
}

fn generate_update_pass(
    comparison: &QuestionnaireComparison,
    metadata: &HashMap<String, ExecutionMetadata>,
) -> Result<Vec<EntityBatches>, String> {
    let mut all_batches = Vec::new();

    for entity in &comparison.entities {
        let spec = entity_spec(&entity.entity)?;
        let meta = metadata_for(metadata, &entity.entity)?;
        let mut operations = Vec::new();

        for record in &entity.records {
            if !matches!(
                record.operation,
                QuestionnaireOperation::Create | QuestionnaireOperation::Update
            ) {
                continue;
            }
            let target_id = record.target_id.or(record.source_id);
            let Some(target_id) = target_id else {
                continue;
            };

            let mut update_record = Record::new(Entity::set(&meta.entity_set_name));
            if record.operation == QuestionnaireOperation::Create {
                for field in spec.fields {
                    if !matches!(field.kind, QuestionnaireFieldKind::Lookup { .. }) {
                        continue;
                    }
                    let Some(value) = record.source_record.get(field.field_name) else {
                        continue;
                    };
                    insert_write_value(&mut update_record, field, value, meta, metadata)?;
                }
            } else {
                for diff in &record.diffs {
                    if spec.state_fields.contains(&diff.field.as_str()) {
                        continue;
                    }
                    let Some(field) = spec
                        .fields
                        .iter()
                        .find(|field| field.field_name == diff.field)
                    else {
                        continue;
                    };
                    insert_write_value(
                        &mut update_record,
                        field,
                        &diff.source_value,
                        meta,
                        metadata,
                    )?;
                }
            }

            if !update_record.fields().is_empty() {
                operations.push(update_op(meta, target_id, update_record));
            }
        }

        push_entity_batches(&mut all_batches, entity.entity.clone(), operations);
    }

    Ok(all_batches)
}

fn generate_associate_pass(
    comparison: &QuestionnaireComparison,
    metadata: &HashMap<String, ExecutionMetadata>,
) -> Result<Vec<EntityBatches>, String> {
    let mut grouped: HashMap<String, Vec<Operation>> = HashMap::new();

    for relation in &comparison.relations {
        let parent_meta = metadata_for(metadata, &relation.parent_entity)?;
        let related_meta = metadata_for(metadata, &relation.related_entity)?;

        for membership in &relation.associations {
            grouped
                .entry(relation.parent_entity.clone())
                .or_default()
                .push(
                    Op::associate(
                        Entity::set(&parent_meta.entity_set_name),
                        membership.parent_id,
                        &relation.relationship_name,
                        Entity::set(&related_meta.entity_set_name),
                        membership.related_id,
                    )
                    .content_id(relation_content_id(
                        "associate",
                        &relation.relationship_name,
                        membership.parent_id,
                        membership.related_id,
                    ))
                    .bypass_plugins()
                    .bypass_flows()
                    .bypass_sync_logic()
                    .build(),
                );
        }
    }

    Ok(grouped_entity_batches(grouped))
}

fn generate_disassociate_pass(
    comparison: &QuestionnaireComparison,
    metadata: &HashMap<String, ExecutionMetadata>,
) -> Result<Vec<EntityBatches>, String> {
    let mut grouped: HashMap<String, Vec<Operation>> = HashMap::new();

    for relation in &comparison.relations {
        let parent_meta = metadata_for(metadata, &relation.parent_entity)?;

        for membership in &relation.disassociations {
            grouped
                .entry(relation.parent_entity.clone())
                .or_default()
                .push(
                    Op::disassociate(
                        Entity::set(&parent_meta.entity_set_name),
                        membership.parent_id,
                        &relation.relationship_name,
                        membership.related_id,
                    )
                    .content_id(relation_content_id(
                        "disassociate",
                        &relation.relationship_name,
                        membership.parent_id,
                        membership.related_id,
                    ))
                    .bypass_plugins()
                    .bypass_flows()
                    .bypass_sync_logic()
                    .build(),
                );
        }
    }

    Ok(grouped_entity_batches(grouped))
}

fn generate_deactivate_pass(
    comparison: &QuestionnaireComparison,
    metadata: &HashMap<String, ExecutionMetadata>,
) -> Vec<EntityBatches> {
    let mut all_batches = Vec::new();

    for entity in &comparison.entities {
        let Some(meta) = metadata.get(&entity.entity) else {
            continue;
        };
        let mut operations = Vec::new();

        for record in &entity.records {
            if record.source_is_active {
                continue;
            }
            let target_id = record.target_id.or(record.source_id);
            let Some(target_id) = target_id else {
                continue;
            };

            let mut update_record = Record::new(Entity::set(&meta.entity_set_name));
            update_record.insert("statecode", record.source_statecode.clone());
            update_record.insert("statuscode", record.source_statuscode.clone());
            operations.push(update_op(meta, target_id, update_record));
        }

        push_entity_batches(&mut all_batches, entity.entity.clone(), operations);
    }

    all_batches
}

fn generate_delete_pass(
    comparison: &QuestionnaireComparison,
    metadata: &HashMap<String, ExecutionMetadata>,
) -> Vec<EntityBatches> {
    let mut all_batches = Vec::new();

    for entity in &comparison.entities {
        let Some(meta) = metadata.get(&entity.entity) else {
            continue;
        };
        let operations: Vec<Operation> = entity
            .orphans
            .iter()
            .filter(|orphan| orphan.operation == QuestionnaireOperation::Delete)
            .filter_map(|orphan| orphan.record_id)
            .map(|target_id| {
                Op::delete(Entity::set(&meta.entity_set_name), target_id)
                    .content_id(target_id.to_string())
                    .bypass_plugins()
                    .bypass_flows()
                    .bypass_sync_logic()
                    .build()
            })
            .collect();

        push_entity_batches(&mut all_batches, entity.entity.clone(), operations);
    }

    all_batches
}

fn insert_write_value(
    record: &mut Record,
    field: &QuestionnaireFieldSpec,
    value: &Value,
    meta: &ExecutionMetadata,
    metadata: &HashMap<String, ExecutionMetadata>,
) -> Result<(), String> {
    match field.kind {
        QuestionnaireFieldKind::Value => {
            record.insert(field.field_name, value.clone());
        }
        QuestionnaireFieldKind::Lookup { target_entity } => {
            let nav_property = meta
                .lookup_nav_properties
                .get(field.field_name)
                .map(String::as_str)
                .unwrap_or(field.field_name);
            let target_set = metadata_for(metadata, target_entity)?
                .entity_set_name
                .clone();
            match value {
                Value::EntityReference(reference) => {
                    record.insert(nav_property, reference.bind(target_set));
                }
                Value::Guid(id) => {
                    record.insert(nav_property, EntityBinding::new(target_set, *id));
                }
                Value::Null => {
                    record.insert(nav_property, EntityBinding::null(target_set));
                }
                _ => {
                    return Err(format!(
                        "Lookup field {} has unsupported value type {}",
                        field.field_name,
                        value.type_name()
                    ));
                }
            }
        }
    }

    Ok(())
}

fn update_op(meta: &ExecutionMetadata, target_id: Uuid, record: Record) -> Operation {
    Op::update(Entity::set(&meta.entity_set_name), target_id, record)
        .content_id(target_id.to_string())
        .bypass_plugins()
        .bypass_flows()
        .bypass_sync_logic()
        .build()
}

fn push_entity_batches(
    all_batches: &mut Vec<EntityBatches>,
    entity: String,
    operations: Vec<Operation>,
) {
    if operations.is_empty() {
        return;
    }

    all_batches.push(EntityBatches {
        entity,
        operation_count: operations.len(),
        batches: build_batches(operations),
    });
}

fn grouped_entity_batches(grouped: HashMap<String, Vec<Operation>>) -> Vec<EntityBatches> {
    grouped
        .into_iter()
        .filter_map(|(entity, operations)| {
            if operations.is_empty() {
                None
            } else {
                Some(EntityBatches {
                    entity,
                    operation_count: operations.len(),
                    batches: build_batches(operations),
                })
            }
        })
        .collect()
}

fn build_batches(operations: Vec<Operation>) -> Vec<Batch> {
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

fn metadata_for<'a>(
    metadata: &'a HashMap<String, ExecutionMetadata>,
    logical_name: &str,
) -> Result<&'a ExecutionMetadata, String> {
    metadata
        .get(logical_name)
        .ok_or_else(|| format!("Missing execution metadata for {}", logical_name))
}

fn entity_spec(logical_name: &str) -> Result<&'static QuestionnaireEntitySpec, String> {
    QUESTIONNAIRE_ENTITIES
        .iter()
        .find(|spec| spec.logical_name == logical_name)
        .ok_or_else(|| format!("Missing questionnaire entity spec for {}", logical_name))
}

fn relation_content_id(
    prefix: &str,
    relationship: &str,
    parent_id: Uuid,
    related_id: Uuid,
) -> String {
    format!("{}:{}:{}:{}", prefix, relationship, parent_id, related_id)
}
