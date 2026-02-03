//! Migration list app for viewing and managing migrations.

use std::collections::HashMap;

use chrono::DateTime;
use chrono::Utc;
use rafter::element;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::List;
use rafter::widgets::ListItem;
use rafter::widgets::ListState;
use rafter::widgets::Text;
use tuidom::Element;

use crate::credentials::CredentialsProvider;

use super::editor::MigrationEditor;
use super::modals::NewMigrationModal;
use super::repository::MigrationRepository;
use super::repository::NewMigration;
use super::types::MigrationSummary;

/// List item for displaying a migration.
#[derive(Debug, Clone)]
pub struct MigrationListItem {
    pub id: i64,
    pub name: String,
    pub source_env_name: String,
    pub target_env_name: String,
    pub updated_at: DateTime<Utc>,
}

impl MigrationListItem {
    fn from_summary(m: MigrationSummary, env_names: &HashMap<i64, String>) -> Self {
        let source_env_name = env_names
            .get(&m.source_environment_id)
            .cloned()
            .unwrap_or_else(|| format!("#{}", m.source_environment_id));
        let target_env_name = env_names
            .get(&m.target_environment_id)
            .cloned()
            .unwrap_or_else(|| format!("#{}", m.target_environment_id));

        Self {
            id: m.id,
            name: m.name,
            source_env_name,
            target_env_name,
            updated_at: m.updated_at,
        }
    }
}

impl ListItem for MigrationListItem {
    type Key = i64;

    fn key(&self) -> i64 {
        self.id
    }

    fn render(&self) -> Element {
        let updated = self.updated_at.format("%Y-%m-%d %H:%M").to_string();
        element! {
            row (width: fill, gap: 2, justify: between) {
                row (gap: 2) {
                    text (content: {self.name.clone()}) style (fg: primary)
                    text (content: {self.source_env_name.clone()}) style (fg: muted)
                    text (content: "->") style (fg: muted)
                    text (content: {self.target_env_name.clone()}) style (fg: muted)
                }
                text (content: {updated}) style (fg: muted)
            }
        }
    }
}

/// Migration list app.
#[app(name = "Migrations", on_blur = Close)]
pub struct MigrationList {
    list_state: ListState<MigrationListItem>,
    /// Cached environment id -> name mapping.
    env_names: HashMap<i64, String>,
    /// Cached environment options for modal (id, name).
    env_options: Vec<(i64, String)>,
}

impl MigrationList {
    /// Create a new migration list app with default state.
    pub fn create() -> Self {
        Self::new(ListState::default(), HashMap::new(), Vec::new())
    }
}

#[app_impl]
impl MigrationList {
    #[on_start]
    async fn on_start(&self, gx: &GlobalContext) {
        self.load_environments(gx).await;
        self.refresh_list(gx).await;
    }

    fn title(&self) -> String {
        let count = self.list_state.with_ref(|s| s.items.len());
        if count > 0 {
            format!("Migrations ({})", count)
        } else {
            "Migrations".to_string()
        }
    }

    // =========================================================================
    // Derived State
    // =========================================================================

    #[derived]
    fn has_selection(&self) -> bool {
        self.list_state.with_ref(|s| s.focused_key.is_some())
    }

    // =========================================================================
    // Keybinds
    // =========================================================================

    #[keybinds]
    fn keybinds() {
        bind("escape", close_app);
        bind("n", new_migration);
        bind("d", delete_migration);
    }

    #[handler]
    async fn close_app(&self, cx: &AppContext) {
        cx.close();
    }

    #[handler]
    async fn new_migration(&self, gx: &GlobalContext) {
        let env_options = self.env_options.get();
        if env_options.is_empty() {
            gx.toast(Toast::error("No environments configured"));
            return;
        }

        let Some(result) = gx
            .modal(NewMigrationModal::with_environments(env_options))
            .await
        else {
            return;
        };

        let repo = gx.data::<MigrationRepository>();
        let new_migration = NewMigration {
            name: result.name,
            description: result.description,
            source_environment_id: result.source_environment_id,
            target_environment_id: result.target_environment_id,
        };

        match repo.create_migration(new_migration).await {
            Ok(_id) => {
                gx.toast(Toast::info("Migration created"));
                self.refresh_list(gx).await;
            }
            Err(e) => {
                log::error!("Failed to create migration: {}", e);
                gx.toast(Toast::error("Failed to create migration"));
            }
        }
    }

