use std::collections::HashMap;

use dataverse_lib::DataverseClient;
use dataverse_lib::api::query::Filter;
use dataverse_lib::model::Entity;
use dataverse_lib::model::Record;
use dataverse_lib::model::Value;
use uuid::Uuid;

use crate::modals::odata_fetch::ODataFetchTask;

use super::scope;
use super::types::{ExistingAssociations, ExistingDeadline, ExistingJunctionRecord, LookupCache};

#[derive(Clone, Debug)]
pub struct FetchIndex {
    pub lookup_entities: Vec<String>,
    pub deadline_index: usize,
    pub junction_index: usize,
}

#[derive(Clone, Debug)]
pub struct MetadataMap {
    pub entity_sets: HashMap<String, String>,
}

pub async fn fetch_metadata(client: &DataverseClient) -> Result<MetadataMap, String> {
    let mut entity_sets = HashMap::new();
    for entity in all_entities() {
        let (set_name, _) = client
            .resolve_entity_core(entity)
            .await
            .map_err(|e| format!("Failed to resolve {entity}: {e}"))?;
        entity_sets.insert(entity.to_string(), set_name);
    }
    Ok(MetadataMap { entity_sets })
}

pub fn build_fetch_tasks(client: DataverseClient) -> (Vec<ODataFetchTask>, FetchIndex) {
    let mut tasks = Vec::new();
    let mut lookup_entities = Vec::new();

    for entity in scope::LOOKUP_ENTITIES {
        let mut query = client.query(Entity::logical(*entity)).bypass_cache();
        if *entity != "systemuser" {
            query = query.filter(Filter::eq("statecode", 0));
        }
        tasks.push(ODataFetchTask::new(
            format!("Lookup: {entity}"),
            client.clone(),
            query,
        ));
        lookup_entities.push(entity.to_string());
    }

    let deadline_index = tasks.len();
    let deadline_query = client
        .query(Entity::logical(scope::ENTITY_DEADLINE))
        .select(&[
            "nrq_deadlineid",
            "nrq_name",
            "nrq_deadlinename",
            "nrq_deadlinedate",
            "nrq_description",
            "nrq_committeemeetingdate",
            "nrq_committeemeetinginperson",
            "nrq_supporttypeoptionset",
            "_nrq_commissionid_value",
            "_nrq_domainid_value",
            "_nrq_fundid_value",
            "_nrq_projectmanagerid_value",
            "_nrq_typeid_value",
            "_nrq_boardofdirectorsmeetingid_value",
        ])
        .expand(scope::REL_CATEGORY, |expand| {
            expand.select(&["nrq_categoryid", "nrq_name"])
        })
        .expand(scope::REL_SUBCATEGORY, |expand| {
            expand.select(&["nrq_subcategoryid", "nrq_name"])
        })
        .expand(scope::REL_FLEMISHSHARE, |expand| {
            expand.select(&["nrq_flemishshareid", "nrq_name"])
        })
        .bypass_cache();
    tasks.push(ODataFetchTask::new(
        "Existing deadlines",
        client.clone(),
        deadline_query,
    ));

    let junction_index = tasks.len();
    let junction_query = client
        .query(Entity::logical(scope::ENTITY_DEADLINE_SUPPORT))
        .select(&[
            "nrq_deadlinesupportid",
            "_nrq_deadlineid_value",
            "_nrq_supportid_value",
            "nrq_name",
        ])
        .bypass_cache();
    tasks.push(ODataFetchTask::new(
        "Existing deadline support",
        client,
        junction_query,
    ));

    (
        tasks,
        FetchIndex {
            lookup_entities,
            deadline_index,
            junction_index,
        },
    )
}

