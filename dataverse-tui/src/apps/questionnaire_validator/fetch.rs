use std::collections::{HashMap, HashSet};

use dataverse_lib::api::query::Filter;
use dataverse_lib::error::Error as DataverseError;
use dataverse_lib::model::metadata::EntityMetadata;
use dataverse_lib::model::{Entity, Record, Value};
use rafter::prelude::*;
use rafter::widgets::TreeNode;
use uuid::Uuid;

use crate::apps::questionnaire_sync::scope::{
    QUESTIONNAIRE_ENTITIES, QUESTIONNAIRE_RELATIONS, QuestionnaireEntitySpec,
    QuestionnaireFieldKind,
};
use crate::modals::LoadingModal;
use crate::modals::odata_fetch::{ODataFetchError, ODataFetchModal, ODataFetchTask};
use crate::systems::client_management::ActiveClientInfo;

use super::QuestionnaireValidator;
use super::tree::build_validation_tree;
use super::types::{EntityRecordSet, QuestionnaireGraph, QuestionnaireSummary, ValidatorView};
use super::util::{entity_spec, guid_value, record_name};
use super::validation::build_validation_report;

const FILTER_CHUNK_SIZE: usize = 20;
const MAX_FETCH_PASSES: usize = 10;

impl QuestionnaireValidator {
    pub(super) async fn fetch_questionnaires(&self, gx: &GlobalContext) {
        let Some(client_info) = self.client_info.get() else {
            gx.toast(Toast::error("No active client"));
            return;
        };

        let client = client_info.client.clone();
        let query = client
            .query(Entity::logical("nrq_questionnaire"))
            .select(&[
                "nrq_questionnaireid",
                "nrq_name",
                "nrq_code",
                "nrq_type",
                "statecode",
                "statuscode",
            ])
            .page_size(1000);
        let tasks = vec![ODataFetchTask::new("Questionnaires", client, query)];

        let results = match gx.modal(ODataFetchModal::create(tasks)).await {
            Ok(results) => results,
            Err(ODataFetchError::TaskFailed { label, error }) => {
                let message = format!("Fetch failed for {}: {}", label, error);
                self.fetch_error.set(Some(message.clone()));
                gx.toast(Toast::error(message));
                return;
            }
            Err(ODataFetchError::Cancelled) => {
                self.fetch_error.set(Some("Fetch cancelled".to_string()));
                return;
            }
        };

        let mut questionnaires: Vec<QuestionnaireSummary> = results
            .into_iter()
            .next()
            .unwrap_or_default()
            .iter()
            .map(QuestionnaireSummary::from_record)
            .collect();
        questionnaires.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        let roots = questionnaires
            .iter()
            .cloned()
            .map(TreeNode::leaf)
            .collect::<Vec<_>>();

        self.questionnaires.set(questionnaires);
        self.questionnaire_tree.update(|tree| {
            tree.set_roots(roots);
        });
        self.fetch_error.set(None);
        gx.toast(Toast::success("Questionnaires fetched"));
    }

    pub(super) async fn load_questionnaire_detail(
        &self,
        gx: &GlobalContext,
        questionnaire: QuestionnaireSummary,
    ) {
        let Some(client_info) = self.client_info.get() else {
            gx.toast(Toast::error("No active client"));
            return;
        };
        let Some(questionnaire_id) = questionnaire.id_uuid() else {
            gx.toast(Toast::error("Selected questionnaire has no valid id"));
            return;
        };

        self.view.set(ValidatorView::Detail);
        self.selected_questionnaire.set(Some(questionnaire.clone()));
        self.validation_report.set(None);
        self.validation_tree
            .update(|tree| tree.set_roots(Vec::new()));

        let metadata = match fetch_metadata(gx, &client_info).await {
            Some(metadata) => metadata,
            None => return,
        };
        let graph = match self
            .fetch_questionnaire_graph(gx, &client_info, questionnaire_id, &metadata)
            .await
        {
            Some(graph) => graph,
            None => return,
        };

        let report = build_validation_report(questionnaire, graph, metadata);
        let roots = build_validation_tree(&report);
        self.validation_tree.update(|tree| {
            tree.set_roots(roots);
            tree.expanded.clear();
            tree.expanded.insert("root".to_string());
        });
        self.validation_report.set(Some(report.clone()));
        self.fetch_error.set(None);
        gx.toast(Toast::success(format!(
            "Validation complete: {} findings",
            report.finding_count
        )));
    }

