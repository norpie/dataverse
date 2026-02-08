//! Modal for editing a Match transform.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::Checkbox;
use rafter::widgets::Text;
use tuidom::Element;

/// Modal for editing a Match transform's `has_default` flag.
#[modal(size = Sm)]
pub struct MatchTransformModal {
    /// Whether the match has a default branch.
    has_default: bool,
}

impl MatchTransformModal {
    /// Create a new Match transform modal.
    pub fn new_modal(has_default: bool) -> Self {
        Self::new(has_default)
    }
}

#[modal_impl]
impl MatchTransformModal {
    fn default_result(&self) -> Option<bool> {
        None
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
        bind("ctrl+s", save);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<bool>>) {
        mx.close(None);
    }

    #[handler]
    async fn save(&self, mx: &ModalContext<Option<bool>>) {
        mx.close(Some(self.has_default.get()));
    }

    fn element(&self) -> Element {
        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Edit Match Transform") style (bold, fg: interact)

                text (content: "Branches are managed in the tree. Use 'a' on the match node to add branches.") style (fg: muted)

                checkbox (state: self.has_default, id: "has-default", label: "Has default branch")

                // Spacer
                box_ (height: fill) {}

                // Buttons
                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn")
                        on_activate: cancel()
                    button (label: "Save", hint: "ctrl+s", id: "save-btn")
                        on_activate: save()
                }
            }
        }
    }
}