pub fn build_import_context(
    results: Vec<Vec<Record>>,
    index: FetchIndex,
    metadata: MetadataMap,
) -> Result<(LookupCache, HashMap<Uuid, ExistingDeadline>), String> {
    let mut lookup_records = HashMap::new();
    for (task_index, entity) in index.lookup_entities.iter().enumerate() {
        let records = results
            .get(task_index)
            .ok_or_else(|| format!("Missing fetch result for {entity}"))?
            .clone();
        lookup_records.insert(entity.clone(), records);
    }

    let deadline_records = results
        .get(index.deadline_index)
        .ok_or_else(|| "Missing existing deadline fetch result".to_string())?
        .clone();
    let junction_records = results
        .get(index.junction_index)
        .ok_or_else(|| "Missing deadline support fetch result".to_string())?
        .clone();

    let mut associations_by_deadline: HashMap<Uuid, ExistingAssociations> = HashMap::new();

    for record in &deadline_records {
        let Some(deadline_id) = record.id() else {
            continue;
        };
        associations_by_deadline.insert(deadline_id, parse_expanded_associations(record));
    }

    for record in junction_records {
        let Some(junction_id) = record.id() else {
            continue;
        };
        let Some(deadline_id) = guid_field(&record, "_nrq_deadlineid_value") else {
            continue;
        };
        let Some(support_id) = guid_field(&record, "_nrq_supportid_value") else {
            continue;
        };
        let name = string_field(&record, "nrq_name").unwrap_or_default();
        associations_by_deadline
            .entry(deadline_id)
            .or_default()
            .support
            .insert(
                support_id,
                ExistingJunctionRecord {
                    junction_id,
                    related_id: support_id,
                    name,
                },
            );
    }

    let mut existing_deadlines = HashMap::new();
    for record in deadline_records {
        let Some(id) = record.id() else {
            continue;
        };
        let associations = associations_by_deadline.remove(&id).unwrap_or_default();
        if let Some(deadline) = ExistingDeadline::from_record(record, associations) {
            existing_deadlines.insert(id, deadline);
        }
    }

    let cache = LookupCache {
        records: lookup_records,
        entity_sets: metadata.entity_sets,
    };
    Ok((cache, existing_deadlines))
}

pub fn record_id(record: &Record, entity: &str) -> Option<Uuid> {
    record.id().or_else(|| {
        let id_field = match entity {
            "systemuser" => "systemuserid".to_string(),
            other => format!("{other}id"),
        };
        guid_field(record, &id_field)
    })
}

pub fn record_name(record: &Record, entity: &str) -> Option<String> {
    if entity == "systemuser" {
        if let Some(value) = string_field(record, "domainname") {
            return Some(value);
        }
        if let Some(value) = string_field(record, "internalemailaddress") {
            return Some(value);
        }
    }

    for field in ["name", "nrq_name", "fullname", "domainname"] {
        if let Some(value) = string_field(record, field) {
            return Some(value);
        }
    }
    None
}

pub fn string_field(record: &Record, field: &str) -> Option<String> {
    match record.get(field) {
        Some(Value::String(value)) => Some(value.clone()),
        Some(Value::Json(value)) => value.as_str().map(ToString::to_string),
        _ => None,
    }
}

pub fn guid_field(record: &Record, field: &str) -> Option<Uuid> {
    match record.get(field) {
        Some(Value::Guid(value)) => Some(*value),
        Some(Value::String(value)) => Uuid::parse_str(value).ok(),
        Some(Value::Json(value)) => value.as_str().and_then(|value| Uuid::parse_str(value).ok()),
        _ => None,
    }
}

fn parse_expanded_associations(record: &Record) -> ExistingAssociations {
    let mut associations = ExistingAssociations::default();
    parse_expanded_collection(
        record,
        scope::REL_CATEGORY,
        "nrq_categoryid",
        "nrq_name",
        &mut associations.category,
    );
    parse_expanded_collection(
        record,
        scope::REL_SUBCATEGORY,
        "nrq_subcategoryid",
        "nrq_name",
        &mut associations.subcategory,
    );
    parse_expanded_collection(
        record,
        scope::REL_FLEMISHSHARE,
        "nrq_flemishshareid",
        "nrq_name",
        &mut associations.flemishshare,
    );
    associations
}

fn parse_expanded_collection(
    record: &Record,
    relationship: &str,
    id_field: &str,
    name_field: &str,
    output: &mut HashMap<Uuid, String>,
) {
    let Some(Value::Records(records)) = record.get(relationship) else {
        return;
    };
    for related in records {
        let Some(id) = related.id().or_else(|| guid_field(related, id_field)) else {
            continue;
        };
        let name = string_field(related, name_field).unwrap_or_else(|| id.to_string());
        output.insert(id, name);
    }
}

fn all_entities() -> Vec<&'static str> {
    let mut entities = scope::LOOKUP_ENTITIES.to_vec();
    entities.push(scope::ENTITY_DEADLINE);
    entities.push(scope::ENTITY_DEADLINE_SUPPORT);
    entities
}
