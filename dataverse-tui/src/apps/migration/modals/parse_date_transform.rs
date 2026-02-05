//! Modal for editing a Parse Date transform.

use rafter::page;
use rafter::prelude::*;
use tuidom::Element;

/// Modal for editing a Parse Date transform's format string.
#[modal(size = Sm)]
pub struct ParseDateTransformModal {
    /// Format string for parsing.
    format: String,
}

impl ParseDateTransformModal {
    /// Create a new Parse Date transform modal.
    pub fn new_modal(current_format: String) -> Self {
        Self::new(current_format)
    }
}

#[modal_impl]
impl ParseDateTransformModal {
    fn default_result(&self) -> Option<String> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<String>>) {
        mx.focus("format-input");
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
        bind("ctrl+s", save);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<String>>) {
        mx.close(None);
    }

    #[handler]
    async fn save(&self, mx: &ModalContext<Option<String>>) {
        let format = self.format.get().clone();
        if !format.is_empty() {
            mx.close(Some(format));
        }
    }

    fn element(&self) -> Element {
        let format = self.format.get();
        let is_empty = format.trim().is_empty();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Edit Parse Date Transform") style (bold, fg: interact)

                column (gap: 0, width: fill) {
                    text (content: "Format string") style (fg: muted)
                    input (state: self.format, id: "format-input", placeholder: "%Y-%m-%d", width: fill)
                }

                // Help text - common format specifiers
                column (gap: 0, width: fill) {
                    text (content: "Common format specifiers:") style (fg: muted)
                    text (content: "  %Y - Year (4 digits)    %m - Month (01-12)") style (fg: muted)
                    text (content: "  %d - Day (01-31)        %H - Hour (00-23)") style (fg: muted)
                    text (content: "  %M - Minute (00-59)     %S - Second (00-59)") style (fg: muted)
                    text (content: "  %Y-%m-%d = 2024-01-15") style (fg: muted)
                    text (content: "  %d/%m/%Y = 15/01/2024") style (fg: muted)
                }

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
