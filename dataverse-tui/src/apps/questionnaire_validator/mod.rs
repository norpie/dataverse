//! Questionnaire Validator app.

use std::collections::{HashMap, HashSet};

use dataverse_lib::api::query::Filter;
use dataverse_lib::error::Error as DataverseError;
use dataverse_lib::model::Entity;
use dataverse_lib::model::Record;
use dataverse_lib::model::Value;
use dataverse_lib::model::metadata::EntityMetadata;
use rafter::element;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::TreeNode;
use tuidom::Color;
use tuidom::Element;
use uuid::Uuid;

use crate::apps::questionnaire_sync::scope::QUESTIONNAIRE_ENTITIES;
use crate::apps::questionnaire_sync::scope::QUESTIONNAIRE_RELATIONS;
use crate::apps::questionnaire_sync::scope::QuestionnaireEntitySpec;
use crate::apps::questionnaire_sync::scope::QuestionnaireFieldKind;
use crate::modals::LoadingModal;
use crate::modals::odata_fetch::ODataFetchError;
use crate::modals::odata_fetch::ODataFetchModal;
use crate::modals::odata_fetch::ODataFetchTask;
use crate::systems::client_management::ActiveClientInfo;
use crate::systems::client_management::ClientManagement;
use crate::systems::client_management::GetActiveClient;

const FILTER_CHUNK_SIZE: usize = 20;
const MAX_FETCH_PASSES: usize = 10;

#[derive(Clone, Debug, PartialEq, Eq)]
enum ValidatorView {
    List,
    Detail,
}

impl Default for ValidatorView {
    fn default() -> Self {
        Self::List
    }
}

#[derive(Clone, Debug)]
struct QuestionnaireSummary {
    id: String,
    name: String,
    code: Option<String>,
    questionnaire_type: Option<i32>,
    statecode: Option<i32>,
    statuscode: Option<i32>,
}

impl QuestionnaireSummary {
    fn from_record(record: &Record) -> Self {
        let id = guid_value(record, "nrq_questionnaireid")
            .map(|id| id.to_string())
            .unwrap_or_else(|| "unknown-id".to_string());
        let name = string_value(record, "nrq_name").unwrap_or_else(|| "unknown name".to_string());

        Self {
            id,
            name,
            code: string_value(record, "nrq_code"),
            questionnaire_type: option_value(record, "nrq_type"),
            statecode: option_value(record, "statecode"),
            statuscode: option_value(record, "statuscode"),
        }
    }

    fn id_uuid(&self) -> Option<Uuid> {
        Uuid::parse_str(&self.id).ok()
    }

    fn is_active(&self) -> bool {
        self.statecode == Some(0)
    }

    fn state_label(&self) -> &'static str {
        if self.is_active() {
            "active"
        } else {
            "inactive"
        }
    }

    fn short_id(&self) -> String {
        short_id(&self.id)
    }
}

impl TreeItem for QuestionnaireSummary {
    type Key = String;

    fn key(&self) -> String {
        self.id.clone()
    }

    fn render(&self) -> Element {
        let state_label = self.state_label();
        let state_color = if self.is_active() { "success" } else { "muted" };
        let code = self.code.clone().unwrap_or_else(|| "no code".to_string());
        let questionnaire_type = self
            .questionnaire_type
            .map(|value| value.to_string())
            .unwrap_or_else(|| "type ?".to_string());
        let statuscode = self
            .statuscode
            .map(|value| value.to_string())
            .unwrap_or_else(|| "status ?".to_string());
        let id = self.short_id();

        element! {
            row (gap: 1) {
                text (content: {self.name.clone()}) style (fg: primary)
                text (content: {format!("[{}]", state_label)}) style (fg: {Color::var(state_color)})
                text (content: {code}) style (fg: muted)
                text (content: {format!("type {}", questionnaire_type)}) style (fg: muted)
                text (content: {format!("status {}", statuscode)}) style (fg: muted)
                text (content: {id}) style (fg: muted)
            }
        }
    }
}

#[derive(Clone, Debug)]
struct EntityRecordSet {
    entity: String,
    records: Vec<Record>,
}

#[derive(Clone, Debug)]
struct QuestionnaireGraph {
    records_by_entity: Vec<EntityRecordSet>,
}

#[derive(Clone, Debug)]
struct ValidatedField {
    field: String,
    value: Option<i32>,
    accepted_values: Vec<i32>,
    valid: bool,
}

