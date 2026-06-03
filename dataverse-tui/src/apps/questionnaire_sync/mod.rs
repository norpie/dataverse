//! Questionnaire Sync app.

pub mod comparison;
pub mod execution;
pub mod modals;
pub mod scope;
pub mod tree;
pub mod types;

use std::collections::HashMap;

use dataverse_lib::model::Entity;
use dataverse_lib::model::Record;
use dataverse_lib::model::Value;
use rafter::page;
use rafter::prelude::*;
use tuidom::Element;

use crate::apps::migration::execution::SubPhase;
use crate::apps::questionnaire_sync::comparison::{QuestionnaireComparison, compare_questionnaire};
use crate::apps::questionnaire_sync::execution::build_execution_plan;
use crate::apps::questionnaire_sync::execution::fetch_execution_metadata;
use crate::apps::questionnaire_sync::modals::{EnvironmentSelection, EnvironmentSelectorModal};
use crate::apps::questionnaire_sync::scope::QUESTIONNAIRE_ENTITIES;
use crate::apps::questionnaire_sync::scope::QUESTIONNAIRE_RELATIONS;
use crate::apps::questionnaire_sync::scope::QuestionnaireEntitySpec;
use crate::apps::questionnaire_sync::scope::QuestionnaireRelationSpec;
use crate::apps::questionnaire_sync::tree::{
    QuestionnaireTreeNode, QuestionnaireTreeSide, build_tree_nodes,
};
use crate::apps::questionnaire_sync::types::{
    QuestionnaireEntitySnapshot, QuestionnaireEnvironmentSnapshot, QuestionnaireRelationMembership,
    QuestionnaireRelationSnapshot,
};
use crate::apps::queue::Queue;
use crate::apps::queue::api::AddItems;
use crate::apps::queue::api::NewItem;
use crate::apps::queue::types::QueuePayload;
use crate::credentials::CredentialsProvider;
use crate::modals::odata_fetch::ODataFetchError;
use crate::modals::odata_fetch::ODataFetchModal;
use crate::modals::odata_fetch::ODataFetchTask;
use crate::systems::client_management::ActiveClientInfo;
use crate::systems::client_management::ClientManagement;
use crate::systems::client_management::GetAnyClient;

