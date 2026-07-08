//! Modal for configuring an imported migration.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::Input;
use rafter::widgets::Select;
use rafter::widgets::SelectState;
use rafter::widgets::Text;
use tuidom::Element;

use crate::apps::migration::repository::MigrationExportBundle;
use crate::credentials::Environment;

/// Result of the import migration modal.
#[derive(Clone, Debug)]
pub struct ImportMigrationResult {
    pub name: String,
    pub source_environment_id: i64,
    pub target_environment_id: i64,
}

/// Modal for choosing local environments for an imported migration.
#[modal(size = Md)]
pub struct ImportMigrationModal {
    #[state(skip)]
    bundle: MigrationExportBundle,
    name: String,
    source_env: SelectState<i64>,
    target_env: SelectState<i64>,
    error: Option<String>,
}

impl ImportMigrationModal {
    /// Create a modal for an imported bundle and local environment list.
    pub fn with_bundle(bundle: MigrationExportBundle, environments: Vec<Environment>) -> Self {
        let source_match = find_env_by_url(&environments, &bundle.source_environment.url);
        let target_match = find_env_by_url(&environments, &bundle.target_environment.url);
        let options = environment_options(&environments);

        let mut source_env = SelectState::new(options.clone());
        if let Some(id) = source_match {
            source_env = source_env.with_value(id);
        }

        let mut target_env = SelectState::new(options);
        if let Some(id) = target_match {
            target_env = target_env.with_value(id);
        }

        Self::new(
            bundle.clone(),
            bundle.migration.name,
            source_env,
            target_env,
            None,
        )
    }
}

#[modal_impl]
impl ImportMigrationModal {
    fn default_result(&self) -> Option<ImportMigrationResult> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<ImportMigrationResult>>) {
        mx.focus("import-migration-name");
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<ImportMigrationResult>>) {
        mx.close(None);
    }

    #[handler]
    async fn submit(&self, mx: &ModalContext<Option<ImportMigrationResult>>) {
        let name = self.name.get().trim().to_string();
        if name.is_empty() {
            self.error.set(Some("Name is required".to_string()));
            return;
        }

        let source_environment_id = self.source_env.with_ref(|s| s.value().copied());
        let target_environment_id = self.target_env.with_ref(|s| s.value().copied());

        let (Some(source_environment_id), Some(target_environment_id)) =
            (source_environment_id, target_environment_id)
        else {
            self.error.set(Some(
                "Please select source and target environments".to_string(),
            ));
            return;
        };

        mx.close(Some(ImportMigrationResult {
            name,
            source_environment_id,
            target_environment_id,
        }));
    }

    fn element(&self) -> Element {
        let error = self.error.get();
        let has_envs = self.source_env.with_ref(|s| !s.options.is_empty());
        let source_summary = format!(
            "{} ({})",
            self.bundle.source_environment.display_name, self.bundle.source_environment.url
        );
        let target_summary = format!(
            "{} ({})",
            self.bundle.target_environment.display_name, self.bundle.target_environment.url
        );
        let phase_count = self.bundle.migration.phases.len();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Import Migration") style (bold, fg: interact)

                if let Some(err) = error {
                    text (content: {err}) style (fg: error)
                }

                column (gap: 1, width: fill, height: fill) {
                    text (content: {format!("{} phases", phase_count)}) style (fg: muted)
                    text (content: "Exported source") style (fg: muted)
                    text (content: {source_summary}) style (fg: primary)
                    text (content: "Exported target") style (fg: muted)
                    text (content: {target_summary}) style (fg: primary)

                    text (content: "Name") style (fg: muted)
                    input (state: self.name, id: "import-migration-name", placeholder: "Migration name...")
                        on_submit: submit()

                    if has_envs {
                        text (content: "Local Source Environment") style (fg: muted)
                        select (state: self.source_env, id: "import-migration-source", placeholder: "Select source...")

                        text (content: "Local Target Environment") style (fg: muted)
                        select (state: self.target_env, id: "import-migration-target", placeholder: "Select target...")
                    } else {
                        column (width: fill, height: fill, justify: center, align: center) {
                            text (content: "No environments available") style (fg: muted)
                            text (content: "Please configure environments first") style (fg: muted)
                        }
                    }
                }

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn") on_activate: cancel()
                    if has_envs {
                        button (label: "Import", id: "import-btn") on_activate: submit()
                    }
                }
            }
        }
    }
}

fn find_env_by_url(environments: &[Environment], url: &str) -> Option<i64> {
    environments
        .iter()
        .find(|env| !url.is_empty() && env.url.eq_ignore_ascii_case(url))
        .map(|env| env.id)
}

fn environment_options(environments: &[Environment]) -> Vec<(i64, String)> {
    environments
        .iter()
        .map(|env| (env.id, format!("{} ({})", env.display_name, env.url)))
        .collect()
}