#[derive(Clone, Debug)]
struct RecordValidation {
    entity: String,
    record_id: String,
    name: String,
    fields: Vec<ValidatedField>,
}

impl RecordValidation {
    fn finding_count(&self) -> usize {
        self.fields.iter().filter(|field| !field.valid).count()
    }
}

#[derive(Clone, Debug)]
struct EntityValidation {
    entity: String,
    record_count: usize,
    findings_count: usize,
    records: Vec<RecordValidation>,
}

#[derive(Clone, Debug)]
struct ValidationReport {
    questionnaire: QuestionnaireSummary,
    record_count: usize,
    finding_count: usize,
    entities: Vec<EntityValidation>,
}

#[derive(Clone, Debug)]
enum ValidationTreeNode {
    Root {
        name: String,
        finding_count: usize,
        record_count: usize,
    },
    Entity {
        entity: String,
        finding_count: usize,
        record_count: usize,
    },
    Record {
        entity: String,
        id: String,
        name: String,
        finding_count: usize,
    },
    Field {
        entity: String,
        id: String,
        field: String,
        value: Option<i32>,
        accepted_values: Vec<i32>,
        valid: bool,
    },
}

impl TreeItem for ValidationTreeNode {
    type Key = String;

    fn key(&self) -> String {
        match self {
            Self::Root { .. } => "root".to_string(),
            Self::Entity { entity, .. } => format!("entity-{}", entity),
            Self::Record { entity, id, .. } => format!("record-{}-{}", entity, id),
            Self::Field {
                entity,
                id,
                field,
                value,
                ..
            } => format!(
                "field-{}-{}-{}-{}",
                entity,
                id,
                field,
                value
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".to_string())
            ),
        }
    }

    fn render(&self) -> Element {
        match self {
            Self::Root {
                name,
                finding_count,
                record_count,
            } => {
                let status = validation_status(*finding_count);
                let color = validation_color(*finding_count);
                element! {
                    row (gap: 1) {
                        text (content: {name.clone()}) style (bold, fg: interact)
                        text (content: {status}) style (fg: {Color::var(color)})
                        text (content: {format!("{} records", record_count)}) style (fg: muted)
                    }
                }
            }
            Self::Entity {
                entity,
                finding_count,
                record_count,
            } => {
                let status = validation_status(*finding_count);
                let color = validation_color(*finding_count);
                element! {
                    row (gap: 1) {
                        text (content: {entity.clone()}) style (fg: primary)
                        text (content: {status}) style (fg: {Color::var(color)})
                        text (content: {format!("{} records", record_count)}) style (fg: muted)
                    }
                }
            }
            Self::Record {
                id,
                name,
                finding_count,
                ..
            } => {
                let status = validation_status(*finding_count);
                let color = validation_color(*finding_count);
                element! {
                    row (gap: 1) {
                        text (content: {name.clone()}) style (fg: primary)
                        text (content: {status}) style (fg: {Color::var(color)})
                        text (content: {short_id(id)}) style (fg: muted)
                    }
                }
            }
            Self::Field {
                field,
                value,
                accepted_values,
                valid,
                ..
            } => {
                let accepted = format_accepted_values(accepted_values);
                let has_value = value.is_some();
                let status = if *valid { "ok" } else { "error" };
                let color = if *valid { "success" } else { "error" };
                let value_text = value
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "not set".to_string());
                element! {
                    row (gap: 1) {
                        text (content: {field.clone()}) style (fg: primary)
                        text (content: {value_text}) style (fg: muted)
                        if has_value {
                            text (content: {status}) style (fg: {Color::var(color)})
                        }
                        if !*valid {
                            text (content: {accepted}) style (fg: muted)
                        }
                    }
                }
            }
        }
    }
}

/// Questionnaire validator app.
#[app(name = "VAF - Questionnaire Validator", singleton, on_blur = Close, default)]
pub struct QuestionnaireValidator {
    client_info: Option<ActiveClientInfo>,
    view: ValidatorView,
    questionnaires: Vec<QuestionnaireSummary>,
    questionnaire_tree: TreeState<QuestionnaireSummary>,
    selected_questionnaire: Option<QuestionnaireSummary>,
    validation_report: Option<ValidationReport>,
    validation_tree: TreeState<ValidationTreeNode>,
    fetch_error: Option<String>,
}

