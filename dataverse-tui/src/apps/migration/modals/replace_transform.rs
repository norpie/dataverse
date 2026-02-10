//! Modal for editing a Replace transform.

use rafter::page;
use rafter::prelude::*;
use tuidom::Element;

/// Result from the Replace transform modal.
pub struct ReplaceResult {
    pub from: String,
    pub to: String,
    pub regex: bool,
}

/// Modal for editing a Replace transform.
#[modal(size = Sm)]
pub struct ReplaceTransformModal {
    /// Pattern to find.
    from: String,
    /// Replacement text.
    to: String,
    /// Whether `from` is a regex pattern.
    regex: bool,
}

impl ReplaceTransformModal {
    /// Create a new Replace transform modal.
    pub fn new_modal(from: String, to: String, regex: bool) -> Self {
        Self::new(from, to, regex)
    }
}

#[modal_impl]
impl ReplaceTransformModal {
    fn default_result(&self) -> Option<ReplaceResult> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<ReplaceResult>>) {
        mx.focus("from-input");
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
        bind("ctrl+s", save);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<ReplaceResult>>) {
        mx.close(None);
    }

    #[handler]
    async fn save(&self, mx: &ModalContext<Option<ReplaceResult>>) {
        let from = self.from.get().clone();
        if !from.is_empty() {
            mx.close(Some(ReplaceResult {
                from,
                to: self.to.get().clone(),
                regex: self.regex.get(),
            }));
        }
    }

    fn element(&self) -> Element {
        let from = self.from.get();
        let is_empty = from.trim().is_empty();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Edit Replace Transform") style (bold, fg: interact)

                column (gap: 0, width: fill) {
                    text (content: "Find") style (fg: muted)
                    input (state: self.from, id: "from-input", placeholder: "Pattern to find", width: fill)
                }

                column (gap: 0, width: fill) {
                    text (content: "Replace with") style (fg: muted)
                    input (state: self.to, id: "to-input", placeholder: "Replacement text", width: fill)
                }

                row (gap: 1, width: fill) {
                    checkbox (state: self.regex, id: "regex-checkbox", label: "Use regular expression")
                }

                // Help text
                text (content: "Replaces occurrences in the current pipeline value (#value)") style (fg: muted)

                // Spacer
                box_ (height: fill) {}

                // Buttons
                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn")
                        on_activate: cancel()
                    button (
                        label: "Save",
                        hint: "ctrl+s",
                        id: "save-btn",
                        disabled: {is_empty}
                    )
                        on_activate: save()
                }
            }
        }
    }
}
