//! Questionnaire Sync app.

pub mod modals;

use std::collections::HashMap;

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use tuidom::Element;

use crate::apps::questionnaire_sync::modals::{EnvironmentSelection, EnvironmentSelectorModal};
use crate::credentials::CredentialsProvider;

/// Questionnaire sync app.
#[app(name = "VAF - Questionnaire Sync", singleton, on_blur = Close, default)]
pub struct QuestionnaireSync {
    env_names: HashMap<i64, String>,
    env_options: Vec<(i64, String)>,
    source_environment_id: Option<i64>,
    target_environment_id: Option<i64>,
    source_environment_name: Option<String>,
    target_environment_name: Option<String>,
}

#[app_impl]
impl QuestionnaireSync {
    pub fn create() -> Self {
        Self::new(HashMap::new(), Vec::new(), None, None, None, None)
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
        if self.source_environment_id.get().is_none() || self.target_environment_id.get().is_none() {
            gx.toast(Toast::error("Select source and target environments first"));
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

                if has_selection {
                    text (content: "Selection ready. The fetch phase can be launched next.") style (fg: muted)
                } else {
                    text (content: "Select a source and target environment to continue.") style (fg: muted)
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