    async fn fetch_questionnaire_graph(
        &self,
        gx: &GlobalContext,
        client_info: &ActiveClientInfo,
        questionnaire_id: Uuid,
        metadata: &HashMap<String, EntityMetadata>,
    ) -> Option<QuestionnaireGraph> {
        let client = client_info.client.clone();
        let mut records_by_entity: HashMap<String, Vec<Record>> = HashMap::new();
        let mut known_ids: HashMap<String, HashSet<Uuid>> = HashMap::new();
        let mut fetched_ids: HashMap<String, HashSet<Uuid>> = HashMap::new();

        add_known_id(&mut known_ids, "nrq_questionnaire", questionnaire_id);

        let mut initial_tasks = vec![ODataFetchTask::new(
            "nrq_questionnaire",
            client.clone(),
            client
                .query(Entity::logical("nrq_questionnaire"))
                .filter(Filter::eq("nrq_questionnaireid", questionnaire_id))
                .page_size(1000),
        )];

        for relation in QUESTIONNAIRE_RELATIONS {
            let Some(related) = entity_spec(relation.related_entity) else {
                continue;
            };
            initial_tasks.push(ODataFetchTask::new(
                relation.relationship_name,
                client.clone(),
                client
                    .query(Entity::logical("nrq_questionnaire"))
                    .select(&["nrq_questionnaireid"])
                    .filter(Filter::eq("nrq_questionnaireid", questionnaire_id))
                    .expand(relation.relationship_name, |expand| {
                        expand.select(&[related.primary_key])
                    })
                    .page_size(1000),
            ));
        }

        let initial_results = match gx.modal(ODataFetchModal::create(initial_tasks)).await {
            Ok(results) => results,
            Err(ODataFetchError::TaskFailed { label, error }) => {
                let message = format!("Fetch failed for {}: {}", label, error);
                self.fetch_error.set(Some(message.clone()));
                gx.toast(Toast::error(message));
                return None;
            }
            Err(ODataFetchError::Cancelled) => {
                self.fetch_error.set(Some("Fetch cancelled".to_string()));
                return None;
            }
        };

        for (index, records) in initial_results.into_iter().enumerate() {
            if index == 0 {
                ingest_records(
                    &mut records_by_entity,
                    &mut known_ids,
                    &mut fetched_ids,
                    "nrq_questionnaire",
                    records,
                );
            } else if let Some(relation) = QUESTIONNAIRE_RELATIONS.get(index - 1) {
                ingest_relation_records(
                    &mut known_ids,
                    relation.related_entity,
                    records,
                    relation.relationship_name,
                );
            }
        }

        for pass in 0..MAX_FETCH_PASSES {
            let mut task_specs = Vec::new();
            for spec in QUESTIONNAIRE_ENTITIES {
                let primary_ids = missing_known_ids(&known_ids, &fetched_ids, spec.logical_name);
                for chunk in primary_ids.chunks(FILTER_CHUNK_SIZE) {
                    task_specs.push(GraphFetchTaskSpec::PrimaryKey {
                        entity: spec.logical_name.to_string(),
                        ids: chunk.to_vec(),
                    });
                }

                for field in spec.fields {
                    let QuestionnaireFieldKind::Lookup { target_entity } = field.kind else {
                        continue;
                    };
                    if !metadata_has_field(metadata, spec.logical_name, field.source_name) {
                        log::warn!(
                            "Skipping questionnaire validator lookup fetch: {}.{} is not present in metadata",
                            spec.logical_name,
                            field.source_name
                        );
                        continue;
                    }
                    let Some(ids) = known_ids.get(target_entity) else {
                        continue;
                    };
                    if ids.is_empty() {
                        continue;
                    }
                    let ids = ids.iter().copied().collect::<Vec<_>>();
                    for chunk in ids.chunks(FILTER_CHUNK_SIZE) {
                        task_specs.push(GraphFetchTaskSpec::Lookup {
                            entity: spec.logical_name.to_string(),
                            field: field.source_name.to_string(),
                            ids: chunk.to_vec(),
                        });
                    }
                }
            }

            if task_specs.is_empty() {
                break;
            }

            let tasks = task_specs
                .iter()
                .map(|spec| {
                    let entity = spec.entity().to_string();
                    let filter = spec.filter();
                    ODataFetchTask::new(
                        spec.label(pass + 1),
                        client.clone(),
                        client
                            .query(Entity::logical(entity.as_str()))
                            .filter(filter)
                            .page_size(1000),
                    )
                })
                .collect::<Vec<_>>();

            let results = match gx.modal(ODataFetchModal::create(tasks)).await {
                Ok(results) => results,
                Err(ODataFetchError::TaskFailed { label, error }) => {
                    let message = format!("Fetch failed for {}: {}", label, error);
                    self.fetch_error.set(Some(message.clone()));
                    gx.toast(Toast::error(message));
                    return None;
                }
                Err(ODataFetchError::Cancelled) => {
                    self.fetch_error.set(Some("Fetch cancelled".to_string()));
                    return None;
                }
            };

            let before = total_known_ids(&known_ids);
            for (task, records) in task_specs.into_iter().zip(results.into_iter()) {
                ingest_records(
                    &mut records_by_entity,
                    &mut known_ids,
                    &mut fetched_ids,
                    task.entity(),
                    records,
                );
            }
            let after = total_known_ids(&known_ids);
            if after == before {
                break;
            }
        }

        let mut sets = QUESTIONNAIRE_ENTITIES
            .iter()
            .filter_map(|spec| {
                let mut records = records_by_entity
                    .remove(spec.logical_name)
                    .unwrap_or_default();
                if records.is_empty() {
                    return None;
                }
                records.sort_by_key(record_name);
                Some(EntityRecordSet {
                    entity: spec.logical_name.to_string(),
                    records,
                })
            })
            .collect::<Vec<_>>();
        sets.sort_by(|a, b| a.entity.cmp(&b.entity));

        Some(QuestionnaireGraph {
            records_by_entity: sets,
        })
    }
}

