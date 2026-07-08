//! Migration list app for viewing and managing migrations.

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

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
use crate::credentials::Environment;
use crate::modals::FileBrowserModal;
use crate::modals::LoadingModal;
use crate::paths;
use crate::systems::client_management::ClientManagement;
use crate::systems::client_management::GetAnyClient;

use super::editor::MigrationEditor;
use super::modals::ImportMigrationModal;
use super::modals::NewMigrationModal;
use super::repository::MigrationExportBundle;
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
        bind("e", export_migration);
        bind("i", import_migration);
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
    async fn export_migration(&self, gx: &GlobalContext) {
        let Some(id) = self.list_state.with_ref(|s| s.focused_key) else {
            return;
        };

        let repo = gx.data::<MigrationRepository>().clone();
        let environments = match self.load_environment_rows(gx).await {
            Some(environments) => environments,
            None => return,
        };

        let export_result = gx
            .modal(LoadingModal::run_with_default(
                "Exporting migration...",
                || Err("Export cancelled".to_string()),
                async move {
                    repo.export_migration(id, &environments)
                        .await
                        .map_err(|e| e.to_string())
                },
            ))
            .await;

        let bundle = match export_result {
            Ok(bundle) => bundle,
            Err(e) => {
                gx.toast(Toast::error(format!("Export failed: {}", e)));
                return;
            }
        };

        let default_filename = format!("{}.migration", sanitize_filename(&bundle.migration.name));
        let start_dir = paths::downloads_dir().unwrap_or_else(|| PathBuf::from("."));
        let Some(file_result) = gx
            .modal(
                FileBrowserModal::browse(&start_dir, vec!["json".to_string()])
                    .with_filename(default_filename),
            )
            .await
        else {
            return;
        };

        let path = next_available_export_path(&normalize_migration_export_path(&file_result.path));
        let write_result = gx
            .modal(LoadingModal::run_with_default(
                "Writing export file...",
                || Err("Export cancelled".to_string()),
                async move { write_export_bundle(path, bundle).await },
            ))
            .await;

        match write_result {
            Ok(path) => gx.toast(Toast::success(format!(
                "Migration exported to {}",
                path.display()
            ))),
            Err(e) => gx.toast(Toast::error(format!("Export failed: {}", e))),
        }
    }

    #[handler]
    async fn import_migration(&self, gx: &GlobalContext) {
        let start_dir = paths::downloads_dir().unwrap_or_else(|| PathBuf::from("."));
        let Some(file_result) = gx
            .modal(
                FileBrowserModal::browse(&start_dir, vec!["json".to_string()]).require_existing(),
            )
            .await
        else {
            return;
        };

        let path = file_result.path;
        let read_result = gx
            .modal(LoadingModal::run_with_default(
                "Reading migration export...",
                || Err("Import cancelled".to_string()),
                async move { read_export_bundle(path).await },
            ))
            .await;

        let bundle = match read_result {
            Ok(bundle) => bundle,
            Err(e) => {
                gx.toast(Toast::error(format!("Import failed: {}", e)));
                return;
            }
        };

        let environments = match self.load_environment_rows(gx).await {
            Some(environments) => environments,
            None => return,
        };

        let Some(result) = gx
            .modal(ImportMigrationModal::with_bundle(
                bundle.clone(),
                environments,
            ))
            .await
        else {
            return;
        };

        let repo = gx.data::<MigrationRepository>().clone();
        let import_result = gx
            .modal(LoadingModal::run_with_default(
                "Importing migration...",
                || Err("Import cancelled".to_string()),
                async move {
                    repo.import_migration(
                        bundle,
                        result.source_environment_id,
                        result.target_environment_id,
                        result.name,
                    )
                    .await
                    .map_err(|e| e.to_string())
                },
            ))
            .await;

        match import_result {
            Ok(_) => {
                gx.toast(Toast::success("Migration imported"));
                self.refresh_list(gx).await;
            }
            Err(e) => gx.toast(Toast::error(format!("Import failed: {}", e))),
        }
    }

    #[handler]
    async fn open_migration(&self, gx: &GlobalContext) {
        let Some(id) = self.list_state.with_ref(|s| s.focused_key) else {
            return;
        };

        let repo = gx.data::<MigrationRepository>();
        let migration = match repo.get_migration(id).await {
            Ok(m) => m,
            Err(e) => {
                log::error!("Failed to load migration: {}", e);
                gx.toast(Toast::error("Failed to load migration"));
                return;
            }
        };

        // Get clients for both environments
        let source_client = match gx
            .request_system::<ClientManagement, GetAnyClient>(GetAnyClient {
                env_id: migration.source_environment_id,
            })
            .await
        {
            Ok(Ok(info)) => info.client,
            Ok(Err(e)) => {
                log::error!("Failed to get source client: {}", e);
                gx.toast(Toast::error("Failed to connect to source environment"));
                return;
            }
            Err(e) => {
                log::error!("Failed to request source client: {:?}", e);
                gx.toast(Toast::error("Failed to connect to source environment"));
                return;
            }
        };

        let target_client = match gx
            .request_system::<ClientManagement, GetAnyClient>(GetAnyClient {
                env_id: migration.target_environment_id,
            })
            .await
        {
            Ok(Ok(info)) => info.client,
            Ok(Err(e)) => {
                log::error!("Failed to get target client: {}", e);
                gx.toast(Toast::error("Failed to connect to target environment"));
                return;
            }
            Err(e) => {
                log::error!("Failed to request target client: {:?}", e);
                gx.toast(Toast::error("Failed to connect to target environment"));
                return;
            }
        };

        let _ = gx.spawn_and_focus(MigrationEditor::new_editor(
            migration,
            source_client,
            target_client,
        ));
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

    async fn load_environment_rows(&self, gx: &GlobalContext) -> Option<Vec<Environment>> {
        let credentials = gx.data::<CredentialsProvider>();
        match credentials.list_environments().await {
            Ok(environments) => Some(environments),
            Err(e) => {
                log::error!("Failed to load environments: {}", e);
                gx.toast(Toast::error("Failed to load environments"));
                None
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
                        button (label: "Import", hint: "i", id: "import-btn") on_activate: import_migration()
                        if has_selection {
                            button (label: "Open", hint: "enter", id: "open-btn") on_activate: open_migration()
                        }
                        if has_selection {
                            button (label: "Export", hint: "e", id: "export-btn") on_activate: export_migration()
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

async fn write_export_bundle(
    path: PathBuf,
    bundle: MigrationExportBundle,
) -> Result<PathBuf, String> {
    tokio::task::spawn_blocking(move || {
        let json = serde_json::to_string_pretty(&bundle).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())?;
        Ok(path)
    })
    .await
    .map_err(|e| e.to_string())?
}

async fn read_export_bundle(path: PathBuf) -> Result<MigrationExportBundle, String> {
    tokio::task::spawn_blocking(move || {
        let json = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&json).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

fn sanitize_filename(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = sanitized.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "migration".to_string()
    } else {
        trimmed
    }
}

fn normalize_migration_export_path(path: &Path) -> PathBuf {
    let path_str = path.to_string_lossy();
    let lower_path = path_str.to_lowercase();
    if lower_path.ends_with(".migration.json") {
        return path.to_path_buf();
    }
    if lower_path.ends_with(".migration") {
        return PathBuf::from(format!("{}.json", path.display()));
    }

    if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("migration");
        return path.with_file_name(format!("{}.migration.json", stem));
    }

    PathBuf::from(format!("{}.migration.json", path.display()))
}

fn next_available_export_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }

    let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("migration.migration.json");
    let stem = file_name
        .strip_suffix(".migration.json")
        .unwrap_or("migration");

    for index in 1.. {
        let candidate = parent.join(format!("{}-{}.migration.json", stem, index));
        if !candidate.exists() {
            return candidate;
        }
    }

    unreachable!()
}
