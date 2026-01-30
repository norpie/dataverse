//! Queue settings modal - configure concurrency and failure threshold.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Input, Text};

/// Modal for editing queue settings.
/// Returns `Some((concurrency, max_failures))` on confirm, `None` on cancel.
#[modal(default)]
pub struct SettingsModal {
    concurrency_input: String,
    max_failures_input: String,
}

impl SettingsModal {
    pub fn with_settings(concurrency: usize, max_failures: usize) -> Self {
        Self {
            concurrency_input: State::new(concurrency.to_string()),
            max_failures_input: State::new(max_failures.to_string()),
            ..Default::default()
        }
    }
}

#[modal_impl]
impl SettingsModal {
    fn default_result(&self) -> Option<(usize, usize)> {
        None
    }

    #[keybinds]
    fn keys() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<(usize, usize)>>) {
        mx.close(None);
    }

    #[handler]
    async fn confirm(&self, mx: &ModalContext<Option<(usize, usize)>>) {
        let concurrency_text = self.concurrency_input.get();
        let concurrency: usize = match concurrency_text.trim().parse() {
            Ok(c) if (1..=20).contains(&c) => c,
            _ => return,
        };

        let max_failures_text = self.max_failures_input.get();
        let max_failures: usize = match max_failures_text.trim().parse() {
            Ok(f) if f >= 1 => f,
            _ => return,
        };

        mx.close(Some((concurrency, max_failures)));
    }

    fn element(&self) -> Element {
        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Queue Settings") style (bold, fg: interact)

                input (
                    state: self.concurrency_input,
                    id: "concurrency",
                    label: "Max Concurrency (1-20)",
                    placeholder: "5"
                )
                input (
                    state: self.max_failures_input,
                    id: "max-failures",
                    label: "Max Consecutive Failures",
                    placeholder: "10"
                )

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel") on_activate: cancel()
                    button (label: "Save", id: "save") on_activate: confirm()
                }
            }
        }
    }
}