impl QuestionnaireValidator {
    pub fn create() -> Self {
        Self::new(
            None,
            ValidatorView::List,
            Vec::new(),
            TreeState::default(),
            None,
            None,
            TreeState::default(),
            None,
        )
    }
}

#[app_impl]
impl QuestionnaireValidator {
    #[on_start]
    async fn on_start(&self, gx: &GlobalContext) {
        match gx
            .request_system::<ClientManagement, GetActiveClient>(GetActiveClient)
            .await
        {
            Ok(Ok(info)) => {
                self.client_info.set(Some(info));
                self.fetch_questionnaires(gx).await;
            }
            Ok(Err(e)) => {
                let message = format!("Client error: {}", e);
                self.fetch_error.set(Some(message.clone()));
                gx.toast(Toast::error(message));
            }
            Err(e) => {
                let message = format!(
                    "No active client. Please configure a connection first. ({:?})",
                    e
                );
                self.fetch_error.set(Some(message.clone()));
                gx.toast(Toast::error(message));
            }
        }
    }

    fn title(&self) -> String {
        let env_name = self
            .client_info
            .with_ref(|info| info.as_ref().map(|info| info.environment_name.clone()))
            .unwrap_or_else(|| "No active environment".to_string());
        match self.view.get() {
            ValidatorView::List => format!("Questionnaire Validator — {}", env_name),
            ValidatorView::Detail => {
                let name = self
                    .selected_questionnaire
                    .with_ref(|questionnaire| questionnaire.as_ref().map(|q| q.name.clone()))
                    .unwrap_or_else(|| "No questionnaire".to_string());
                format!("Questionnaire Validator — {} — {}", env_name, name)
            }
        }
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", close_or_back);
        bind("enter", activate_questionnaire);
        bind("r", refresh);
    }

    #[handler]
    async fn close_or_back(&self, cx: &AppContext) {
        if self.view.get() == ValidatorView::Detail {
            self.view.set(ValidatorView::List);
            self.selected_questionnaire.set(None);
            self.validation_report.set(None);
            self.validation_tree
                .update(|tree| tree.set_roots(Vec::new()));
        } else {
            cx.close();
        }
    }

    #[handler]
    async fn activate_questionnaire(&self, gx: &GlobalContext) {
        if self.view.get() != ValidatorView::List {
            return;
        }

        let focused_key = self.questionnaire_tree.with_ref(|tree| {
            tree.focused_key
                .clone()
                .or_else(|| tree.last_activated.clone())
        });
        let Some(focused_key) = focused_key else {
            gx.toast(Toast::error("Focus a questionnaire first"));
            return;
        };
        let questionnaire = self
            .questionnaires
            .with_ref(|items| items.iter().find(|item| item.id == focused_key).cloned());
        let Some(questionnaire) = questionnaire else {
            gx.toast(Toast::error("Questionnaire not found"));
            return;
        };

        self.load_questionnaire_detail(gx, questionnaire).await;
    }

    #[handler]
    async fn refresh(&self, gx: &GlobalContext) {
        if self.client_info.get().is_none() && !self.ensure_client(gx).await {
            return;
        }

        match self.view.get() {
            ValidatorView::List => self.fetch_questionnaires(gx).await,
            ValidatorView::Detail => {
                let questionnaire = self.selected_questionnaire.get();
                if let Some(questionnaire) = questionnaire {
                    self.load_questionnaire_detail(gx, questionnaire).await;
                }
            }
        }
    }

    async fn ensure_client(&self, gx: &GlobalContext) -> bool {
        match gx
            .request_system::<ClientManagement, GetActiveClient>(GetActiveClient)
            .await
        {
            Ok(Ok(info)) => {
                self.client_info.set(Some(info));
                true
            }
            Ok(Err(e)) => {
                let message = format!("Client error: {}", e);
                self.fetch_error.set(Some(message.clone()));
                gx.toast(Toast::error(message));
                false
            }
            Err(e) => {
                let message = format!(
                    "No active client. Please configure a connection first. ({:?})",
                    e
                );
                self.fetch_error.set(Some(message.clone()));
                gx.toast(Toast::error(message));
                false
            }
        }
    }

