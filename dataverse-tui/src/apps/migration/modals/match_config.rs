//! Modal for editing match config (Same ID / Find mode).
//!
//! Uses page routing for mode selection:
//! - Same ID tab: informational — records matched by identical GUIDs
//! - Find tab: informational — conditions managed in the tree

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::Text;
use tuidom::Element;

use crate::apps::migration::types::MatchStrategy;

/// Page enum — each page represents a match strategy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    SameId,
    Find,
}

/// Modal for editing match config.
#[modal(size = Sm, pages)]
pub struct MatchConfigModal {
    /// Validation error.
    error: Option<String>,
}

impl MatchConfigModal {
    /// Create a modal for editing match config.
    pub fn new_modal(current_strategy: MatchStrategy) -> Self {
        let mut modal = Self::new(None);
        if current_strategy == MatchStrategy::Find {
            modal.navigate(Page::Find);
        }
        modal
    }
}

#[modal_impl(layout = layout)]
impl MatchConfigModal {
    fn default_result(&self) -> Option<MatchStrategy> {
        None
    }

    #[on_start]
    async fn on_start(&self, _mx: &ModalContext<Option<MatchStrategy>>) {}

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
        bind("ctrl+s", save);
        bind("1", tab_same_id);
        bind("2", tab_find);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<MatchStrategy>>) {
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
    async fn save(&self, mx: &ModalContext<Option<MatchStrategy>>) {
        let strategy = match self.page() {
            Page::SameId => MatchStrategy::SameId,
            Page::Find => MatchStrategy::Find,
        };
        mx.close(Some(strategy));
    }

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
}
