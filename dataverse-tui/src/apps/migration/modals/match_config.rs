//! Modal for editing match config (Same ID / Find / Lua mode).
//!
//! Uses page routing for mode selection:
//! - Same ID tab: informational — records matched by identical GUIDs
//! - Find tab: informational — conditions managed in the tree
//! - Lua tab: script import/export for custom matching logic

use std::fs;
use std::path::PathBuf;

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::Text;
use tuidom::Element;

use crate::apps::migration::types::MatchStrategy;
use crate::modals::FileBrowserModal;
use crate::paths;

/// Result returned by the match config modal.
pub struct MatchConfigResult {
    pub strategy: MatchStrategy,
    pub lua_script: Option<String>,
}

/// Page enum — each page represents a match strategy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    SameId,
    Find,
    Lua,
}

/// Modal for editing match config.
#[modal(size = Sm, pages)]
pub struct MatchConfigModal {
    /// Validation error.
    error: Option<String>,
    /// Lua script content.
    lua_script: Option<String>,
}

impl MatchConfigModal {
    /// Create a modal for editing match config.
    pub fn new_modal(current_strategy: MatchStrategy, lua_script: Option<String>) -> Self {
        let modal = Self::new(None, lua_script);
        match current_strategy {
            MatchStrategy::Find => modal.navigate(Page::Find),
            MatchStrategy::Lua => modal.navigate(Page::Lua),
            MatchStrategy::SameId => {}
        }
        modal
    }
}

#[modal_impl(layout = layout)]
impl MatchConfigModal {
    fn default_result(&self) -> Option<MatchConfigResult> {
        None
    }

    #[on_start]
    async fn on_start(&self, _mx: &ModalContext<Option<MatchConfigResult>>) {}

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
        bind("ctrl+s", save);
        bind("1", tab_same_id);
        bind("2", tab_find);
        bind("3", tab_lua);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<MatchConfigResult>>) {
        mx.close(None);
    }

    #[handler]
    async fn tab_same_id(&self, _gx: &GlobalContext) {
        if self.page() == Page::SameId {
            return;
        }
        self.navigate(Page::SameId);
    }

    #[handler]
    async fn tab_find(&self, _gx: &GlobalContext) {
        if self.page() == Page::Find {
            return;
        }
        self.navigate(Page::Find);
    }

    #[handler]
    async fn tab_lua(&self, _gx: &GlobalContext) {
        if self.page() == Page::Lua {
            return;
        }
        self.navigate(Page::Lua);
    }

    #[handler]
    async fn save(&self, mx: &ModalContext<Option<MatchConfigResult>>) {
        let (strategy, lua_script) = match self.page() {
            Page::SameId => (MatchStrategy::SameId, None),
            Page::Find => (MatchStrategy::Find, None),
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
                (MatchStrategy::Lua, Some(script))
            }
        };
        mx.close(Some(MatchConfigResult {
            strategy,
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

        let start_dir = paths::downloads_dir().unwrap_or_else(|| PathBuf::from("."));
        let Some(result) = gx
            .modal(
                FileBrowserModal::browse(&start_dir, vec!["lua".to_string()])
                    .with_filename("match_config"),
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
                text (content: "Match Config") style (bold, fg: interact)

                if let Some(err) = error {
                    text (content: {&err}) style (fg: error)
                }

                row (gap: 2) {
                    button (label: "Same ID", hint: "1", id: "tab-same-id")
                        style (fg: if current == Page::SameId { interact } else { muted })
                        on_activate: tab_same_id()
                    button (label: "Find", hint: "2", id: "tab-find")
                        style (fg: if current == Page::Find { interact } else { muted })
                        on_activate: tab_find()
                    button (label: "Lua", hint: "3", id: "tab-lua")
                        style (fg: if current == Page::Lua { interact } else { muted })
                        on_activate: tab_lua()
                }

                { content }
            }
        }
    }

    #[page(SameId)]
    fn same_id_page(&self) -> Element {
        page! {
            column (width: fill, height: fill) {
                column (width: fill, height: fill) {
                    text (content: "Source and target records are matched by identical GUIDs.") style (fg: muted)
                    text (content: "Use this when both environments share the same record IDs.") style (fg: muted)
                }

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn") on_activate: cancel()
                    button (label: "Save", hint: "ctrl+s", id: "save-btn") on_activate: save()
                }
            }
        }
    }

    #[page(Find)]
    fn find_page(&self) -> Element {
        page! {
            column (width: fill, height: fill) {
                column (width: fill, height: fill) {
                    text (content: "Match source records to target records using conditions.") style (fg: muted)
                    text (content: "After saving, use 'a' on the Match Config node to add conditions.") style (fg: muted)
                    text (content: "Each condition specifies a target field and a transform chain to compute the match value from the source record.") style (fg: muted)
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
                column (width: fill, height: fill, justify: center, align: center) {
                    text (content: "Match source records to target records using a Lua script.") style (fg: muted)
                    text (content: "The script receives all source and target records and returns a mapping.") style (fg: muted)

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
                        button (label: "Save", hint: "ctrl+s", id: "save-btn") on_activate: save()
                    }
                }
            }
        }
    }
}