    async fn fetch_questionnaires(&self, gx: &GlobalContext) {
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

    async fn load_questionnaire_detail(
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
                records.sort_by(|a, b| record_name(a).cmp(&record_name(b)));
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

    fn element(&self) -> Element {
        match self.view.get() {
            ValidatorView::List => self.list_element(),
            ValidatorView::Detail => self.detail_element(),
        }
    }

    fn list_element(&self) -> Element {
        let env_name = self
            .client_info
            .with_ref(|info| info.as_ref().map(|info| info.environment_name.clone()))
            .unwrap_or_else(|| "No active environment".to_string());
        let fetch_error = self.fetch_error.get();
        let count = self.questionnaires.with_ref(|items| items.len());
        let active_count = self
            .questionnaires
            .with_ref(|items| items.iter().filter(|item| item.is_active()).count());
        let inactive_count = count.saturating_sub(active_count);

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: background) {
                text (content: "Questionnaire Validator") style (bold, fg: interact)
                row (width: fill, justify: between) {
                    text (content: {format!("Environment: {}", env_name)}) style (fg: primary)
                    text (content: {format!("{} questionnaires · {} active · {} inactive", count, active_count, inactive_count)}) style (fg: muted)
                }

                if let Some(error) = fetch_error {
                    text (content: {error}) style (fg: error)
                }

                if count == 0 {
                    box_ (width: fill, height: fill) style (bg: surface) {
                        column (padding: (1, 2), gap: 1) {
                            text (content: "No questionnaires loaded.") style (fg: muted)
                            text (content: "Press r to refresh.") style (fg: muted)
                        }
                    }
                } else {
                    box_ (id: "questionnaire-validator-list-container", width: fill, height: fill) style (bg: surface) {
                        tree (state: self.questionnaire_tree, id: "questionnaire-validator-list", width: fill, height: fill) on_activate: activate_questionnaire()
                    }
                }

                row (width: fill, justify: between) {
                    text (content: "enter validate · r refresh") style (fg: muted)
                    button (label: "Close", hint: "esc", id: "questionnaire-validator-close") on_activate: close_or_back()
                }
            }
        }
    }

