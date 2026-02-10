//! Modal for selecting a phase to preview.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::RadioGroup;
use rafter::widgets::RadioState;
use rafter::widgets::Text;
use tuidom::Element;

/// Modal for selecting which phase to preview.
#[modal(size = Sm)]
pub struct SelectPhaseModal {
    phases: RadioState<i64>,
}

impl SelectPhaseModal {
    /// Create a new phase selection modal.
    ///
    /// `phases` is a list of `(id, name)` pairs.
    pub fn new_modal(phases: Vec<(i64, String)>) -> Self {
        let options: Vec<(i64, String)> = phases
            .into_iter()
            .map(|(id, name)| (id, name))
            .collect();
        let state = RadioState::new(options);
        Self::new(state)
    }
}

#[modal_impl]
impl SelectPhaseModal {
    fn default_result(&self) -> Option<i64> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<i64>>) {
        mx.focus("phase-radio");
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<i64>>) {
        mx.close(None);
    }

    #[handler]
    async fn submit(&self, mx: &ModalContext<Option<i64>>) {
        let selected = self.phases.with_ref(|s| s.value);
        mx.close(selected);
    }

    fn element(&self) -> Element {
        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Select Phase") style (bold, fg: interact)
                text (content: "Choose a phase to preview.") style (fg: muted)

                radio_group (state: self.phases, id: "phase-radio")

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn") on_activate: cancel()
                    button (label: "Preview", id: "preview-btn") on_activate: submit()
                }
            }
        }
    }
}
