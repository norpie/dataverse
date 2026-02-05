//! Modal for editing a phase with tabbed interface.
//!
//! Uses page routing for mode selection:
//! - Declarative tab: name settings, declarative mode
//! - Lua tab: name settings + script import/export, lua mode

use std::fs;
use std::path::PathBuf;

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::Input;
use rafter::widgets::Text;

use crate::apps::migration::repository::MigrationRepository;
use crate::apps::migration::repository::Update;
use crate::apps::migration::types::Mode;
use crate::apps::migration::types::Phase;
use crate::modals::ConfirmModal;
use crate::modals::FileBrowserModal;
use crate::paths;

/// Page enum - each page represents a mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Declarative,
    Lua,
}

/// Result of the edit phase modal.
#[derive(Debug, Clone)]
pub struct EditPhaseResult {
    pub name: String,
    pub mode: Mode,
    pub lua_script: Update<String>,
}

/// Modal for editing a phase.
#[modal(size = Md, pages)]
pub struct EditPhaseModal {
    #[state(skip)]
    phase_id: i64,
    #[state(skip)]
    initial_mode: Mode,
    name: String,
    lua_script: Option<String>,
    error: Option<String>,
}

impl EditPhaseModal {
    /// Create an edit phase modal for the given phase.
    pub fn for_phase(phase: &Phase) -> Self {
        Self::new(
            phase.id,
            phase.mode,
            phase.name.clone(),
            phase.lua_script.clone(),
            None,
        )
    }
}

