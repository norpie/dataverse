//! Modal for creating/editing a Find transform.
//!
//! Uses page routing for mode selection:
//! - Where tab: entity autocomplete + fallback select (declarative conditions managed in tree)
//! - Lua tab: entity autocomplete + fallback select + script import/export

use std::fs;
use std::path::PathBuf;

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::AutocompleteState;
use rafter::widgets::Button;
use rafter::widgets::Select;
use rafter::widgets::SelectState;
use rafter::widgets::Text;
use tuidom::Element;

use crate::apps::migration::types::FindFallback;
use crate::apps::migration::types::FindMode;
use crate::modals::FileBrowserModal;
use crate::paths;

/// Page enum — each page represents a mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Where,
    Lua,
}

/// Fallback kind for the select widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
enum FallbackKind {
    Error,
    #[default]
    Null,
    Default,
}

impl FallbackKind {
    fn all() -> Vec<(FallbackKind, String)> {
        vec![
            (FallbackKind::Error, "Error — fail the record".to_string()),
            (FallbackKind::Null, "Null — use null value".to_string()),
            (
                FallbackKind::Default,
                "Default — execute fallback chain".to_string(),
            ),
        ]
    }

    fn from_fallback(fb: &FindFallback) -> Self {
        match fb {
            FindFallback::Error => FallbackKind::Error,
            FindFallback::Null => FallbackKind::Null,
            FindFallback::Default => FallbackKind::Default,
        }
    }

    fn to_fallback(self) -> FindFallback {
        match self {
            FallbackKind::Error => FindFallback::Error,
            FallbackKind::Null => FindFallback::Null,
            FallbackKind::Default => FindFallback::Default,
        }
    }
}

impl ToString for FallbackKind {
    fn to_string(&self) -> String {
        match self {
            FallbackKind::Error => "Error".to_string(),
            FallbackKind::Null => "Null".to_string(),
            FallbackKind::Default => "Default".to_string(),
        }
    }
}

/// Result of the find transform modal.
#[derive(Debug, Clone)]
pub struct FindTransformResult {
    pub entity: String,
    pub fallback: FindFallback,
    pub mode: FindMode,
}

/// Modal for creating/editing a Find transform.
#[modal(size = Md, pages)]
pub struct FindTransformModal {
    /// Target entity autocomplete.
    entity: AutocompleteState<String>,
    /// Fallback selector.
    fallback_select: SelectState<FallbackKind>,
    /// Lua script content (only used in Lua mode).
    lua_script: Option<String>,
    /// Validation error.
    error: Option<String>,
}

impl FindTransformModal {
    /// Create a new Find transform modal for creation.
    pub fn new_modal(target_entities: Vec<String>) -> Self {
        let entity_options: Vec<_> = target_entities
            .into_iter()
            .map(|name| (name.clone(), name))
            .collect();

        Self::new(
            AutocompleteState::new(entity_options),
            SelectState::new(FallbackKind::all()).with_value(FallbackKind::Null),
            None,
            None,
        )
    }

    /// Create a Find transform modal for editing an existing transform.
    pub fn edit_modal(
        target_entities: Vec<String>,
        entity: &str,
        fallback: &FindFallback,
        mode: &FindMode,
    ) -> Self {
        let entity_options: Vec<_> = target_entities
            .into_iter()
            .map(|name| (name.clone(), name))
            .collect();

        let entity_state =
            AutocompleteState::new(entity_options).with_value(entity.to_string());
        let fallback_kind = FallbackKind::from_fallback(fallback);
        let fallback_select = SelectState::new(FallbackKind::all()).with_value(fallback_kind);

        let lua_script = match mode {
            FindMode::Lua { script } => Some(script.clone()),
            FindMode::Where => None,
        };

        Self::new(entity_state, fallback_select, lua_script, None)
    }

    fn selected_fallback(&self) -> FallbackKind {
        self.fallback_select
            .get()
            .value()
            .copied()
            .unwrap_or_default()
    }
}