#[derive(Clone, Debug)]
enum GraphFetchTaskSpec {
    PrimaryKey {
        entity: String,
        ids: Vec<Uuid>,
    },
    Lookup {
        entity: String,
        field: String,
        ids: Vec<Uuid>,
    },
}

impl GraphFetchTaskSpec {
    fn entity(&self) -> &str {
        match self {
            Self::PrimaryKey { entity, .. } | Self::Lookup { entity, .. } => entity,
        }
    }

    fn label(&self, pass: usize) -> String {
        match self {
            Self::PrimaryKey { entity, ids } => {
                format!("Pass {} — {} by id ({})", pass, entity, ids.len())
            }
            Self::Lookup { entity, field, ids } => {
                format!("Pass {} — {} by {} ({})", pass, entity, field, ids.len())
            }
        }
    }

    fn filter(&self) -> Filter {
        match self {
            Self::PrimaryKey { entity, ids } => {
                let primary_key = entity_spec(entity)
                    .map(|spec| spec.primary_key)
                    .unwrap_or(entity.as_str());
                id_filter(primary_key, ids)
            }
            Self::Lookup { field, ids, .. } => id_filter(field, ids),
        }
    }
}

async fn fetch_metadata(
    gx: &GlobalContext,
    client_info: &ActiveClientInfo,
) -> Option<HashMap<String, EntityMetadata>> {
    let client = client_info.client.clone();
    let entity_names = QUESTIONNAIRE_ENTITIES
        .iter()
        .map(|spec| spec.logical_name.to_string())
        .collect::<Vec<_>>();

    match gx
        .modal(LoadingModal::run_with_default(
            "Fetching questionnaire metadata",
            || Err(DataverseError::Cancelled),
            async move {
                let mut metadata = HashMap::new();
                for entity in entity_names {
                    let entity_metadata = client
                        .metadata()
                        .entity(Entity::logical(entity.as_str()))
                        .await?;
                    metadata.insert(entity, entity_metadata);
                }
                Ok::<_, DataverseError>(metadata)
            },
        ))
        .await
    {
        Ok(metadata) => Some(metadata),
        Err(e) if e.is_cancelled() => None,
        Err(e) => {
            gx.toast(Toast::error(format!("Failed to fetch metadata: {}", e)));
            None
        }
    }
}

