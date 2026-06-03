//! Questionnaire Validator app.

mod fetch;
mod tree;
mod types;
mod util;
mod validation;

use rafter::page;
use rafter::prelude::*;
use tuidom::Color;
use tuidom::Element;

use crate::systems::client_management::ActiveClientInfo;
use crate::systems::client_management::ClientManagement;
use crate::systems::client_management::GetActiveClient;

use self::tree::{ValidationTreeNode, validation_color, validation_status};
use self::types::{QuestionnaireSummary, ValidationReport, ValidatorView};

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
