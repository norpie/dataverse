//! Modal for editing test GUIDs for an entity mapping.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::Input;
use rafter::widgets::List;
use rafter::widgets::ListState;
use rafter::widgets::Text;
use uuid::Uuid;

/// Parse a string containing GUIDs separated by commas, spaces, or newlines.
/// Returns a list of valid, lowercased GUIDs with duplicates removed.
fn parse_guids(input: &str) -> Vec<String> {
    let mut guids = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Split by comma, space, newline, or any combination
    for part in input.split(|c: char| c == ',' || c.is_whitespace()) {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Try to parse as UUID
        if let Ok(uuid) = Uuid::parse_str(trimmed) {
            let normalized = uuid.to_string().to_lowercase();
            if seen.insert(normalized.clone()) {
                guids.push(normalized);
            }
        }
    }

    guids
}

/// Modal for editing test GUIDs.
#[modal(size = Md)]
pub struct TestGuidsModal {
    #[state(skip)]
    entity_mapping_id: i64,
    guids: ListState<String>,
    paste_input: String,
    error: Option<String>,
}

impl TestGuidsModal {
    /// Create a modal for editing test GUIDs.
    pub fn new_modal(entity_mapping_id: i64, initial_guids: Vec<String>) -> Self {
        Self::new(
            entity_mapping_id,
            ListState::new(initial_guids),
            String::new(),
            None,
        )
    }
}

#[modal_impl]
impl TestGuidsModal {
    fn default_result(&self) -> Option<Vec<String>> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<Vec<String>>>) {
        // Focus paste input if no GUIDs, otherwise focus list
        if self.guids.with_ref(|g| g.items.is_empty()) {
            mx.focus("paste-input");
        } else {
            mx.focus("guids-list");
        }
    }

    // =========================================================================
    // Derived State
    // =========================================================================

    #[derived]
    fn guid_count(&self) -> usize {
        self.guids.with_ref(|g| g.items.len())
    }

    #[derived]
    fn has_focused(&self) -> bool {
        self.guids.with_ref(|g| g.focused_key.is_some())
    }

    // =========================================================================
    // Keybinds
    // =========================================================================

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
        bind("a", add_guids);
        bind("d", remove_selected);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<Vec<String>>>) {
        mx.close(None);
    }

    #[handler]
    async fn add_guids(&self, cx: &AppContext) {
        let input = self.paste_input.get();
        if input.trim().is_empty() {
            // If paste input is empty, focus it
            cx.focus("paste-input");
            return;
        }

        let new_guids = parse_guids(&input);
        if new_guids.is_empty() {
            self.error
                .set(Some("No valid GUIDs found in input".to_string()));
            return;
        }

        // Add new GUIDs to list (avoiding duplicates)
        self.guids.update(|state| {
            let existing: std::collections::HashSet<_> = state.items.iter().cloned().collect();
            for guid in new_guids {
                if !existing.contains(&guid) {
                    state.push_item(guid);
                }
            }
        });

        // Clear input and error
        self.paste_input.set(String::new());
        self.error.set(None);

        // Focus list
        cx.focus("guids-list");
    }

    #[handler]
    async fn remove_selected(&self, cx: &AppContext) {
        let focused_key = self.guids.with_ref(|g| g.focused_key.clone());

        if let Some(key) = focused_key {
            self.guids.update(|state| {
                let new_items: Vec<_> = state.items
                    .iter()
                    .filter(|guid| **guid != key)
                    .cloned()
                    .collect();
                state.set_items(new_items);
            });
        }

        // Re-focus list
        cx.focus("guids-list");
    }

    #[handler]
    async fn submit(&self, mx: &ModalContext<Option<Vec<String>>>) {
        let guids = self.guids.with_ref(|g| g.items.to_vec());
        mx.close(Some(guids));
    }

    fn element(&self) -> Element {
        let error = self.error.get();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Edit Test GUIDs") style (bold, fg: interact)

                if let Some(err) = error {
                    text (content: {err}) style (fg: error)
                }

                text (content: {format!("Test GUIDs ({})", self.guid_count())}) style (fg: muted)

                // List of GUIDs
                box_ (id: "guids-list-container", height: fill, width: fill) style (bg: surface2) {
                    list (state: self.guids, id: "guids-list", width: fill, height: fill)
                }

                text (content: "Paste GUIDs (comma/space/newline):") style (fg: muted)

                input (
                    state: self.paste_input,
                    id: "paste-input",
                    placeholder: "e.g., guid1, guid2, guid3"
                )
                    on_submit: add_guids()

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn")
                        on_activate: cancel()

                    row (gap: 1) {
                        button (
                            label: "Remove",
                            hint: "d",
                            id: "remove-btn",
                            disabled: {!self.has_focused()}
                        )
                            on_activate: remove_selected()

                        button (label: "Add", hint: "a", id: "add-btn")
                            on_activate: add_guids()

                        button (label: "Save", id: "save-btn")
                            on_activate: submit()
                    }
                }
            }
        }
    }
}
