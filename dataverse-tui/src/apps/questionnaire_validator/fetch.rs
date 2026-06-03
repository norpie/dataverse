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
use crate::modals::odata_fetch::{ODataFetchError, ODataFetchModal, ODataFetchTask};
use crate::modals::{LoadingModal, LoadingUpdater};
use crate::systems::client_management::ActiveClientInfo;

use super::QuestionnaireValidator;
use super::tree::{build_bulk_validation_tree, build_validation_tree};
use super::types::{EntityRecordSet, QuestionnaireGraph, QuestionnaireSummary, ValidatorView};
use super::util::{entity_spec, guid_value, lookup_guid_value, record_name};
use super::validation::{build_bulk_validation_result, build_validation_report};

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

    pub(super) async fn run_bulk_validation(&self, gx: &GlobalContext) {
        let Some(client_info) = self.client_info.get() else {
            gx.toast(Toast::error("No active client"));
            return;
        };

        self.view.set(ValidatorView::Bulk);
        self.bulk_result.set(None);
        self.bulk_tree.update(|tree| tree.set_roots(Vec::new()));

        let metadata = match fetch_metadata(gx, &client_info).await {
            Some(metadata) => metadata,
            None => return,
        };
        let scope = match fetch_full_scope(gx, &client_info).await {
            Some(scope) => scope,
            None => return,
        };

        let report_result = gx
            .modal(LoadingModal::run_with_default_updates(
                "Preparing bulk validation...",
                || Err(DataverseError::Cancelled),
                |updater| async move {
                    let reports = build_bulk_reports(scope, metadata, updater).await;
                    Ok::<_, DataverseError>(reports)
                },
            ))
            .await;

        let reports = match report_result {
            Ok(reports) => reports,
            Err(e) if e.is_cancelled() => return,
            Err(e) => {
                gx.toast(Toast::error(format!("Bulk validation failed: {}", e)));
                return;
            }
        };
        let result = build_bulk_validation_result(reports);
        let roots = build_bulk_validation_tree(&result);
        self.bulk_tree.update(|tree| {
            tree.set_roots(roots);
            tree.expanded.clear();
            tree.expanded.insert("bulk-root".to_string());
        });
        self.bulk_result.set(Some(result.clone()));
        self.fetch_error.set(None);
        gx.toast(Toast::success(format!(
            "Bulk validation complete: {} findings in {} questionnaires",
            result.finding_count, result.failed_questionnaire_count
        )));
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
        if let Some(id) = lookup_guid_value(record, field.source_name) {
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

#[derive(Clone, Debug)]
struct FullScopeFetch {
    records_by_entity: HashMap<String, Vec<Record>>,
    relations: Vec<RelationMembershipSet>,
}

#[derive(Clone, Debug)]
struct RelationMembershipSet {
    parent_entity: String,
    related_entity: String,
    memberships: Vec<RelationMembership>,
}

#[derive(Clone, Debug)]
struct RelationMembership {
    parent_id: Uuid,
    related_id: Uuid,
}

#[derive(Clone, Debug)]
struct BulkFetchSpec {
    kind: BulkFetchSpecKind,
}

#[derive(Clone, Debug)]
enum BulkFetchSpecKind {
    Entity(&'static QuestionnaireEntitySpec),
    Relation(&'static crate::apps::questionnaire_sync::scope::QuestionnaireRelationSpec),
}

impl BulkFetchSpec {
    fn entity(entity: &'static QuestionnaireEntitySpec) -> Self {
        Self {
            kind: BulkFetchSpecKind::Entity(entity),
        }
    }

    fn relation(
        relation: &'static crate::apps::questionnaire_sync::scope::QuestionnaireRelationSpec,
    ) -> Self {
        Self {
            kind: BulkFetchSpecKind::Relation(relation),
        }
    }

    fn label(&self) -> String {
        match self.kind {
            BulkFetchSpecKind::Entity(entity) => entity.logical_name.to_string(),
            BulkFetchSpecKind::Relation(relation) => relation.relationship_name.to_string(),
        }
    }
}

async fn fetch_full_scope(
    gx: &GlobalContext,
    client_info: &ActiveClientInfo,
) -> Option<FullScopeFetch> {
    let client = client_info.client.clone();
    let mut specs = Vec::new();
    for entity in QUESTIONNAIRE_ENTITIES {
        specs.push(BulkFetchSpec::entity(entity));
    }
    for relation in QUESTIONNAIRE_RELATIONS {
        specs.push(BulkFetchSpec::relation(relation));
    }

    let tasks = specs
        .iter()
        .map(|spec| {
            let query = match spec.kind {
                BulkFetchSpecKind::Entity(entity) => client
                    .query(Entity::logical(entity.logical_name))
                    .page_size(1000),
                BulkFetchSpecKind::Relation(relation) => {
                    let parent = entity_spec(relation.parent_entity).unwrap_or_else(|| {
                        panic!("missing entity spec: {}", relation.parent_entity)
                    });
                    let related = entity_spec(relation.related_entity).unwrap_or_else(|| {
                        panic!("missing entity spec: {}", relation.related_entity)
                    });
                    client
                        .query(Entity::logical(relation.parent_entity))
                        .select(&[parent.primary_key])
                        .expand(relation.relationship_name, |expand| {
                            expand.select(&[related.primary_key])
                        })
                        .page_size(1000)
                }
            };
            ODataFetchTask::new(spec.label(), client.clone(), query)
        })
        .collect::<Vec<_>>();

    let results = match gx.modal(ODataFetchModal::create(tasks)).await {
        Ok(results) => results,
        Err(ODataFetchError::TaskFailed { label, error }) => {
            gx.toast(Toast::error(format!(
                "Fetch failed for {}: {}",
                label, error
            )));
            return None;
        }
        Err(ODataFetchError::Cancelled) => return None,
    };

    let mut records_by_entity = HashMap::new();
    let mut relations = Vec::new();
    for (spec, records) in specs.into_iter().zip(results.into_iter()) {
        match spec.kind {
            BulkFetchSpecKind::Entity(entity) => {
                log::debug!(
                    "Bulk questionnaire fetch {} records: {}",
                    entity.logical_name,
                    records.len()
                );
                records_by_entity.insert(entity.logical_name.to_string(), records);
            }
            BulkFetchSpecKind::Relation(relation) => {
                log::debug!(
                    "Bulk questionnaire relation fetch {} parent records: {}",
                    relation.relationship_name,
                    records.len()
                );
                relations.push(relation_membership_set(relation, records));
            }
        }
    }

    Some(FullScopeFetch {
        records_by_entity,
        relations,
    })
}

fn relation_membership_set(
    relation: &crate::apps::questionnaire_sync::scope::QuestionnaireRelationSpec,
    records: Vec<Record>,
) -> RelationMembershipSet {
    let Some(parent) = entity_spec(relation.parent_entity) else {
        return RelationMembershipSet {
            parent_entity: relation.parent_entity.to_string(),
            related_entity: relation.related_entity.to_string(),
            memberships: Vec::new(),
        };
    };
    let Some(related) = entity_spec(relation.related_entity) else {
        return RelationMembershipSet {
            parent_entity: relation.parent_entity.to_string(),
            related_entity: relation.related_entity.to_string(),
            memberships: Vec::new(),
        };
    };
    let mut memberships = Vec::new();
    for record in records {
        let Some(parent_id) = guid_value(&record, parent.primary_key) else {
            continue;
        };
        let Some(Value::Records(related_records)) = record.get(relation.relationship_name) else {
            continue;
        };
        for related_record in related_records {
            if let Some(related_id) = guid_value(related_record, related.primary_key) {
                memberships.push(RelationMembership {
                    parent_id,
                    related_id,
                });
            }
        }
    }
    RelationMembershipSet {
        parent_entity: relation.parent_entity.to_string(),
        related_entity: relation.related_entity.to_string(),
        memberships,
    }
}

async fn build_bulk_reports(
    scope: FullScopeFetch,
    metadata: HashMap<String, EntityMetadata>,
    updater: LoadingUpdater,
) -> Vec<super::types::ValidationReport> {
    let questionnaires = scope
        .records_by_entity
        .get("nrq_questionnaire")
        .cloned()
        .unwrap_or_default();
    let questionnaire_count = questionnaires.len();
    let mut reports = Vec::new();

    updater.update(format!(
        "Bulk validation: preparing {} questionnaires",
        questionnaire_count
    ));
    tokio::task::yield_now().await;

    for (index, questionnaire_record) in questionnaires.into_iter().enumerate() {
        let questionnaire = QuestionnaireSummary::from_record(&questionnaire_record);
        updater.update(format!(
            "Bulk validation {}/{} — {}",
            index + 1,
            questionnaire_count,
            questionnaire.name
        ));

        let Some(questionnaire_id) = questionnaire.id_uuid() else {
            continue;
        };
        let graph = graph_for_questionnaire(&scope, questionnaire_id);
        reports.push(build_validation_report(
            questionnaire,
            graph,
            metadata.clone(),
        ));

        if index % 5 == 0 {
            tokio::task::yield_now().await;
        }
    }

    updater.update("Bulk validation: building failure summary");
    tokio::task::yield_now().await;

    reports
}

fn graph_for_questionnaire(scope: &FullScopeFetch, questionnaire_id: Uuid) -> QuestionnaireGraph {
    let mut structural_ids: HashMap<String, HashSet<Uuid>> = HashMap::new();
    add_known_id(&mut structural_ids, "nrq_questionnaire", questionnaire_id);

    loop {
        let before = total_known_ids(&structural_ids);

        add_records_by_lookup(
            scope,
            &mut structural_ids,
            "nrq_questionnairepage",
            "nrq_relatedquestionnaire",
            "nrq_questionnaire",
        );
        add_records_by_lookup(
            scope,
            &mut structural_ids,
            "nrq_questionnairepageline",
            "nrq_questionnaireid",
            "nrq_questionnaire",
        );
        add_records_by_lookup(
            scope,
            &mut structural_ids,
            "nrq_questionnairepageline",
            "nrq_questionnairepageid",
            "nrq_questionnairepage",
        );
        add_records_by_lookup(
            scope,
            &mut structural_ids,
            "nrq_questiongroupline",
            "nrq_questionnairepageid",
            "nrq_questionnairepage",
        );
        add_referenced_records(
            scope,
            &mut structural_ids,
            "nrq_questiongroupline",
            "nrq_questiongroupid",
            "nrq_questiongroup",
        );
        add_records_by_lookup(
            scope,
            &mut structural_ids,
            "nrq_question",
            "nrq_questionnaireid",
            "nrq_questionnaire",
        );
        add_records_by_lookup(
            scope,
            &mut structural_ids,
            "nrq_question",
            "nrq_questionpage",
            "nrq_questionnairepage",
        );
        add_records_by_lookup(
            scope,
            &mut structural_ids,
            "nrq_question",
            "nrq_questiongroupid",
            "nrq_questiongroup",
        );
        add_records_by_lookup(
            scope,
            &mut structural_ids,
            "nrq_questioncondition",
            "nrq_questionnaireid",
            "nrq_questionnaire",
        );
        add_records_by_lookup(
            scope,
            &mut structural_ids,
            "nrq_questioncondition",
            "nrq_questionid",
            "nrq_question",
        );
        add_records_by_lookup(
            scope,
            &mut structural_ids,
            "nrq_questionconditionaction",
            "nrq_questionconditionid",
            "nrq_questioncondition",
        );
        add_records_by_lookup(
            scope,
            &mut structural_ids,
            "nrq_questionconditionaction",
            "nrq_questionid",
            "nrq_question",
        );

        let after = total_known_ids(&structural_ids);
        if after == before {
            break;
        }
    }

    let mut graph_ids = structural_ids.clone();
    add_direct_relation_context(scope, &mut graph_ids, questionnaire_id);
    add_lookup_context(scope, &mut graph_ids, &structural_ids);

    log::debug!(
        "Bulk questionnaire graph {} counts: {}",
        questionnaire_id,
        graph_ids
            .iter()
            .map(|(entity, ids)| format!("{}={}", entity, ids.len()))
            .collect::<Vec<_>>()
            .join(", ")
    );

    let mut sets = QUESTIONNAIRE_ENTITIES
        .iter()
        .filter_map(|spec| {
            let ids = graph_ids.get(spec.logical_name)?;
            let mut records = scope
                .records_by_entity
                .get(spec.logical_name)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(|record| {
                    guid_value(record, spec.primary_key)
                        .map(|id| ids.contains(&id))
                        .unwrap_or(false)
                })
                .collect::<Vec<_>>();
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

    QuestionnaireGraph {
        records_by_entity: sets,
    }
}

fn add_records_by_lookup(
    scope: &FullScopeFetch,
    known_ids: &mut HashMap<String, HashSet<Uuid>>,
    entity: &str,
    lookup_field: &str,
    target_entity: &str,
) {
    let Some(target_ids) = known_ids.get(target_entity).cloned() else {
        return;
    };
    if target_ids.is_empty() {
        return;
    }
    let Some(spec) = entity_spec(entity) else {
        return;
    };
    let Some(records) = scope.records_by_entity.get(entity) else {
        return;
    };
    for record in records {
        let Some(lookup_id) = lookup_guid_value(record, lookup_field) else {
            continue;
        };
        if !target_ids.contains(&lookup_id) {
            continue;
        }
        if let Some(record_id) = guid_value(record, spec.primary_key) {
            add_known_id(known_ids, entity, record_id);
        }
    }
}

fn add_referenced_records(
    scope: &FullScopeFetch,
    known_ids: &mut HashMap<String, HashSet<Uuid>>,
    source_entity: &str,
    lookup_field: &str,
    target_entity: &str,
) {
    let Some(source_ids) = known_ids.get(source_entity).cloned() else {
        return;
    };
    if source_ids.is_empty() {
        return;
    }
    let Some(source_spec) = entity_spec(source_entity) else {
        return;
    };
    let Some(source_records) = scope.records_by_entity.get(source_entity) else {
        return;
    };
    for record in source_records {
        let Some(source_id) = guid_value(record, source_spec.primary_key) else {
            continue;
        };
        if !source_ids.contains(&source_id) {
            continue;
        }
        if let Some(target_id) = lookup_guid_value(record, lookup_field) {
            add_known_id(known_ids, target_entity, target_id);
        }
    }
}

fn add_direct_relation_context(
    scope: &FullScopeFetch,
    graph_ids: &mut HashMap<String, HashSet<Uuid>>,
    questionnaire_id: Uuid,
) {
    for relation in &scope.relations {
        if relation.parent_entity != "nrq_questionnaire" {
            continue;
        }
        for membership in &relation.memberships {
            if membership.parent_id == questionnaire_id {
                add_known_id(graph_ids, &relation.related_entity, membership.related_id);
            }
        }
    }
}

fn add_lookup_context(
    scope: &FullScopeFetch,
    graph_ids: &mut HashMap<String, HashSet<Uuid>>,
    structural_ids: &HashMap<String, HashSet<Uuid>>,
) {
    let mut context_ids: HashMap<String, HashSet<Uuid>> = HashMap::new();
    for spec in QUESTIONNAIRE_ENTITIES {
        let Some(source_ids) = structural_ids.get(spec.logical_name) else {
            continue;
        };
        let Some(records) = scope.records_by_entity.get(spec.logical_name) else {
            continue;
        };
        for record in records {
            let Some(record_id) = guid_value(record, spec.primary_key) else {
                continue;
            };
            if !source_ids.contains(&record_id) {
                continue;
            }
            for field in spec.fields {
                let QuestionnaireFieldKind::Lookup { target_entity } = field.kind else {
                    continue;
                };
                if let Some(target_id) = lookup_guid_value(record, field.source_name) {
                    add_known_id(&mut context_ids, target_entity, target_id);
                }
            }
        }
    }

    for (entity, ids) in context_ids {
        for id in ids {
            add_known_id(graph_ids, &entity, id);
        }
    }
}
