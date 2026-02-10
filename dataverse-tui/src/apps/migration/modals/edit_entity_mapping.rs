//! Modal for creating/editing an entity mapping with tabbed interface.
//!
//! Uses page routing for mode selection:
//! - Declarative tab: name, source entity, target entity
//! - Lua tab: name only + script import/export

use std::fs;
use std::path::PathBuf;

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Autocomplete;
use rafter::widgets::AutocompleteState;
use rafter::widgets::Button;
use rafter::widgets::Input;
use rafter::widgets::Text;

use crate::apps::migration::repository::MigrationRepository;
use crate::apps::migration::types::EntityMapping;
use crate::apps::migration::types::Mode;
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

/// Result of the entity mapping modal.
#[derive(Debug, Clone)]
pub enum EntityMappingResult {
    /// Declarative mode with source and target entities.
    Declarative {
        name: String,
        source_entity: String,
        target_entity: String,
    },
    /// Lua mode with a script.
    Lua { name: String, lua_script: String },
}

/// Modal for creating/editing an entity mapping.
#[modal(size = Md, pages)]
pub struct EditEntityMappingModal {
    #[state(skip)]
    entity_mapping_id: i64,
    #[state(skip)]
    initial_mode: Mode,
    name: String,
    source_entity: AutocompleteState<String>,
    target_entity: AutocompleteState<String>,
    lua_script: Option<String>,
    error: Option<String>,
}

impl EditEntityMappingModal {
    /// Create a modal for a new entity mapping.
    pub fn new_mapping(source_entities: Vec<String>, target_entities: Vec<String>) -> Self {
        let source_options: Vec<_> = source_entities
            .into_iter()
            .map(|name| (name.clone(), name))
            .collect();
        let target_options: Vec<_> = target_entities
            .into_iter()
            .map(|name| (name.clone(), name))
            .collect();

        Self::new(
            0,
            Mode::Declarative,
            String::new(),
            AutocompleteState::new(source_options),
            AutocompleteState::new(target_options),
            None,
            None,
        )
    }

    /// Create a modal for editing an existing entity mapping.
    pub fn edit_mapping(
        em: &EntityMapping,
        source_entities: Vec<String>,
        target_entities: Vec<String>,
    ) -> Self {
        let source_options: Vec<_> = source_entities
            .into_iter()
            .map(|name| (name.clone(), name))
            .collect();
        let target_options: Vec<_> = target_entities
            .into_iter()
            .map(|name| (name.clone(), name))
            .collect();

        let source_state =
            AutocompleteState::new(source_options).with_value(em.source_entity.clone());

        let target_state =
            AutocompleteState::new(target_options).with_value(em.target_entity.clone());

        Self::new(
            em.id,
            em.mode,
            em.name.clone(),
            source_state,
            target_state,
            em.lua_script.clone(),
            None,
        )
    }
}

#[modal_impl(layout = layout)]
impl EditEntityMappingModal {
    fn default_result(&self) -> Option<EntityMappingResult> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<EntityMappingResult>>) {
        // Navigate to correct tab based on initial mode
        if self.initial_mode == Mode::Lua {
            self.navigate(Page::Lua);
        }
        mx.focus("mapping-name");
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
    async fn cancel(&self, mx: &ModalContext<Option<EntityMappingResult>>) {
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
                .modal(ConfirmModal::with_message(
                    "Switching to Declarative will clear the Lua script. Continue?",
                ))
                .await;
            if !confirmed {
                return;
            }
            self.lua_script.set(None);
        }

        // Check if config nodes exist (only if editing)
        if self.entity_mapping_id != 0 {
            let repo = gx.data::<MigrationRepository>();
            // TODO: Check if any child config exists and confirm deletion
            // For now, just navigate
        }

        self.navigate(Page::Declarative);
    }

    #[handler]
    async fn tab_lua(&self, gx: &GlobalContext) {
        if self.page() == Page::Lua {
            return;
        }

        // Switching from Declarative to Lua - confirm if config exists
        if self.entity_mapping_id != 0 {
            let repo = gx.data::<MigrationRepository>();
            // TODO: Check child config nodes and confirm deletion
            // For now, just show generic confirmation
            let confirmed = gx
                .modal(ConfirmModal::with_message(
                    "Switching to Lua will clear all declarative configuration. Continue?",
                ))
                .await;
            if !confirmed {
                return;
            }
        }

        self.navigate(Page::Lua);
    }

    #[handler]
    async fn submit(&self, mx: &ModalContext<Option<EntityMappingResult>>) {
        let name = self.name.get().trim().to_string();
        if name.is_empty() {
            self.error.set(Some("Name is required".to_string()));
            return;
        }

        let result = match self.page() {
            Page::Declarative => {
                let source = self.source_entity.with_ref(|s| s.value().cloned());
                let target = self.target_entity.with_ref(|s| s.value().cloned());

                let (Some(source_entity), Some(target_entity)) = (source, target) else {
                    self.error.set(Some(
                        "Please select both source and target entities".to_string(),
                    ));
                    return;
                };

                EntityMappingResult::Declarative {
                    name,
                    source_entity,
                    target_entity,
                }
            }
            Page::Lua => {
                let Some(lua_script) = self.lua_script.get() else {
                    self.error
                        .set(Some("Lua mode requires a script".to_string()));
                    return;
                };
                EntityMappingResult::Lua { name, lua_script }
            }
        };

        mx.close(Some(result));
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

        let mapping_name = self.name.get();
        let start_dir = paths::downloads_dir().unwrap_or_else(|| PathBuf::from("."));
        let Some(result) = gx
            .modal(
                FileBrowserModal::browse(&start_dir, vec!["lua".to_string()])
                    .with_filename(&mapping_name),
            )
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
        let is_new = self.entity_mapping_id == 0;
        let title = if is_new {
            "New Entity Mapping"
        } else {
            "Edit Entity Mapping"
        };

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: {title}) style (bold, fg: interact)

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
                    input (state: self.name, id: "mapping-name", label: "Name", placeholder: "e.g., Account Sync")
                        on_submit: submit()

                    text (content: "Source Entity") style (fg: muted)
                    autocomplete (state: self.source_entity, id: "source-entity", placeholder: "Select source entity...")

                    text (content: "Target Entity") style (fg: muted)
                    autocomplete (state: self.target_entity, id: "target-entity", placeholder: "Select target entity...")
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
                input (state: self.name, id: "mapping-name", label: "Name", placeholder: "e.g., Custom Mapping")
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
