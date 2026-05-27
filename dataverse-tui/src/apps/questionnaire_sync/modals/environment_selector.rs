//! Modal for selecting source and target environments for questionnaire sync.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Select, SelectState};
use tuidom::Element;

/// Result of the environment selector modal.
#[derive(Debug, Clone)]
pub struct EnvironmentSelection {
    pub source_environment_id: i64,
    pub target_environment_id: i64,
}

/// Modal for selecting a source and target environment.
#[modal(default, size = Md)]
pub struct EnvironmentSelectorModal {
    #[state(skip)]
    environments: Vec<(i64, String)>,

    source_env: SelectState<i64>,
    target_env: SelectState<i64>,
    error: Option<String>,
}

impl EnvironmentSelectorModal {
    /// Create a new selector modal with preloaded environment options.
    pub fn with_environments(environments: Vec<(i64, String)>) -> Self {
        Self::new(
            environments.clone(),
            SelectState::new(environments.clone()),
            SelectState::new(environments),
            None,
        )
    }
}

#[modal_impl]
impl EnvironmentSelectorModal {
    fn default_result(&self) -> Option<EnvironmentSelection> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<EnvironmentSelection>>) {
        mx.focus("questionnaire-sync-source-environment");
    }

    #[keybinds]
    fn keys() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<EnvironmentSelection>>) {
        mx.close(None);
    }

    #[handler]
    async fn submit(&self, gx: &GlobalContext, mx: &ModalContext<Option<EnvironmentSelection>>) {
        self.error.set(None);
        let source_id = self.source_env.with_ref(|state| state.value().cloned());
        let target_id = self.target_env.with_ref(|state| state.value().cloned());

        let (Some(source_id), Some(target_id)) = (source_id, target_id) else {
            self.error.set(Some(
                "Please select both source and target environments".to_string(),
            ));
            return;
        };

        if source_id == target_id {
            self.error.set(Some(
                "Source and target environments must be different".to_string(),
            ));
            gx.toast(Toast::error(
                "Source and target environments must be different",
            ));
            return;
        }

        mx.close(Some(EnvironmentSelection {
            source_environment_id: source_id,
            target_environment_id: target_id,
        }));
    }

    fn element(&self) -> Element {
        let has_environments = !self.environments.is_empty();
        let error = self.error.get();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Questionnaire Sync") style (bold, fg: interact)
                text (content: "Select the source and target Dataverse environments.") style (fg: muted)

                if let Some(err) = error {
                    text (content: {err}) style (fg: error)
                }

                if has_environments {
                    column (gap: 1, width: fill) {
                        text (content: "Source Environment") style (fg: muted)
                        select (state: self.source_env, id: "questionnaire-sync-source-environment", placeholder: "Select source...")

                        text (content: "Target Environment") style (fg: muted)
                        select (state: self.target_env, id: "questionnaire-sync-target-environment", placeholder: "Select target...")
                    }
                } else {
                    column (width: fill, height: fill, justify: center, align: center) {
                        text (content: "No environments available") style (fg: muted)
                        text (content: "Configure environments first") style (fg: muted)
                    }
                }

                column (flex_grow: 1) {}

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "questionnaire-sync-cancel") on_activate: cancel()
                    if has_environments {
                        button (label: "Continue", hint: "enter", id: "questionnaire-sync-submit") on_activate: submit()
                    }
                }
            }
        }
    }
}
