use chrono::{DateTime, LocalResult, TimeZone, Utc};
use chrono_tz::Europe::Brussels;
use dataverse_lib::api::{Batch, Op, Operation};
use dataverse_lib::model::types::EntityBinding;
use dataverse_lib::model::{Entity, Record, Value};
use uuid::Uuid;

use crate::apps::queue::api::NewItem;
use crate::apps::queue::types::QueuePayload;
use crate::systems::client_management::ActiveClientInfo;

use super::diff::{diff_associations, field_change_count};
use super::scope;
use super::types::{DeadlineMode, DeadlineRecord, LookupCache};

const BATCH_SIZE: usize = 50;

pub fn build_queue_items(
    records: &[DeadlineRecord],
    cache: &LookupCache,
    client: &ActiveClientInfo,
) -> Vec<NewItem> {
    let deadline_set = entity_set(cache, scope::ENTITY_DEADLINE);
    let mut create_ops = Vec::new();
    let mut update_ops = Vec::new();
    let mut remove_ops = Vec::new();
    let mut add_ops = Vec::new();

    for record in records {
        if !record.is_actionable() {
            continue;
        }
        match record.mode {
            DeadlineMode::Create => {
                create_ops.push(create_deadline_op(record, cache, &deadline_set));
                add_ops.extend(association_create_ops(record, cache, &deadline_set));
            }
            DeadlineMode::Update => {
                let field_ops = update_deadline_ops(record, cache, &deadline_set);
                update_ops.extend(field_ops);
                if let Some(existing) = &record.existing {
                    let association_diff = diff_associations(record, &existing.associations);
                    remove_ops.extend(association_remove_ops(
                        record,
                        cache,
                        &deadline_set,
                        &association_diff,
                    ));
                    add_ops.extend(association_add_ops(
                        record,
                        cache,
                        &deadline_set,
                        &association_diff,
                    ));
                }
            }
            DeadlineMode::Unchanged | DeadlineMode::Error(_) => {}
        }
    }

    let mut items = Vec::new();
    push_batches(
        &mut items,
        create_ops,
        client,
        70,
        "deadline-import",
        "Deadline creates",
    );
    push_batches(
        &mut items,
        update_ops,
        client,
        60,
        "deadline-import",
        "Deadline updates",
    );
    push_batches(
        &mut items,
        remove_ops,
        client,
        50,
        "deadline-import",
        "Deadline removals",
    );
    push_batches(
        &mut items,
        add_ops,
        client,
        40,
        "deadline-import",
        "Deadline additions",
    );
    items
}

pub fn generated_name(record: &DeadlineRecord) -> Option<String> {
    let deadline_name = record.fields.direct.get("nrq_deadlinename")?;
    let mut parts = vec![deadline_name.clone()];
    if let Some(date) = record.fields.deadline_date {
        let time = record
            .fields
            .deadline_time
            .unwrap_or_else(|| chrono::NaiveTime::from_hms_opt(12, 0, 0).unwrap());
        parts.push(format!(
            "{} {}",
            date.format("%d/%m/%Y"),
            time.format("%H:%M")
        ));
    }
    Some(parts.join(" - "))
}

pub fn deadline_datetime(record: &DeadlineRecord) -> Option<DateTime<Utc>> {
    record
        .fields
        .deadline_date
        .and_then(|date| brussels_datetime(date, record.fields.deadline_time).ok())
}

pub fn committee_datetime(record: &DeadlineRecord) -> Option<DateTime<Utc>> {
    record
        .fields
        .committee_date
        .and_then(|date| brussels_datetime(date, record.fields.committee_time).ok())
}

fn create_deadline_op(
    record: &DeadlineRecord,
    cache: &LookupCache,
    deadline_set: &str,
) -> Operation {
    let mut payload = build_full_payload(record, cache, deadline_set);
    payload.insert("nrq_deadlineid", Value::Guid(record.id));
    Op::create(Entity::set(deadline_set), payload)
        .content_id(record.id.to_string())
        .bypass_plugins()
        .bypass_flows()
        .bypass_sync_logic()
        .suppress_duplicate_detection()
        .build()
}

fn update_deadline_ops(
    record: &DeadlineRecord,
    cache: &LookupCache,
    deadline_set: &str,
) -> Vec<Operation> {
    if field_change_count(record) == 0 {
        return Vec::new();
    }
    let payload = build_full_payload(record, cache, deadline_set);
    if payload.fields().is_empty() {
        return Vec::new();
    }
    vec![
        Op::update(Entity::set(deadline_set), record.id, payload)
            .content_id(record.id.to_string())
            .bypass_plugins()
            .bypass_flows()
            .bypass_sync_logic()
            .build(),
    ]
}