    #[handler]
    async fn delete_migration(&self, gx: &GlobalContext) {
        let Some(id) = self.list_state.with_ref(|s| s.focused_key) else {
            return;
        };

        let confirmed = gx
            .modal(crate::modals::ConfirmModal::with_message(
                "Delete this migration?",
            ))
            .await;

        if !confirmed {
            return;
        }

        let repo = gx.data::<MigrationRepository>();
        match repo.delete_migration(id).await {
            Ok(()) => {
                gx.toast(Toast::info("Migration deleted"));
                self.refresh_list(gx).await;
            }
            Err(e) => {
                log::error!("Failed to delete migration: {}", e);
                gx.toast(Toast::error("Failed to delete migration"));
            }
        }
    }

    #[handler]
    async fn open_migration(&self, gx: &GlobalContext) {
        let Some(id) = self.list_state.with_ref(|s| s.focused_key) else {
            return;
        };

        let repo = gx.data::<MigrationRepository>();
        match repo.get_migration(id).await {
            Ok(migration) => {
                let _ = gx.spawn_and_focus(MigrationEditor::for_migration(migration));
            }
            Err(e) => {
                log::error!("Failed to load migration: {}", e);
                gx.toast(Toast::error("Failed to load migration"));
            }
        }
    }



    // =========================================================================
    // Internal
    // =========================================================================

    async fn load_environments(&self, gx: &GlobalContext) {
        let credentials = gx.data::<CredentialsProvider>();
        match credentials.list_environments().await {
            Ok(envs) => {
                let env_names: HashMap<i64, String> = envs
                    .iter()
                    .map(|e| (e.id, e.display_name.clone()))
                    .collect();
                let env_options: Vec<(i64, String)> = envs
                    .iter()
                    .map(|e| (e.id, e.display_name.clone()))
                    .collect();

                self.env_names.set(env_names);
                self.env_options.set(env_options);
            }
            Err(e) => {
                log::error!("Failed to load environments: {}", e);
            }
        }
    }

    async fn refresh_list(&self, gx: &GlobalContext) {
        let repo = gx.data::<MigrationRepository>();
        let env_names = self.env_names.get();

        match repo.list_migrations().await {
            Ok(migrations) => {
                let items: Vec<MigrationListItem> = migrations
                    .into_iter()
                    .map(|m| MigrationListItem::from_summary(m, &env_names))
                    .collect();
                self.list_state.set(ListState::new(items));
            }
            Err(e) => {
                log::error!("Failed to load migrations: {}", e);
                gx.toast(Toast::error("Failed to load migrations"));
            }
        }
    }

    // =========================================================================
    // UI
    // =========================================================================

    fn element(&self) -> Element {
        let is_empty = self.list_state.with_ref(|s| s.items.is_empty());
        let has_selection = self.has_selection();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: background) {
                // Header
                text (content: "Migrations") style (bold, fg: interact)

                // List
                if is_empty {
                    column (width: fill, height: fill, justify: center, align: center) {
                        text (content: "No migrations") style (fg: muted)
                        text (content: "Press n to create one") style (fg: muted)
                    }
                } else {
                    box_ (id: "migration-list-container", height: fill, width: fill) style (bg: surface) {
                        list (state: self.list_state, id: "migration-list", width: fill, height: fill)
                            on_activate: open_migration()
                    }
                }

                // Footer
                row (width: fill, justify: between) {
                    button (label: "Close", hint: "esc", id: "close-btn") on_activate: close_app()
                    row (gap: 1) {
                        button (label: "New", hint: "n", id: "new-btn") on_activate: new_migration()
                        if has_selection {
                            button (label: "Open", hint: "enter", id: "open-btn") on_activate: open_migration()
                        }
                        if has_selection {
                            button (label: "Delete", hint: "d", id: "delete-btn") on_activate: delete_migration()
                        }
                    }
                }
            }
        }
    }
}
