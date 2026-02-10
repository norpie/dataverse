//! Modal for creating a new migration.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::Input;
use rafter::widgets::Select;
use rafter::widgets::SelectState;
use rafter::widgets::Text;
use tuidom::Element;

/// Result of the new migration modal.
#[derive(Debug, Clone)]
pub struct NewMigrationResult {
    pub name: String,
    pub description: Option<String>,
    pub source_environment_id: i64,
    pub target_environment_id: i64,
}

/// Modal for creating a new migration.
#[modal(size = Md)]
pub struct NewMigrationModal {
    name: String,
    description: String,
    source_env: SelectState<i64>,
    target_env: SelectState<i64>,
    error: Option<String>,
}

impl NewMigrationModal {
    /// Create a new migration modal with the given environment options.
    pub fn with_environments(environments: Vec<(i64, String)>) -> Self {
        Self::new(
            String::new(),
            String::new(),
            SelectState::new(environments.clone()),
            SelectState::new(environments),
            None,
        )
    }
}

#[modal_impl]
impl NewMigrationModal {
    fn default_result(&self) -> Option<NewMigrationResult> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<NewMigrationResult>>) {
        mx.focus("new-migration-name");
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<NewMigrationResult>>) {
        mx.close(None);
    }

    #[handler]
    async fn submit(&self, mx: &ModalContext<Option<NewMigrationResult>>) {
        let name = self.name.get().trim().to_string();
        if name.is_empty() {
            self.error.set(Some("Name is required".to_string()));
            return;
        }

        let source_id = self.source_env.with_ref(|s| s.value().cloned());
        let target_id = self.target_env.with_ref(|s| s.value().cloned());

        let (Some(source_id), Some(target_id)) = (source_id, target_id) else {
            self.error.set(Some(
                "Please select source and target environments".to_string(),
            ));
            return;
        };

        if source_id == target_id {
            self.error
                .set(Some("Source and target must be different".to_string()));
            return;
        }

        let description = {
            let desc = self.description.get().trim().to_string();
            if desc.is_empty() { None } else { Some(desc) }
        };

        mx.close(Some(NewMigrationResult {
            name,
            description,
            source_environment_id: source_id,
            target_environment_id: target_id,
        }));
    }

    fn element(&self) -> Element {
        let error = self.error.get();
        let has_envs = self.source_env.with_ref(|s| !s.options.is_empty());

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                // Title
                text (content: "New Migration") style (bold, fg: interact)

                // Error
                if let Some(err) = error {
                    text (content: {err}) style (fg: error)
                }

                // Form content
                if has_envs {
                    column (gap: 1, width: fill, height: fill) {
                        text (content: "Name") style (fg: muted)
                        input (state: self.name, id: "new-migration-name", placeholder: "Migration name...")
                            on_submit: submit()

                        text (content: "Description") style (fg: muted)
                        input (state: self.description, id: "new-migration-desc", placeholder: "Optional description...")

                        text (content: "Source Environment") style (fg: muted)
                        select (state: self.source_env, id: "new-migration-source", placeholder: "Select source...")

                        text (content: "Target Environment") style (fg: muted)
                        select (state: self.target_env, id: "new-migration-target", placeholder: "Select target...")
                    }
                } else {
                    column (width: fill, height: fill, justify: center, align: center) {
                        text (content: "No environments available") style (fg: muted)
                        text (content: "Please configure environments first") style (fg: muted)
                    }
                }

                // Buttons at bottom (Cancel left, Ok right)
                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn") on_activate: cancel()
                    if has_envs {
                        button (label: "Create", id: "create-btn") on_activate: submit()
                    }
                }
            }
        }
    }
}
