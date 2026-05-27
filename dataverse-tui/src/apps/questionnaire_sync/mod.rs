//! Questionnaire Sync app.

pub mod modals;

use std::collections::HashMap;

use dataverse_lib::model::Entity;
use dataverse_lib::model::Record;
use dataverse_lib::DataverseClient;
use rafter::element;
use rafter::page;
use rafter::prelude::*;
use tuidom::Element;

use crate::apps::questionnaire_sync::modals::{EnvironmentSelection, EnvironmentSelectorModal};
use crate::credentials::CredentialsProvider;
use crate::modals::odata_fetch::ODataFetchError;
use crate::modals::odata_fetch::ODataFetchModal;
use crate::modals::odata_fetch::ODataFetchTask;
use crate::systems::client_management::ClientManagement;
use crate::systems::client_management::GetAnyClient;

#[derive(Clone)]
struct EntitySnapshot {
    entity: String,
    record_count: usize,
    records: Vec<Record>,
}

#[derive(Clone)]
struct EnvironmentSnapshot {
    environment_id: i64,
    environment_name: String,
    entities: Vec<EntitySnapshot>,
}

#[derive(Clone)]
struct FetchSpec {
    environment_id: i64,
    environment_name: String,
    entity: &'static str,
}

impl FetchSpec {
    fn new(environment_id: i64, environment_name: String, entity: &'static str) -> Self {
        Self {
            environment_id,
            environment_name,
            entity,
        }
    }

    fn label(&self) -> String {
        format!("{} — {}", self.environment_name, self.entity)
    }
}

fn questionnaire_sync_entities() -> Vec<&'static str> {
    vec![
        "nrq_questionnaire",
        "nrq_questionnairepage",
        "nrq_questionnairepageline",
        "nrq_questiongroup",
        "nrq_questiongroupline",
        "nrq_question",
        "nrq_questiontemplateline",
        "nrq_questioncondition",
        "nrq_questionconditionaction",
        "nrq_questiontemplate",
        "nrq_questiontag",
        "nrq_role",
        "nrq_pdfreport",
        "nrq_domain",
        "nrq_type",
        "nrq_fund",
        "nrq_support",
        "nrq_category",
        "nrq_subcategory",
        "nrq_flemishshare",
        "nrq_betalingsschijf",
        "nrq_betalingsschijflijn",
        "nrq_grootboekrekening",
        "nrq_kostenplaats",
    ]
}