#[modal_impl(layout = layout)]
impl FindTransformModal {
    fn default_result(&self) -> Option<FindTransformResult> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<FindTransformResult>>) {
        mx.focus("entity-autocomplete");
    }

    // =========================================================================
    // Keybinds
    // =========================================================================

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
        bind("ctrl+s", save);
        bind("1", tab_where);
        bind("2", tab_lua);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<FindTransformResult>>) {
        mx.close(None);
    }

    #[handler]
    async fn tab_where(&self, _gx: &GlobalContext) {
        if self.page() == Page::Where {
            return;
        }
        self.navigate(Page::Where);
    }

    #[handler]
    async fn tab_lua(&self, _gx: &GlobalContext) {
        if self.page() == Page::Lua {
            return;
        }
        self.navigate(Page::Lua);
    }

    #[handler]
    async fn save(&self, mx: &ModalContext<Option<FindTransformResult>>) {
        let entity = self.entity.with_ref(|s| s.value().cloned());
        let Some(entity) = entity else {
            self.error
                .set(Some("Please select a target entity".to_string()));
            return;
        };

        if entity.trim().is_empty() {
            self.error
                .set(Some("Please select a target entity".to_string()));
            return;
        }

        let fallback = self.selected_fallback().to_fallback();

        let mode = match self.page() {
            Page::Where => FindMode::Where,
            Page::Lua => {
                let Some(script) = self.lua_script.get() else {
                    self.error
                        .set(Some("Lua mode requires a script".to_string()));
                    return;
                };
                if script.trim().is_empty() {
                    self.error
                        .set(Some("Lua mode requires a script".to_string()));
                    return;
                }
                FindMode::Lua { script }
            }
        };

        mx.close(Some(FindTransformResult {
            entity,
            fallback,
            mode,
        }));
    }

    // =========================================================================
    // Script handlers
    // =========================================================================

    #[handler]
    async fn import_script(&self, gx: &GlobalContext) {
        let start_dir = paths::downloads_dir().unwrap_or_else(|| PathBuf::from("."));
        let Some(result) = gx
            .modal(
                FileBrowserModal::browse(&start_dir, vec!["lua".to_string()]).require_existing(),
            )
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

        let entity_name = self
            .entity
            .with_ref(|s| s.value().cloned())
            .unwrap_or_default();
        let filename = if entity_name.is_empty() {
            "find".to_string()
        } else {
            format!("find_{}", entity_name)
        };
        let start_dir = paths::downloads_dir().unwrap_or_else(|| PathBuf::from("."));
        let Some(result) = gx
            .modal(
                FileBrowserModal::browse(&start_dir, vec!["lua".to_string()])
                    .with_filename(&filename),
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

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Find Transform") style (bold, fg: interact)

                if let Some(err) = error {
                    text (content: {&err}) style (fg: error)
                }

                row (gap: 2) {
                    button (label: "Where", hint: "1", id: "tab-where")
                        style (fg: if current == Page::Where { interact } else { muted })
                        on_activate: tab_where()
                    button (label: "Lua", hint: "2", id: "tab-lua")
                        style (fg: if current == Page::Lua { interact } else { muted })
                        on_activate: tab_lua()
                }

                { content }
            }
        }
    }

    #[page(Where)]
    fn where_page(&self) -> Element {
        page! {
            column (width: fill, height: fill) {
                column (width: fill, height: fill) {
                    text (content: "Target Entity") style (fg: muted)
                    autocomplete (state: self.entity, id: "entity-autocomplete", placeholder: "Select entity to search in...")

                    text (content: "Fallback") style (fg: muted)
                    select (state: self.fallback_select, id: "fallback-select", width: fill)

                    text (content: "Conditions are managed in the tree after creation. Use 'a' on the find node to add conditions.") style (fg: muted)
                }

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn") on_activate: cancel()
                    button (label: "Save", hint: "ctrl+s", id: "save-btn") on_activate: save()
                }
            }
        }
    }

    #[page(Lua)]
    fn lua_page(&self) -> Element {
        let has_script = self.lua_script.get().is_some();

        page! {
            column (width: fill, height: fill) {
                column (width: fill, height: fill) {
                    text (content: "Target Entity") style (fg: muted)
                    autocomplete (state: self.entity, id: "entity-autocomplete", placeholder: "Select entity to search in...")

                    text (content: "Fallback") style (fg: muted)
                    select (state: self.fallback_select, id: "fallback-select", width: fill)

                    column (width: fill, height: fill, justify: center, align: center) {
                        if has_script {
                            text (content: "Script loaded") style (fg: success)
                        }
                        if !has_script {
                            text (content: "No script loaded") style (fg: muted)
                        }
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
                        button (label: "Save", hint: "ctrl+s", id: "save-btn") on_activate: save()
                    }
                }
            }
        }
    }
}
