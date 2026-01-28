//! Number editor modal for the Top value.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, NumberInput, NumberInputState, Text};

/// Modal for editing the Top (record limit) value.
#[modal(default)]
pub struct NumberEditorModal {
    #[state(skip)]
    initial: f64,

    value: NumberInputState,
}

impl NumberEditorModal {
    pub fn new(current: Option<u32>) -> Self {
        Self {
            initial: current.unwrap_or(100) as f64,
            ..Default::default()
        }
    }
}

#[modal_impl]
impl NumberEditorModal {
    fn default_result(&self) -> Option<u32> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<u32>>) {
        self.value
            .set(NumberInputState::new(self.initial).with_min(1.0).integer());
        mx.focus("top-value");
    }

    #[keybinds]
    fn keys() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<u32>>) {
        mx.close(None);
    }

    #[handler]
    async fn confirm(&self, mx: &ModalContext<Option<u32>>) {
        let val = self.value.with_ref(|s| s.value() as u32);
        mx.close(Some(val));
    }

    fn element(&self) -> Element {
        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Record Limit") style (bold, fg: interact)
                number_input (state: self.value, id: "top-value", placeholder: "100", width: 10)
                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel") on_activate: cancel()
                    button (label: "Ok", id: "ok") on_activate: confirm()
                }
            }
        }
    }
}