/// Questionnaire sync app.
#[app(name = "VAF - Questionnaire Sync", singleton, on_blur = Close, default)]
pub struct QuestionnaireSync {
    env_names: HashMap<i64, String>,
    env_options: Vec<(i64, String)>,
    source_environment_id: Option<i64>,
    target_environment_id: Option<i64>,
    source_environment_name: Option<String>,
    target_environment_name: Option<String>,
    source_snapshot: Option<EnvironmentSnapshot>,
    target_snapshot: Option<EnvironmentSnapshot>,
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
        bind("q", queue);
    }

    #[handler]
    async fn close_app(&self, cx: &AppContext) {
        cx.close();
    }

    #[handler]
    async fn queue(&self, gx: &GlobalContext) {
        if self.source_snapshot.get().is_none() || self.target_snapshot.get().is_none() {
            gx.toast(Toast::error("Fetch the environments first"));
            return;
        }

        gx.toast(Toast::info("Queue phase coming soon"));
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

        let source_client = match self.get_client_for_env(gx, source_env_id).await {
            Some(client) => client,
            None => return,
        };
        let target_client = match self.get_client_for_env(gx, target_env_id).await {
            Some(client) => client,
            None => return,
        };

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
        for entity in questionnaire_sync_entities() {
            specs.push(FetchSpec::new(source_env_id, source_env_name.clone(), entity));
            specs.push(FetchSpec::new(target_env_id, target_env_name.clone(), entity));
        }

        let tasks: Vec<ODataFetchTask> = specs
            .iter()
            .map(|spec| {
                let client = if spec.environment_id == source_env_id {
                    source_client.clone()
                } else {
                    target_client.clone()
                };
                let query = client.query(Entity::logical(spec.entity)).page_size(1000);
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
        for (spec, records) in specs.into_iter().zip(results.into_iter()) {
            let entity_snapshot = EntitySnapshot {
                entity: spec.entity.to_string(),
                record_count: records.len(),
                records,
            };
            if spec.environment_id == source_env_id {
                source_entities.push(entity_snapshot);
            } else {
                target_entities.push(entity_snapshot);
            }
        }

        self.source_snapshot.set(Some(EnvironmentSnapshot {
            environment_id: source_env_id,
            environment_name: source_env_name,
            entities: source_entities,
        }));
        self.target_snapshot.set(Some(EnvironmentSnapshot {
            environment_id: target_env_id,
            environment_name: target_env_name,
            entities: target_entities,
        }));
        self.fetch_error.set(None);
        gx.toast(Toast::success("Questionnaire data fetched"));
    }

    async fn get_client_for_env(
        &self,
        gx: &GlobalContext,
        env_id: i64,
    ) -> Option<DataverseClient> {
        match gx
            .request_system::<ClientManagement, GetAnyClient>(GetAnyClient { env_id })
            .await
        {
            Ok(Ok(info)) => Some(info.client),
            Ok(Err(e)) => {
                gx.toast(Toast::error(format!("Failed to connect to environment: {}", e)));
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

    fn element(&self) -> Element {
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
        let source_snapshot = self.source_snapshot.get();
        let target_snapshot = self.target_snapshot.get();
        let fetch_error = self.fetch_error.get();
        let source_total = source_snapshot
            .as_ref()
            .map(environment_total_records)
            .unwrap_or(0);
        let target_total = target_snapshot
            .as_ref()
            .map(environment_total_records)
            .unwrap_or(0);
        let source_snapshot_view = source_snapshot
            .as_ref()
            .map(|snapshot| render_snapshot("Source data", snapshot));
        let target_snapshot_view = target_snapshot
            .as_ref()
            .map(|snapshot| render_snapshot("Target data", snapshot));

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: background) {
                text (content: "Questionnaire Sync") style (bold, fg: interact)
                text (content: {format!("Loaded environments: {}", env_count)}) style (fg: muted)

                column (gap: 1, width: fill) {
                    row (width: fill, justify: between) {
                        text (content: "Source") style (fg: muted)
                        text (content: {source_name.clone()}) style (fg: primary)
                    }
                    row (width: fill, justify: between) {
                        text (content: "Target") style (fg: muted)
                        text (content: {target_name.clone()}) style (fg: primary)
                    }
                }

                if let Some(err) = fetch_error {
                    text (content: {err}) style (fg: error)
                }

                if has_selection {
                    text (content: "Selection ready. The fetch phase runs automatically.") style (fg: muted)
                } else {
                    text (content: "Select a source and target environment to continue.") style (fg: muted)
                }

                column (gap: 1, width: fill) {
                    row (width: fill, justify: between) {
                        text (content: "Source snapshot") style (fg: muted)
                        text (content: {format!("{} records", source_total)}) style (fg: primary)
                    }
                    row (width: fill, justify: between) {
                        text (content: "Target snapshot") style (fg: muted)
                        text (content: {format!("{} records", target_total)}) style (fg: primary)
                    }
                }

                if let Some(snapshot_view) = source_snapshot_view {
                    { snapshot_view }
                }

                if let Some(snapshot_view) = target_snapshot_view {
                    { snapshot_view }
                }

                column (flex_grow: 1) {}

                row (width: fill, justify: between) {
                    button (label: "Close", hint: "esc", id: "questionnaire-sync-close") on_activate: close_app()
                    button (label: "Queue", hint: "q", id: "questionnaire-sync-queue") on_activate: queue()
                }
            }
        }
    }
}

fn environment_total_records(snapshot: &EnvironmentSnapshot) -> usize {
    snapshot.entities.iter().map(|entity| entity.record_count).sum()
}

fn render_snapshot(title: &str, snapshot: &EnvironmentSnapshot) -> Element {
    let entity_rows: Vec<Element> = snapshot
        .entities
        .iter()
        .map(|entity| {
            element! {
                row (width: fill, justify: between) {
                    text (content: {entity.entity.clone()}) style (fg: primary)
                    text (content: {format!("{}", entity.record_count)}) style (fg: muted)
                }
            }
        })
        .collect();

    element! {
        column (gap: 1, width: fill) {
            text (content: {format!("{} — {} (#{})", title, snapshot.environment_name, snapshot.environment_id)}) style (fg: interact)
            column (gap: 0, width: fill) {
                ...entity_rows
            }
        }
    }
}