#[derive(Clone)]
enum FetchSpecKind {
    Entity(&'static QuestionnaireEntitySpec),
    Relation(&'static QuestionnaireRelationSpec),
}

#[derive(Clone)]
struct FetchSpec {
    environment_id: i64,
    environment_name: String,
    kind: FetchSpecKind,
}

impl FetchSpec {
    fn entity(
        environment_id: i64,
        environment_name: String,
        entity: &'static QuestionnaireEntitySpec,
    ) -> Self {
        Self {
            environment_id,
            environment_name,
            kind: FetchSpecKind::Entity(entity),
        }
    }

    fn relation(
        environment_id: i64,
        environment_name: String,
        relation: &'static QuestionnaireRelationSpec,
    ) -> Self {
        Self {
            environment_id,
            environment_name,
            kind: FetchSpecKind::Relation(relation),
        }
    }

    fn label(&self) -> String {
        match self.kind {
            FetchSpecKind::Entity(entity) => {
                format!("{} — {}", self.environment_name, entity.logical_name)
            }
            FetchSpecKind::Relation(relation) => {
                format!("{} — {}", self.environment_name, relation.relationship_name)
            }
        }
    }
}

fn entity_spec(logical_name: &str) -> &'static QuestionnaireEntitySpec {
    QUESTIONNAIRE_ENTITIES
        .iter()
        .find(|spec| spec.logical_name == logical_name)
        .unwrap_or_else(|| panic!("missing questionnaire entity spec: {}", logical_name))
}

fn relation_snapshot_from_records(
    relation: &QuestionnaireRelationSpec,
    records: Vec<Record>,
) -> QuestionnaireRelationSnapshot {
    let parent = entity_spec(relation.parent_entity);
    let related = entity_spec(relation.related_entity);
    let mut memberships = Vec::new();

    for record in records {
        let Some(parent_id) = guid_field(&record, parent.primary_key) else {
            continue;
        };
        let Some(Value::Records(related_records)) = record.get(relation.relationship_name) else {
            continue;
        };

        for related_record in related_records {
            if let Some(related_id) = guid_field(related_record, related.primary_key) {
                memberships.push(QuestionnaireRelationMembership {
                    parent_id,
                    related_id,
                });
            }
        }
    }

    QuestionnaireRelationSnapshot {
        relationship_name: relation.relationship_name.to_string(),
        parent_entity: relation.parent_entity.to_string(),
        related_entity: relation.related_entity.to_string(),
        memberships,
    }
}

fn guid_field(record: &Record, field: &str) -> Option<uuid::Uuid> {
    match record.get(field) {
        Some(Value::Guid(id)) => Some(*id),
        _ => None,
    }
}

fn sub_phase_priority(sub_phase: SubPhase) -> i32 {
    match sub_phase {
        SubPhase::Create => 70,
        SubPhase::Activate => 60,
        SubPhase::Update => 50,
        SubPhase::Associate => 40,
        SubPhase::Disassociate => 30,
        SubPhase::Deactivate => 20,
        SubPhase::Delete => 10,
    }
}

/// Questionnaire sync app.
#[app(name = "VAF - Questionnaire Sync", singleton, on_blur = Close, default)]
pub struct QuestionnaireSync {
    env_names: HashMap<i64, String>,
    env_options: Vec<(i64, String)>,
    source_environment_id: Option<i64>,
    target_environment_id: Option<i64>,
    source_account_id: Option<i64>,
    target_account_id: Option<i64>,
    source_environment_name: Option<String>,
    target_environment_name: Option<String>,
    source_snapshot: Option<QuestionnaireEnvironmentSnapshot>,
    target_snapshot: Option<QuestionnaireEnvironmentSnapshot>,
    comparison: Option<QuestionnaireComparison>,
    current_entity_index: usize,
    source_tree_state: TreeState<QuestionnaireTreeNode>,
    target_tree_state: TreeState<QuestionnaireTreeNode>,
    fetch_error: Option<String>,
}

#[app_impl]
impl QuestionnaireSync {
    pub fn create() -> Self {
        Self::new(
            HashMap::new(),
            Vec::new(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            0,
            TreeState::default(),
            TreeState::default(),
            None,
        )
    }

    #[on_start]
    async fn on_start(&self, gx: &GlobalContext, cx: &AppContext) {
        self.load_environments(gx).await;
        if !self.ensure_selection(gx).await {
            cx.close();
        }
    }

    fn title(&self) -> String {
        let source = self
            .source_environment_name
            .get()
            .clone()
            .unwrap_or_else(|| "Source: not selected".to_string());
        let target = self
            .target_environment_name
            .get()
            .clone()
            .unwrap_or_else(|| "Target: not selected".to_string());
        format!("Questionnaire Sync — {} → {}", source, target)
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", close_app);
        bind("left", previous_entity);
        bind("right", next_entity);
        bind("q", queue);
    }

    #[handler]
    async fn close_app(&self, cx: &AppContext) {
        cx.close();
    }

    #[handler]
    async fn queue(&self, gx: &GlobalContext) {
        let Some(comparison) = self.comparison.with_ref(|comparison| comparison.clone()) else {
            gx.toast(Toast::error("Fetch the environments first"));
            return;
        };
        let Some(target_env_id) = self.target_environment_id.get() else {
            gx.toast(Toast::error("Target environment is not selected"));
            return;
        };
        let Some(target_account_id) = self.target_account_id.get() else {
            gx.toast(Toast::error("Target account is not available"));
            return;
        };

        let Some(target_info) = self.get_client_for_env(gx, target_env_id).await else {
            return;
        };
        let metadata = match fetch_execution_metadata(&target_info.client).await {
            Ok(metadata) => metadata,
            Err(e) => {
                log::error!("Failed to prepare questionnaire sync metadata: {}", e);
                gx.toast(Toast::error("Failed to prepare queue metadata"));
                return;
            }
        };
        let plan = match build_execution_plan(&comparison, &metadata) {
            Ok(plan) => plan,
            Err(e) => {
                log::error!("Failed to build questionnaire sync plan: {}", e);
                gx.toast(Toast::error(format!("Failed to build sync plan: {}", e)));
                return;
            }
        };

        if plan.is_empty() {
            gx.toast(Toast::info("No questionnaire sync operations to queue"));
            return;
        }

        let mut items = Vec::new();
        for sub_phase in SubPhase::ALL {
            let priority = sub_phase_priority(*sub_phase);
            for entity_batches in plan.batches_for(*sub_phase) {
                for batch in &entity_batches.batches {
                    let op_count = batch.operation_count();
                    items.push(NewItem {
                        priority,
                        payload: QueuePayload::Batch(batch.clone()),
                        env_id: target_env_id,
                        account_id: target_account_id,
                        source: "questionnaire-sync".to_string(),
                        description: format!(
                            "{} {} ({})",
                            sub_phase.label(),
                            entity_batches.entity,
                            op_count
                        ),
                    });
                }
            }
        }

        let operation_count = plan.total_operations();
        match gx.request::<Queue, AddItems>(AddItems { items }).await {
            Ok(response) => gx.toast(Toast::success(format!(
                "Queued {} questionnaire sync operations in {} items",
                operation_count,
                response.ids.len()
            ))),
            Err(e) => {
                log::error!("Failed to queue questionnaire sync operations: {:?}", e);
                gx.toast(Toast::error(
                    "Failed to queue questionnaire sync operations",
                ));
            }
        }
    }

    #[handler]
    async fn previous_entity(&self) {
        self.shift_entity(-1);
    }

    #[handler]
    async fn next_entity(&self) {
        self.shift_entity(1);
    }

    fn rebuild_trees(&self, comparison: &QuestionnaireComparison) {
        let source_nodes = build_tree_nodes(
            comparison,
            QuestionnaireTreeSide::Source,
            self.current_entity_index.get(),
        );
        let target_nodes = build_tree_nodes(
            comparison,
            QuestionnaireTreeSide::Target,
            self.current_entity_index.get(),
        );

        self.source_tree_state.update(|state| {
            state.set_roots(source_nodes);
        });
        self.target_tree_state.update(|state| {
            state.set_roots(target_nodes);
        });
    }

    fn shift_entity(&self, delta: i32) {
        let Some(comparison) = self.comparison.get() else {
            return;
        };

        let len = comparison.entities.len();
        if len == 0 {
            return;
        }

        let current = self.current_entity_index.get() as i32;
        let next = (current + delta).rem_euclid(len as i32) as usize;
        self.current_entity_index.set(next);
        self.rebuild_trees(&comparison);
    }

    async fn ensure_selection(&self, gx: &GlobalContext) -> bool {
        let env_options = self.env_options.get();
        if env_options.is_empty() {
            return false;
        }

        let Some(result) = gx
            .modal(EnvironmentSelectorModal::with_environments(env_options))
            .await
        else {
            return false;
        };

        self.apply_selection(result, gx).await;
        self.run_fetch_phase(gx).await;
        true
    }

    async fn apply_selection(&self, result: EnvironmentSelection, gx: &GlobalContext) {
        let env_names = self.env_names.get();

        let source_name = match env_names.get(&result.source_environment_id) {
            Some(name) => name.clone(),
            None => {
                gx.toast(Toast::error("Could not resolve source environment"));
                return;
            }
        };
        let target_name = match env_names.get(&result.target_environment_id) {
            Some(name) => name.clone(),
            None => {
                gx.toast(Toast::error("Could not resolve target environment"));
                return;
            }
        };

        self.source_environment_id
            .set(Some(result.source_environment_id));
        self.target_environment_id
            .set(Some(result.target_environment_id));
        self.source_environment_name.set(Some(source_name));
        self.target_environment_name.set(Some(target_name));
    }

    async fn run_fetch_phase(&self, gx: &GlobalContext) {
        let Some(source_env_id) = self.source_environment_id.get() else {
            return;
        };
        let Some(target_env_id) = self.target_environment_id.get() else {
            return;
        };

        let source_info = match self.get_client_for_env(gx, source_env_id).await {
            Some(info) => info,
            None => return,
        };
        let target_info = match self.get_client_for_env(gx, target_env_id).await {
            Some(info) => info,
            None => return,
        };
        self.source_account_id.set(Some(source_info.account_id));
        self.target_account_id.set(Some(target_info.account_id));
        let source_client = source_info.client;
        let target_client = target_info.client;

        let source_env_name = self
            .source_environment_name
            .get()
            .clone()
            .unwrap_or_else(|| format!("#{}", source_env_id));
        let target_env_name = self
            .target_environment_name
            .get()
            .clone()
            .unwrap_or_else(|| format!("#{}", target_env_id));

        let mut specs = Vec::new();
        for entity in QUESTIONNAIRE_ENTITIES {
            specs.push(FetchSpec::entity(
                source_env_id,
                source_env_name.clone(),
                entity,
            ));
            specs.push(FetchSpec::entity(
                target_env_id,
                target_env_name.clone(),
                entity,
            ));
        }
        for relation in QUESTIONNAIRE_RELATIONS {
            specs.push(FetchSpec::relation(
                source_env_id,
                source_env_name.clone(),
                relation,
            ));
            specs.push(FetchSpec::relation(
                target_env_id,
                target_env_name.clone(),
                relation,
            ));
        }

        let tasks: Vec<ODataFetchTask> = specs
            .iter()
            .map(|spec| {
                let client = if spec.environment_id == source_env_id {
                    source_client.clone()
                } else {
                    target_client.clone()
                };
                let query = match spec.kind {
                    FetchSpecKind::Entity(entity) => client
                        .query(Entity::logical(entity.logical_name))
                        .page_size(1000),
                    FetchSpecKind::Relation(relation) => {
                        let parent = entity_spec(relation.parent_entity);
                        let related = entity_spec(relation.related_entity);
                        client
                            .query(Entity::logical(relation.parent_entity))
                            .select(&[parent.primary_key])
                            .expand(relation.relationship_name, |expand| {
                                expand.select(&[related.primary_key])
                            })
                            .page_size(1000)
                    }
                };
                ODataFetchTask::new(spec.label(), client, query)
            })
            .collect();

        let results: Vec<Vec<Record>> = match gx.modal(ODataFetchModal::create(tasks)).await {
            Ok(results) => results,
            Err(ODataFetchError::TaskFailed { label, error }) => {
                let msg = format!("Fetch failed for {}: {}", label, error);
                self.fetch_error.set(Some(msg.clone()));
                gx.toast(Toast::error(msg));
                return;
            }
            Err(ODataFetchError::Cancelled) => {
                self.fetch_error.set(Some("Fetch cancelled".to_string()));
                return;
            }
        };

        let mut source_entities = Vec::new();
        let mut target_entities = Vec::new();
        let mut source_relations = Vec::new();
        let mut target_relations = Vec::new();
        for (spec, records) in specs.into_iter().zip(results.into_iter()) {
            match spec.kind {
                FetchSpecKind::Entity(entity) => {
                    let entity_snapshot = QuestionnaireEntitySnapshot {
                        entity: entity.logical_name.to_string(),
                        records,
                    };
                    if spec.environment_id == source_env_id {
                        source_entities.push(entity_snapshot);
                    } else {
                        target_entities.push(entity_snapshot);
                    }
                }
                FetchSpecKind::Relation(relation) => {
                    let relation_snapshot = relation_snapshot_from_records(relation, records);
                    if spec.environment_id == source_env_id {
                        source_relations.push(relation_snapshot);
                    } else {
                        target_relations.push(relation_snapshot);
                    }
                }
            }
        }

        let source_snapshot = QuestionnaireEnvironmentSnapshot {
            environment_id: source_env_id,
            environment_name: source_env_name,
            entities: source_entities,
            relations: source_relations,
        };
        let target_snapshot = QuestionnaireEnvironmentSnapshot {
            environment_id: target_env_id,
            environment_name: target_env_name,
            entities: target_entities,
            relations: target_relations,
        };
        let comparison = compare_questionnaire(&source_snapshot, &target_snapshot);

        self.source_snapshot.set(Some(source_snapshot));
        self.target_snapshot.set(Some(target_snapshot));
        self.comparison.set(Some(comparison.clone()));
        self.current_entity_index.set(0);
        self.rebuild_trees(&comparison);
        self.fetch_error.set(None);
        gx.toast(Toast::success("Questionnaire data fetched"));
    }

    async fn get_client_for_env(
        &self,
        gx: &GlobalContext,
        env_id: i64,
    ) -> Option<ActiveClientInfo> {
        match gx
            .request_system::<ClientManagement, GetAnyClient>(GetAnyClient { env_id })
            .await
        {
            Ok(Ok(info)) => Some(info),
            Ok(Err(e)) => {
                gx.toast(Toast::error(format!(
                    "Failed to connect to environment: {}",
                    e
                )));
                None
            }
            Err(e) => {
                gx.toast(Toast::error(format!("Failed to request client: {:?}", e)));
                None
            }
        }
    }

    async fn load_environments(&self, gx: &GlobalContext) {
        let credentials = gx.data::<CredentialsProvider>();
        match credentials.list_environments().await {
            Ok(envs) => {
                let env_names: HashMap<i64, String> = envs
                    .iter()
                    .map(|env| (env.id, env.display_name.clone()))
                    .collect();
                let env_options: Vec<(i64, String)> = envs
                    .into_iter()
                    .map(|env| (env.id, env.display_name))
                    .collect();
                self.env_names.set(env_names);
                self.env_options.set(env_options);
            }
            Err(e) => {
                log::error!("Failed to load environments for Questionnaire Sync: {}", e);
                gx.toast(Toast::error("Failed to load environments"));
            }
        }
    }

    #[watch]
    async fn sync_tree_states(&self) {
        let source_state = self.source_tree_state.get();
        self.sync_target_tree_from_source(source_state);
    }

    fn sync_target_tree_from_source(
        &self,
        source_state: rafter::widgets::TreeState<QuestionnaireTreeNode>,
    ) {
        self.target_tree_state.update(|state| {
            state.expanded = source_state.expanded.clone();
            state.selection = source_state.selection.clone();
            state.scroll = source_state.scroll.clone();
            state.last_activated = source_state.last_activated.clone();
            state.focused_key = source_state.focused_key.clone();
            state.set_roots(state.roots.as_ref().clone());
        });
    }

    fn element(&self) -> Element {
        let start_time = std::time::Instant::now();
        let source_name = self
            .source_environment_name
            .get()
            .clone()
            .unwrap_or_else(|| "Not selected".to_string());
        let target_name = self
            .target_environment_name
            .get()
            .clone()
            .unwrap_or_else(|| "Not selected".to_string());
        let env_count = self.env_options.with_ref(|envs| envs.len());
        let has_selection = self.source_environment_id.get().is_some()
            && self.target_environment_id.get().is_some();
        let fetch_error = self.fetch_error.get();
        let current_entity_index = self.current_entity_index.get();
        let (entity_count, current_entity_name, current_entity_total) = self
            .comparison
            .with_ref(|comparison| {
                let Some(comparison) = comparison.as_ref() else {
                    return (0, String::from("No entity"), 0);
                };

                let entity_count = comparison.entities.len();
                let current_entity = comparison.entities.get(current_entity_index);
                let current_entity_name = current_entity
                    .map(|entity| entity.entity.clone())
                    .unwrap_or_else(|| "No entity".to_string());
                let current_entity_total = current_entity
                    .map(|entity: &crate::apps::questionnaire_sync::comparison::QuestionnaireEntityComparison| entity.total_records())
                    .unwrap_or(0);
                (entity_count, current_entity_name, current_entity_total)
            });
        let current_entity_position = if entity_count == 0 {
            0
        } else {
            current_entity_index + 1
        };
        log::debug!("Processed helpers {:?}", start_time.elapsed());

        let page_result = page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: background) {
                text (content: "Questionnaire Sync") style (bold, fg: interact)
                text (content: {format!("Loaded environments: {}", env_count)}) style (fg: muted)

                row (width: fill, justify: between) {
                    text (content: {format!("Source: {}", source_name)}) style (fg: primary)
                    text (content: {format!("Target: {}", target_name)}) style (fg: primary)
                }

                if let Some(err) = fetch_error {
                    text (content: {err}) style (fg: error)
                }

                if has_selection {
                    text (content: "Selection ready. The fetch phase runs automatically.") style (fg: muted)
                } else {
                    text (content: "Select a source and target environment to continue.") style (fg: muted)
                }

                row (width: fill, justify: between) {
                    text (content: {format!("Entity {} / {}", current_entity_position, entity_count)}) style (fg: muted)
                    text (content: {current_entity_name.clone()}) style (fg: primary)
                    text (content: {format!("{} rows", current_entity_total)}) style (fg: muted)
                }

                row (width: fill, height: fill) {
                    column (width: {tuidom::Size::Flex(1)}, height: fill, gap: 1) {
                        text (content: {format!("Source tree — {}", current_entity_name)}) style (fg: interact)
                        box_ (id: "questionnaire-sync-source-tree-container", width: fill, height: fill) style (bg: surface) {
                            tree (state: self.source_tree_state, id: "questionnaire-sync-source-tree", width: fill, height: fill)
                        }
                    }

                    column (width: 1) {}

                    column (width: {tuidom::Size::Flex(1)}, height: fill, gap: 1) {
                        text (content: {format!("Target tree — {}", current_entity_name)}) style (fg: interact)
                        box_ (id: "questionnaire-sync-target-tree-container", width: fill, height: fill) style (bg: surface) {
                            tree (state: self.target_tree_state, id: "questionnaire-sync-target-tree", width: fill, height: fill)
                        }
                    }
                }

                row (width: fill, justify: between) {
                    button (label: "Close", hint: "esc", id: "questionnaire-sync-close") on_activate: close_app()
                    button (label: "Queue", hint: "q", id: "questionnaire-sync-queue") on_activate: queue()
                }
            }
        };
        log::debug!("Built element in {:?}", start_time.elapsed());
        page_result
    }
}