    fn detail_element(&self) -> Element {
        let fetch_error = self.fetch_error.get();
        let questionnaire = self.selected_questionnaire.get();
        let name = questionnaire
            .as_ref()
            .map(|questionnaire| questionnaire.name.clone())
            .unwrap_or_else(|| "No questionnaire".to_string());
        let state = questionnaire
            .as_ref()
            .map(|questionnaire| questionnaire.state_label().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let (record_count, finding_count) = self.validation_report.with_ref(|report| {
            report
                .as_ref()
                .map(|report| (report.record_count, report.finding_count))
                .unwrap_or((0, 0))
        });
        let status = validation_status(finding_count);
        let status_color = validation_color(finding_count);

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: background) {
                text (content: "Questionnaire Validation") style (bold, fg: interact)
                row (width: fill, justify: between) {
                    text (content: {name}) style (fg: primary)
                    text (content: {format!("{} · {} records · {} findings", state, record_count, finding_count)}) style (fg: muted)
                    text (content: {status}) style (fg: {Color::var(status_color)})
                }

                if let Some(error) = fetch_error {
                    text (content: {error}) style (fg: error)
                }

                if self.validation_report.get().is_none() {
                    box_ (width: fill, height: fill) style (bg: surface) {
                        column (padding: (1, 2), gap: 1) {
                            text (content: "Loading validation details...") style (fg: muted)
                        }
                    }
                } else {
                    box_ (id: "questionnaire-validator-detail-container", width: fill, height: fill) style (bg: surface) {
                        tree (state: self.validation_tree, id: "questionnaire-validator-detail-tree", width: fill, height: fill)
                    }
                }

                row (width: fill, justify: between) {
                    text (content: "r refresh") style (fg: muted)
                    button (label: "Back", hint: "esc", id: "questionnaire-validator-back") on_activate: close_or_back()
                }
            }
        }
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

fn build_validation_report(
    questionnaire: QuestionnaireSummary,
    graph: QuestionnaireGraph,
    metadata: HashMap<String, EntityMetadata>,
) -> ValidationReport {
    let option_values = build_option_value_map(&metadata);
    let mut entities = Vec::new();
    let mut record_count = 0;
    let mut finding_count = 0;

    for set in graph.records_by_entity {
        let mut records = Vec::new();
        let Some(spec) = entity_spec(&set.entity) else {
            continue;
        };

        for record in &set.records {
            record_count += 1;
            let record_id = guid_value(record, spec.primary_key)
                .map(|id| id.to_string())
                .unwrap_or_else(|| "unknown-id".to_string());
            let name = record_name(record);
            let fields = validate_record_options(&set.entity, record, &option_values);
            let record_findings = fields.iter().filter(|field| !field.valid).count();
            finding_count += record_findings;
            records.push(RecordValidation {
                entity: set.entity.clone(),
                record_id,
                name,
                fields,
            });
        }

        let entity_findings = records.iter().map(RecordValidation::finding_count).sum();
        entities.push(EntityValidation {
            entity: set.entity,
            record_count: set.records.len(),
            findings_count: entity_findings,
            records,
        });
    }

    ValidationReport {
        questionnaire,
        record_count,
        finding_count,
        entities,
    }
}

fn build_validation_tree(report: &ValidationReport) -> Vec<TreeNode<ValidationTreeNode>> {
    let children = report
        .entities
        .iter()
        .map(|entity| {
            let records = entity
                .records
                .iter()
                .map(|record| {
                    let fields = record
                        .fields
                        .iter()
                        .map(|field| {
                            TreeNode::leaf(ValidationTreeNode::Field {
                                entity: record.entity.clone(),
                                id: record.record_id.clone(),
                                field: field.field.clone(),
                                value: field.value,
                                accepted_values: field.accepted_values.clone(),
                                valid: field.valid,
                            })
                        })
                        .collect::<Vec<_>>();
                    let finding_count = record.finding_count();
                    if fields.is_empty() {
                        TreeNode::leaf(ValidationTreeNode::Record {
                            entity: record.entity.clone(),
                            id: record.record_id.clone(),
                            name: record.name.clone(),
                            finding_count,
                        })
                    } else {
                        TreeNode::branch(
                            ValidationTreeNode::Record {
                                entity: record.entity.clone(),
                                id: record.record_id.clone(),
                                name: record.name.clone(),
                                finding_count,
                            },
                            fields,
                        )
                    }
                })
                .collect::<Vec<_>>();
            TreeNode::branch(
                ValidationTreeNode::Entity {
                    entity: entity.entity.clone(),
                    finding_count: entity.findings_count,
                    record_count: entity.record_count,
                },
                records,
            )
        })
        .collect::<Vec<_>>();

    vec![TreeNode::branch(
        ValidationTreeNode::Root {
            name: report.questionnaire.name.clone(),
            finding_count: report.finding_count,
            record_count: report.record_count,
        },
        children,
    )]
}

fn build_option_value_map(
    metadata: &HashMap<String, EntityMetadata>,
) -> HashMap<(String, String), Vec<i32>> {
    let mut map = HashMap::new();
    for (entity, entity_metadata) in metadata {
        for attribute in &entity_metadata.picklist_attributes {
            if !attribute.logical_name.starts_with("nrq_") {
                continue;
            }
            map.insert(
                (entity.clone(), attribute.logical_name.clone()),
                attribute
                    .option_set
                    .options
                    .iter()
                    .map(|option| option.value)
                    .collect(),
            );
        }
        for attribute in &entity_metadata.multi_select_picklist_attributes {
            if !attribute.logical_name.starts_with("nrq_") {
                continue;
            }
            map.insert(
                (entity.clone(), attribute.logical_name.clone()),
                attribute
                    .option_set
                    .options
                    .iter()
                    .map(|option| option.value)
                    .collect(),
            );
        }
        for attribute in &entity_metadata.state_attributes {
            if !attribute.logical_name.starts_with("nrq_") {
                continue;
            }
            map.insert(
                (entity.clone(), attribute.logical_name.clone()),
                attribute
                    .option_set
                    .options
                    .iter()
                    .map(|option| option.value)
                    .collect(),
            );
        }
        for attribute in &entity_metadata.status_attributes {
            if !attribute.logical_name.starts_with("nrq_") {
                continue;
            }
            map.insert(
                (entity.clone(), attribute.logical_name.clone()),
                attribute
                    .option_set
                    .options
                    .iter()
                    .map(|option| option.value)
                    .collect(),
            );
        }
    }
    log::debug!(
        "Questionnaire validator option-set metadata fields: {}",
        map.len()
    );
    map
}

fn validate_record_options(
    entity: &str,
    record: &Record,
    option_values: &HashMap<(String, String), Vec<i32>>,
) -> Vec<ValidatedField> {
    let mut fields = Vec::new();
    let mut option_fields = option_values
        .keys()
        .filter_map(|(option_entity, field)| {
            if option_entity == entity {
                Some(field.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    option_fields.sort();

    for field in option_fields {
        let Some(value) = record.get(&field) else {
            add_missing_option_field(entity, &field, option_values, &mut fields);
            continue;
        };

        match value {
            Value::OptionSet(option) => {
                validate_option_value(entity, &field, option.value, option_values, &mut fields);
            }
            Value::MultiOptionSet(options) => {
                if options.values.is_empty() {
                    add_missing_option_field(entity, &field, option_values, &mut fields);
                }
                for value in &options.values {
                    validate_option_value(entity, &field, *value, option_values, &mut fields);
                }
            }
            Value::Int(value) => {
                validate_option_value(entity, &field, *value, option_values, &mut fields);
            }
            Value::Long(value) => {
                if let Ok(value) = i32::try_from(*value) {
                    validate_option_value(entity, &field, value, option_values, &mut fields);
                }
            }
            Value::String(value) => {
                let parsed = parse_option_string(value);
                if parsed.is_empty() {
                    add_missing_option_field(entity, &field, option_values, &mut fields);
                }
                for value in parsed {
                    validate_option_value(entity, &field, value, option_values, &mut fields);
                }
            }
            Value::Null => {
                add_missing_option_field(entity, &field, option_values, &mut fields);
            }
            _ => {
                add_missing_option_field(entity, &field, option_values, &mut fields);
            }
        }
    }
    fields.sort_by(|a, b| a.field.cmp(&b.field).then(a.value.cmp(&b.value)));
    fields
}

fn parse_option_string(value: &str) -> Vec<i32> {
    value
        .split(',')
        .filter_map(|part| part.trim().parse::<i32>().ok())
        .collect()
}

fn add_missing_option_field(
    entity: &str,
    field: &str,
    option_values: &HashMap<(String, String), Vec<i32>>,
    fields: &mut Vec<ValidatedField>,
) {
    let key = (entity.to_string(), field.to_string());
    let Some(accepted_values) = option_values.get(&key) else {
        return;
    };
    fields.push(ValidatedField {
        field: field.to_string(),
        value: None,
        accepted_values: accepted_values.clone(),
        valid: true,
    });
}

fn validate_option_value(
    entity: &str,
    field: &str,
    value: i32,
    option_values: &HashMap<(String, String), Vec<i32>>,
    fields: &mut Vec<ValidatedField>,
) {
    let key = (entity.to_string(), field.to_string());
    let Some(accepted_values) = option_values.get(&key) else {
        return;
    };
    fields.push(ValidatedField {
        field: field.to_string(),
        value: Some(value),
        accepted_values: accepted_values.clone(),
        valid: accepted_values.contains(&value),
    });
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

fn entity_spec(logical_name: &str) -> Option<&'static QuestionnaireEntitySpec> {
    QUESTIONNAIRE_ENTITIES
        .iter()
        .find(|spec| spec.logical_name == logical_name)
}

fn record_name(record: &Record) -> String {
    string_value(record, "nrq_name").unwrap_or_else(|| {
        record
            .fields()
            .iter()
            .find_map(|(field, value)| {
                if field.ends_with("id") {
                    if let Value::Guid(id) = value {
                        return Some(short_id(&id.to_string()));
                    }
                }
                None
            })
            .unwrap_or_else(|| "unknown name".to_string())
    })
}

fn validation_status(finding_count: usize) -> &'static str {
    if finding_count == 0 { "ok" } else { "error" }
}

fn validation_color(finding_count: usize) -> &'static str {
    if finding_count == 0 {
        "success"
    } else {
        "error"
    }
}

fn format_accepted_values(values: &[i32]) -> String {
    let mut values = values.to_vec();
    values.sort();
    let sample = values
        .iter()
        .take(12)
        .map(i32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    if values.len() > 12 {
        format!("accepted: {},...", sample)
    } else {
        format!("accepted: {}", sample)
    }
}

fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

fn string_value(record: &Record, field: &str) -> Option<String> {
    match record.get(field) {
        Some(Value::String(value)) => Some(value.clone()),
        _ => None,
    }
}

fn guid_value(record: &Record, field: &str) -> Option<Uuid> {
    match record.get(field) {
        Some(Value::Guid(value)) => Some(*value),
        _ => None,
    }
}

fn option_value(record: &Record, field: &str) -> Option<i32> {
    match record.get(field) {
        Some(Value::OptionSet(value)) => Some(value.value),
        Some(Value::Int(value)) => Some(*value),
        _ => None,
    }
}