fn metadata_has_field(
    metadata: &HashMap<String, EntityMetadata>,
    entity: &str,
    field: &str,
) -> bool {
    metadata
        .get(entity)
        .map(|entity_metadata| {
            entity_metadata
                .attributes
                .iter()
                .any(|attribute| attribute.logical_name == field)
                || entity_metadata
                    .picklist_attributes
                    .iter()
                    .any(|attribute| attribute.logical_name == field)
                || entity_metadata
                    .multi_select_picklist_attributes
                    .iter()
                    .any(|attribute| attribute.logical_name == field)
                || entity_metadata
                    .state_attributes
                    .iter()
                    .any(|attribute| attribute.logical_name == field)
                || entity_metadata
                    .status_attributes
                    .iter()
                    .any(|attribute| attribute.logical_name == field)
        })
        .unwrap_or(false)
}

fn ingest_records(
    records_by_entity: &mut HashMap<String, Vec<Record>>,
    known_ids: &mut HashMap<String, HashSet<Uuid>>,
    fetched_ids: &mut HashMap<String, HashSet<Uuid>>,
    entity: &str,
    records: Vec<Record>,
) {
    let Some(spec) = entity_spec(entity) else {
        return;
    };
    for record in records {
        let Some(id) = guid_value(&record, spec.primary_key) else {
            continue;
        };
        let was_new = fetched_ids
            .entry(entity.to_string())
            .or_default()
            .insert(id);
        add_known_id(known_ids, entity, id);
        ingest_lookup_ids(known_ids, spec, &record);
        if was_new {
            records_by_entity
                .entry(entity.to_string())
                .or_default()
                .push(record);
        }
    }
}

fn ingest_lookup_ids(
    known_ids: &mut HashMap<String, HashSet<Uuid>>,
    spec: &QuestionnaireEntitySpec,
    record: &Record,
) {
    for field in spec.fields {
        let QuestionnaireFieldKind::Lookup { target_entity } = field.kind else {
            continue;
        };
        if let Some(id) = guid_value(record, field.source_name) {
            add_known_id(known_ids, target_entity, id);
        }
    }
}

fn ingest_relation_records(
    known_ids: &mut HashMap<String, HashSet<Uuid>>,
    related_entity: &str,
    records: Vec<Record>,
    relationship_name: &str,
) {
    let Some(related_spec) = entity_spec(related_entity) else {
        return;
    };
    for record in records {
        let Some(Value::Records(related_records)) = record.get(relationship_name) else {
            continue;
        };
        for related_record in related_records {
            if let Some(id) = guid_value(related_record, related_spec.primary_key) {
                add_known_id(known_ids, related_entity, id);
            }
        }
    }
}

fn missing_known_ids(
    known_ids: &HashMap<String, HashSet<Uuid>>,
    fetched_ids: &HashMap<String, HashSet<Uuid>>,
    entity: &str,
) -> Vec<Uuid> {
    let Some(known) = known_ids.get(entity) else {
        return Vec::new();
    };
    let fetched = fetched_ids.get(entity);
    known
        .iter()
        .filter(|id| fetched.is_none_or(|fetched| !fetched.contains(id)))
        .copied()
        .collect()
}

fn add_known_id(known_ids: &mut HashMap<String, HashSet<Uuid>>, entity: &str, id: Uuid) {
    known_ids.entry(entity.to_string()).or_default().insert(id);
}

fn total_known_ids(known_ids: &HashMap<String, HashSet<Uuid>>) -> usize {
    known_ids.values().map(HashSet::len).sum()
}

fn id_filter(field: &str, ids: &[Uuid]) -> Filter {
    if ids.len() == 1 {
        Filter::eq(field, ids[0])
    } else {
        Filter::or(ids.iter().map(|id| Filter::eq(field, *id)))
    }
}