fn build_full_payload(
    record: &DeadlineRecord,
    _cache: &LookupCache,
    _deadline_set: &str,
) -> Record {
    let mut payload = Record::new(Entity::set(scope::ENTITY_DEADLINE));

    for (field, value) in scope::constant_fields() {
        payload.insert(field, value);
    }
    for (field, value) in &record.fields.direct {
        payload.insert(field.clone(), value.clone());
    }
    if let Some(name) = generated_name(record) {
        payload.insert("nrq_name", name);
    }
    if let Some(datetime) = deadline_datetime(record) {
        payload.insert("nrq_deadlinedate", datetime);
    }
    if let Some(datetime) = committee_datetime(record) {
        payload.insert("nrq_committeemeetingdate", datetime);
    }
    for (field, lookup) in &record.fields.lookups {
        payload.insert(
            field.clone(),
            Value::EntityBinding(EntityBinding::new(lookup.target_set.clone(), lookup.id)),
        );
    }
    for (field, value) in &record.fields.picklists {
        payload.insert(field.clone(), *value);
    }
    for (field, value) in &record.fields.booleans {
        payload.insert(field.clone(), *value);
    }

    payload
}

fn association_create_ops(
    record: &DeadlineRecord,
    cache: &LookupCache,
    deadline_set: &str,
) -> Vec<Operation> {
    let mut ops = Vec::new();
    for (id, name) in &record.associations.category {
        ops.push(associate_op(
            deadline_set,
            record.id,
            scope::REL_CATEGORY,
            cache,
            scope::ENTITY_CATEGORY,
            *id,
            name,
        ));
    }
    for (id, name) in &record.associations.subcategory {
        ops.push(associate_op(
            deadline_set,
            record.id,
            scope::REL_SUBCATEGORY,
            cache,
            scope::ENTITY_SUBCATEGORY,
            *id,
            name,
        ));
    }
    for (id, name) in &record.associations.flemishshare {
        ops.push(associate_op(
            deadline_set,
            record.id,
            scope::REL_FLEMISHSHARE,
            cache,
            scope::ENTITY_FLEMISHSHARE,
            *id,
            name,
        ));
    }
    for (id, name) in &record.associations.support {
        ops.push(create_support_junction_op(record.id, *id, name, cache));
    }
    ops
}

fn association_add_ops(
    record: &DeadlineRecord,
    cache: &LookupCache,
    deadline_set: &str,
    diff: &super::types::AssociationDiff,
) -> Vec<Operation> {
    let mut ops = Vec::new();
    for (id, name) in &diff.category_to_add {
        ops.push(associate_op(
            deadline_set,
            record.id,
            scope::REL_CATEGORY,
            cache,
            scope::ENTITY_CATEGORY,
            *id,
            name,
        ));
    }
    for (id, name) in &diff.subcategory_to_add {
        ops.push(associate_op(
            deadline_set,
            record.id,
            scope::REL_SUBCATEGORY,
            cache,
            scope::ENTITY_SUBCATEGORY,
            *id,
            name,
        ));
    }
    for (id, name) in &diff.flemishshare_to_add {
        ops.push(associate_op(
            deadline_set,
            record.id,
            scope::REL_FLEMISHSHARE,
            cache,
            scope::ENTITY_FLEMISHSHARE,
            *id,
            name,
        ));
    }
    for (id, name) in &diff.support_to_add {
        ops.push(create_support_junction_op(record.id, *id, name, cache));
    }
    ops
}

fn association_remove_ops(
    record: &DeadlineRecord,
    cache: &LookupCache,
    deadline_set: &str,
    diff: &super::types::AssociationDiff,
) -> Vec<Operation> {
    let mut ops = Vec::new();
    for (id, _) in &diff.category_to_remove {
        ops.push(disassociate_op(
            deadline_set,
            record.id,
            scope::REL_CATEGORY,
            *id,
        ));
    }
    for (id, _) in &diff.subcategory_to_remove {
        ops.push(disassociate_op(
            deadline_set,
            record.id,
            scope::REL_SUBCATEGORY,
            *id,
        ));
    }
    for (id, _) in &diff.flemishshare_to_remove {
        ops.push(disassociate_op(
            deadline_set,
            record.id,
            scope::REL_FLEMISHSHARE,
            *id,
        ));
    }
    let junction_set = entity_set(cache, scope::ENTITY_DEADLINE_SUPPORT);
    for junction in &diff.support_to_remove {
        ops.push(
            Op::delete(Entity::set(&junction_set), junction.junction_id)
                .content_id(junction.junction_id.to_string())
                .bypass_plugins()
                .bypass_flows()
                .bypass_sync_logic()
                .build(),
        );
    }
    ops
}

