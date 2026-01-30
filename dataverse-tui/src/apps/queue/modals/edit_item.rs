//! Edit item modal - allows editing queue item properties.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Input, Text};

use crate::apps::queue::repository::UpdateItem;
use crate::apps::queue::types::QueueItem;

/// Modal for editing a queue item's properties.
/// Returns `Some(UpdateItem)` on confirm, `None` on cancel.
#[modal(default)]
pub struct EditItemModal {
    #[state(skip)]
    env_id: i64,
    priority_input: String,
    description_input: String,
    source_input: String,
}

impl EditItemModal {
    pub fn for_item(item: &QueueItem) -> Self {
        Self {
            env_id: item.env_id,
            priority_input: State::new(item.priority.to_string()),
            description_input: State::new(item.description.clone()),
            source_input: State::new(item.source.clone()),
            ..Default::default()
        }
    }
}

#[modal_impl]
impl EditItemModal {
    fn default_result(&self) -> Option<UpdateItem> {
        None
    }

    #[keybinds]
    fn keys() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<UpdateItem>>) {
        mx.close(None);
    }

    #[handler]
    async fn confirm(&self, mx: &ModalContext<Option<UpdateItem>>) {
        let priority_text = self.priority_input.get();
        let priority: i32 = match priority_text.trim().parse() {
            Ok(p) => p,
            Err(_) => return,
        };

        let description = self.description_input.get();
        let source = self.source_input.get();

        if description.trim().is_empty() {
            return;
        }

        mx.close(Some(UpdateItem {
            priority,
            description,
            source,
            env_id: self.env_id,
        }));
    }

    fn element(&self) -> Element {
        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Edit Item") style (bold, fg: interact)

                input (
                    state: self.priority_input,
                    id: "priority",
                    label: "Priority",
                    placeholder: "0"
                )
                input (
                    state: self.description_input,
                    id: "description",
                    label: "Description",
                    placeholder: "Item description..."
                )
                input (
                    state: self.source_input,
                    id: "source",
                    label: "Source",
                    placeholder: "e.g. manual, import, sync"
                )

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel") on_activate: cancel()
                    button (label: "Save", id: "save") on_activate: confirm()
                }
            }
        }
    }
}
