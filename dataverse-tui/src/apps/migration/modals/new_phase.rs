//! Modal for creating a new phase.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::Input;
use rafter::widgets::Text;
use tuidom::Element;

use crate::apps::migration::types::Mode;

/// Result of the new phase modal.
#[derive(Debug, Clone)]
pub struct NewPhaseResult {
    pub name: String,
    pub mode: Mode,
}

/// Modal for creating a new phase.
#[modal(size = Sm)]
pub struct NewPhaseModal {
    name: String,
    error: Option<String>,
}

impl NewPhaseModal {
    /// Create a new phase modal.
    pub fn new_modal() -> Self {
        Self::new(String::new(), None)
    }
}

#[modal_impl]
impl NewPhaseModal {
    fn default_result(&self) -> Option<NewPhaseResult> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<NewPhaseResult>>) {
        mx.focus("new-phase-name");
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<NewPhaseResult>>) {
        mx.close(None);
    }

    #[handler]
    async fn submit(&self, mx: &ModalContext<Option<NewPhaseResult>>) {
        let name = self.name.get().trim().to_string();
        if name.is_empty() {
            self.error.set(Some("Name is required".to_string()));
            return;
        }

        mx.close(Some(NewPhaseResult {
            name,
            mode: Mode::Declarative,
        }));
    }

    fn element(&self) -> Element {
        let error = self.error.get();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "New Phase") style (bold, fg: interact)

                if let Some(err) = error {
                    text (content: {err}) style (fg: error)
                }

                column (gap: 1, width: fill, height: fill) {
                    text (content: "Name") style (fg: muted)
                    input (state: self.name, id: "new-phase-name", placeholder: "Phase name...")
                        on_submit: submit()
                }

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn") on_activate: cancel()
                    button (label: "Create", id: "create-btn") on_activate: submit()
                }
            }
        }
    }
}