#[modal_impl(layout = layout)]
impl EditPhaseModal {
    fn default_result(&self) -> Option<EditPhaseResult> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<EditPhaseResult>>) {
        // Navigate to correct tab based on initial mode
        if self.initial_mode == Mode::Lua {
            self.navigate(Page::Lua);
        }
        mx.focus("edit-phase-name");
    }

    // =========================================================================
    // Keybinds
    // =========================================================================

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
        bind("1", tab_declarative);
        bind("2", tab_lua);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<EditPhaseResult>>) {
        mx.close(None);
    }

    #[handler]
    async fn tab_declarative(&self, gx: &GlobalContext) {
        if self.page() == Page::Declarative {
            return;
        }

        // Switching from Lua to Declarative - confirm if script exists
        if self.lua_script.get().is_some() {
            let confirmed = gx
                .modal(ConfirmModal::with_message("Switching to Declarative will clear the Lua script. Continue?"))
                .await;
            if !confirmed {
                return;
            }
            self.lua_script.set(None);
        }

        self.navigate(Page::Declarative);
    }

    #[handler]
    async fn tab_lua(&self, gx: &GlobalContext) {
        if self.page() == Page::Lua {
            return;
        }

        // Switching from Declarative to Lua - confirm if entity mappings exist
        let repo = gx.data::<MigrationRepository>();
        let entity_count = repo
            .get_entity_mappings(self.phase_id)
            .await
            .map(|v| v.len())
            .unwrap_or(0);

        if entity_count > 0 {
            let confirmed = gx
                .modal(ConfirmModal::with_message(format!(
                    "Switching to Lua will delete {} entity mapping(s). Continue?",
                    entity_count
                )))
                .await;
            if !confirmed {
                return;
            }

            // Delete all entity mappings for this phase
            if let Err(e) = repo.delete_entity_mappings_for_phase(self.phase_id).await {
                log::error!("Failed to delete entity mappings: {}", e);
                gx.toast(Toast::error("Failed to delete entity mappings"));
                return;
            }
        }

        self.navigate(Page::Lua);
    }

    #[handler]
    async fn submit(&self, mx: &ModalContext<Option<EditPhaseResult>>) {
        let name = self.name.get().trim().to_string();
        if name.is_empty() {
            self.error.set(Some("Name is required".to_string()));
            return;
        }

        let mode = match self.page() {
            Page::Declarative => Mode::Declarative,
            Page::Lua => Mode::Lua,
        };

        // Lua mode requires a script
        if mode == Mode::Lua && self.lua_script.get().is_none() {
            self.error.set(Some("Lua mode requires a script".to_string()));
            return;
        }

        let lua_script = match (mode, self.lua_script.get()) {
            (Mode::Declarative, _) => Update::Clear,
            (Mode::Lua, Some(script)) => Update::Set(script),
            (Mode::Lua, None) => unreachable!(), // validated above
        };

        mx.close(Some(EditPhaseResult {
            name,
            mode,
            lua_script,
        }));
    }

    // =========================================================================
    // Script handlers
    // =========================================================================

    #[handler]
    async fn import_script(&self, gx: &GlobalContext) {
        let start_dir = paths::downloads_dir().unwrap_or_else(|| PathBuf::from("."));
        let Some(result) = gx
            .modal(FileBrowserModal::browse(&start_dir, vec!["lua".to_string()]).require_existing())
            .await
        else {
            return;
        };

        match fs::read_to_string(&result.path) {
            Ok(content) => {
                self.lua_script.set(Some(content));
                gx.toast(Toast::info("Script imported"));
            }
            Err(e) => {
                log::error!("Failed to read script file: {}", e);
                gx.toast(Toast::error("Failed to read script file"));
            }
        }
    }

    #[handler]
    async fn export_script(&self, gx: &GlobalContext) {
        let Some(script) = self.lua_script.get() else {
            gx.toast(Toast::warning("No script to export"));
            return;
        };

        let phase_name = self.name.get();
        let start_dir = paths::downloads_dir().unwrap_or_else(|| PathBuf::from("."));
        let Some(result) = gx
            .modal(FileBrowserModal::browse(&start_dir, vec!["lua".to_string()]).with_filename(&phase_name))
            .await
        else {
            return;
        };

        match fs::write(&result.path, script) {
            Ok(()) => {
                gx.toast(Toast::info("Script exported"));
            }
            Err(e) => {
                log::error!("Failed to write script file: {}", e);
                gx.toast(Toast::error("Failed to export script"));
            }
        }
    }

    #[handler]
    async fn clear_script(&self, gx: &GlobalContext) {
        self.lua_script.set(None);
        gx.toast(Toast::info("Script cleared"));
    }

    // =========================================================================
    // Layout and Pages
    // =========================================================================

    fn layout(&self, content: Element) -> Element {
        let current = self.page();
        let error = self.error.get();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Edit Phase") style (bold, fg: interact)

                if let Some(err) = error {
                    text (content: {err}) style (fg: error)
                }

                row (gap: 2) {
                    button (label: "Declarative", hint: "1", id: "tab-declarative")
                        style (fg: if current == Page::Declarative { interact } else { muted })
                        on_activate: tab_declarative()
                    button (label: "Lua", hint: "2", id: "tab-lua")
                        style (fg: if current == Page::Lua { interact } else { muted })
                        on_activate: tab_lua()
                }

                { content }
            }
        }
    }

    #[page(Declarative)]
    fn declarative_page(&self) -> Element {
        page! {
            column (width: fill, height: fill) {
                column (width: fill, height: fill) {
                    input (state: self.name, id: "edit-phase-name", label: "Name")
                        on_submit: submit()
                }

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn") on_activate: cancel()
                    button (label: "Save", id: "save-btn") on_activate: submit()
                }
            }
        }
    }

    #[page(Lua)]
    fn lua_page(&self) -> Element {
        let has_script = self.lua_script.get().is_some();

        page! {
            column (width: fill, height: fill) {
                input (state: self.name, id: "edit-phase-name", label: "Name")
                    on_submit: submit()

                column (width: fill, height: fill, justify: center, align: center) {
                    if has_script {
                        text (content: "Script loaded") style (fg: success)
                    }
                    if !has_script {
                        text (content: "No script loaded") style (fg: muted)
                    }
                }

                row (width: fill, justify: between) {
                    row (gap: 1) {
                        button (label: "Import", id: "import-btn") on_activate: import_script()
                        if has_script {
                            button (label: "Export", id: "export-btn") on_activate: export_script()
                        }
                        if has_script {
                            button (label: "Clear", id: "clear-btn") on_activate: clear_script()
                        }
                    }
                    row (gap: 1) {
                        button (label: "Cancel", hint: "esc", id: "cancel-btn") on_activate: cancel()
                        button (label: "Save", id: "save-btn") on_activate: submit()
                    }
                }
            }
        }
    }
}
