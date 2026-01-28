//! Load query modal for selecting a saved query.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Input, List, ListItem, ListState, Text};
use tuidom::Element;

/// A saved query item for the list.
#[derive(Debug, Clone)]
pub struct QueryItem {
    pub id: i64,
    pub label: String,
}

impl ListItem for QueryItem {
    type Key = i64;

    fn key(&self) -> i64 {
        self.id
    }

    fn render(&self) -> Element {
        Element::text(&self.label)
    }
}

/// Modal for selecting a saved query to load or delete.
/// Returns (query_to_load, queries_to_delete).
#[modal(default, size = Md)]
pub struct LoadQueryModal {
    #[state(skip)]
    items: Vec<QueryItem>,

    input: String,
    list: ListState<QueryItem>,
    staged_deletes: Vec<i64>,
}

impl LoadQueryModal {
    /// Create with pre-fetched query list: (id, display_label).
    pub fn new(options: Vec<(i64, String)>) -> Self {
        let items: Vec<QueryItem> = options
            .into_iter()
            .map(|(id, label)| QueryItem { id, label })
            .collect();
        Self {
            items,
            ..Default::default()
        }
    }
}

#[modal_impl]
impl LoadQueryModal {
    fn default_result(&self) -> Option<(Option<i64>, Vec<i64>)> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<(Option<i64>, Vec<i64>)>>) {
        self.list.set(ListState::new(self.items.clone()));
        mx.focus("load-query-input");
    }

    #[keybinds]
    fn keys() {
        bind("escape", cancel);
        bind("d", delete);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<(Option<i64>, Vec<i64>)>>) {
        let deletes = self.staged_deletes.get();
        mx.close(Some((None, deletes)));
    }

    #[handler]
    async fn delete(&self) {
        let focused = self.list.with_ref(|s| s.focused_key);
        if let Some(id) = focused {
            // Stage deletion: add to list and remove from visible items
            self.staged_deletes.update(|deletes| deletes.push(id));

            // Remove item from visible list and update focus
            let new_items: Vec<QueryItem> = self.list.with_ref(|s| {
                s.items
                    .iter()
                    .filter(|item| item.id != id)
                    .cloned()
                    .collect()
            });

            let new_focus = if new_items.is_empty() {
                None
            } else {
                Some(new_items[0].id)
            };

            self.list.update(|s| {
                s.set_items(new_items);
                s.focused_key = new_focus;
            });
        }
    }

    #[handler]
    async fn on_activate(&self, mx: &ModalContext<Option<(Option<i64>, Vec<i64>)>>) {
        let activated = self.list.with_ref(|s| s.last_activated);
        if let Some(id) = activated {
            let deletes = self.staged_deletes.get();
            mx.close(Some((Some(id), deletes)));
        }
    }

    #[handler]
    async fn on_input_change(&self) {
        let query = self.input.get();
        let filtered: Vec<QueryItem> = if query.is_empty() {
            self.items.clone()
        } else {
            let lower = query.to_lowercase();
            self.items
                .iter()
                .filter(|item| item.label.to_lowercase().contains(&lower))
                .cloned()
                .collect()
        };
        self.list.update(|s| s.set_items(filtered));
    }

    #[handler]
    async fn on_input_submit(&self, mx: &ModalContext<Option<(Option<i64>, Vec<i64>)>>) {
        let first_key = self.list.with_ref(|s| s.items.first().map(|i| i.key()));
        if let Some(key) = first_key {
            mx.focus(&format!("load-query-list-item-{}", key));
        }
    }

    fn element(&self) -> Element {
        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Load Query") style (bold, fg: interact)

                input (state: self.input, id: "load-query-input", placeholder: "Search saved queries...")
                    on_change: on_input_change()
                    on_submit: on_input_submit()

                box_ (id: "load-query-list-container", height: fill, width: fill) {
                    list (state: self.list, id: "load-query-list")
                        on_activate: on_activate()
                }

                row (width: fill, justify: between) {
                    button (label: "Done", hint: "esc", id: "cancel") on_activate: cancel()
                    row (gap: 2) {
                        row (gap: 1) {
                            text (content: "d") style (fg: primary)
                            text (content: "stage delete") style (fg: muted)
                        }
                        row (gap: 1) {
                            text (content: "enter") style (fg: primary)
                            text (content: "load") style (fg: muted)
                        }
                    }
                }
            }
        }
    }
}