fn associate_op(
    deadline_set: &str,
    deadline_id: Uuid,
    relationship: &str,
    cache: &LookupCache,
    target_entity: &str,
    target_id: Uuid,
    _name: &str,
) -> Operation {
    let target_set = entity_set(cache, target_entity);
    Op::associate(
        Entity::set(deadline_set),
        deadline_id,
        relationship,
        Entity::set(&target_set),
        target_id,
    )
    .content_id(format!(
        "associate-{relationship}-{deadline_id}-{target_id}"
    ))
    .bypass_plugins()
    .bypass_flows()
    .bypass_sync_logic()
    .suppress_duplicate_detection()
    .build()
}

fn disassociate_op(
    deadline_set: &str,
    deadline_id: Uuid,
    relationship: &str,
    target_id: Uuid,
) -> Operation {
    Op::disassociate(
        Entity::set(deadline_set),
        deadline_id,
        relationship,
        target_id,
    )
    .content_id(format!(
        "disassociate-{relationship}-{deadline_id}-{target_id}"
    ))
    .bypass_plugins()
    .bypass_flows()
    .bypass_sync_logic()
    .build()
}

fn create_support_junction_op(
    deadline_id: Uuid,
    support_id: Uuid,
    support_name: &str,
    cache: &LookupCache,
) -> Operation {
    let deadline_set = entity_set(cache, scope::ENTITY_DEADLINE);
    let support_set = entity_set(cache, scope::ENTITY_SUPPORT);
    let junction_set = entity_set(cache, scope::ENTITY_DEADLINE_SUPPORT);
    let mut payload = Record::new(Entity::set(&junction_set));
    payload.insert(
        "nrq_DeadlineId",
        Value::EntityBinding(EntityBinding::new(deadline_set, deadline_id)),
    );
    payload.insert(
        "nrq_SupportId",
        Value::EntityBinding(EntityBinding::new(support_set, support_id)),
    );
    payload.insert("nrq_name", support_name.to_string());
    payload.insert("nrq_enablehearing", false);
    payload.insert("nrq_enablereporter", true);

    Op::create(Entity::set(&junction_set), payload)
        .content_id(format!("deadline-support-{deadline_id}-{support_id}"))
        .bypass_plugins()
        .bypass_flows()
        .bypass_sync_logic()
        .suppress_duplicate_detection()
        .build()
}

fn push_batches(
    items: &mut Vec<NewItem>,
    operations: Vec<Operation>,
    client: &ActiveClientInfo,
    priority: i32,
    source: &str,
    description: &str,
) {
    for (index, chunk) in operations.chunks(BATCH_SIZE).enumerate() {
        let mut batch = Batch::new()
            .continue_on_error()
            .bypass_plugins()
            .bypass_flows()
            .bypass_sync_logic();
        for op in chunk {
            batch = batch.add(op.clone());
        }
        items.push(NewItem {
            priority,
            payload: QueuePayload::Batch(batch),
            env_id: client.env_id,
            account_id: client.account_id,
            source: source.to_string(),
            description: format!(
                "{} batch {} ({} operations)",
                description,
                index + 1,
                chunk.len()
            ),
        });
    }
}

fn entity_set(cache: &LookupCache, entity: &str) -> String {
    cache
        .entity_sets
        .get(entity)
        .cloned()
        .unwrap_or_else(|| format!("{}s", entity))
}

fn brussels_datetime(
    date: chrono::NaiveDate,
    time: Option<chrono::NaiveTime>,
) -> Result<DateTime<Utc>, String> {
    let local_time = time.unwrap_or_else(|| chrono::NaiveTime::from_hms_opt(12, 0, 0).unwrap());
    let local = date.and_time(local_time);
    match Brussels.from_local_datetime(&local) {
        LocalResult::Single(value) => Ok(value.with_timezone(&Utc)),
        LocalResult::Ambiguous(earlier, _) => Ok(earlier.with_timezone(&Utc)),
        LocalResult::None => Err(format!("Invalid Brussels time (DST gap): {local}")),
    }
}
