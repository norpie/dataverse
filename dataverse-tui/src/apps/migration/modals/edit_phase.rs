//! Modal for editing a phase.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::Input;
use rafter::widgets::RadioGroup;
use rafter::widgets::RadioState;
use rafter::widgets::Text;

use crate::apps::migration::types::Mode;
use crate::apps::migration::types::Phase;

/// Result of the edit phase modal.
#[derive(Debug, Clone)]
pub struct EditPhaseResult {
    pub name: String,
    pub mode: Mode,
}

/// Modal for editing a phase.
#[modal(size = Sm)]
pub struct EditPhaseModal {
    #[state(skip)]
    phase_id: i64,
    name: String,
    mode: RadioState<Mode>,
    error: Option<String>,
}

impl EditPhaseModal {
    /// Create an edit phase modal for the given phase.
    pub fn for_phase(phase: &Phase) -> Self {
        let mode_state = RadioState::new([
            (Mode::Declarative, "Declarative"),
            (Mode::Lua, "Lua"),
        ])
        .with_value(phase.mode);

        Self::new(phase.id, phase.name.clone(), mode_state, None)
    }
}

#[modal_impl]
impl EditPhaseModal {
    fn default_result(&self) -> Option<EditPhaseResult> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<EditPhaseResult>>) {
        mx.focus("edit-phase-name");
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<EditPhaseResult>>) {
        mx.close(None);
    }

    #[handler]
    async fn submit(&self, mx: &ModalContext<Option<EditPhaseResult>>) {
        let name = self.name.get().trim().to_string();
        if name.is_empty() {
            self.error.set(Some("Name is required".to_string()));
            return;
        }

        let mode = self.mode.with_ref(|s| s.value.unwrap_or(Mode::Declarative));

        mx.close(Some(EditPhaseResult { name, mode }));
    }

    fn element(&self) -> Element {
        let error = self.error.get();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Edit Phase") style (bold, fg: interact)

                if let Some(err) = error {
                    text (content: {err}) style (fg: error)
                }

                column (width: fill, height: fill) {
                    input (state: self.name, id: "edit-phase-name", label: "Name")
                        on_submit: submit()
                    column {
                        text (content: "Mode") style (fg: muted)
                        radio_group (state: self.mode, id: "edit-phase-mode")
                    }
                }

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn") on_activate: cancel()
                    button (label: "Save", id: "save-btn") on_activate: submit()
                }
            }
        }
    }
}
