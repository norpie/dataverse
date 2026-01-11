//! Input modal demonstrating modals that return user input.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Input, Text};

// ============================================================================
// Input Modal (returns String)
// ============================================================================

#[modal(size = Md)]
pub struct InputModal {
    #[state(skip)]
    pub prompt: String,
    value: String,
}

impl InputModal {
    pub fn with_prompt(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            ..Default::default()
        }
    }
}

#[modal_impl(Result = Option<String>)]
impl InputModal {
    #[keybinds]
    fn keys() {
        bind("escape", cancel);
        bind("enter", submit);
    }

    #[handler]
    async fn submit(&self, mx: &ModalContext<Option<String>>) {
        let value = self.value.get();
        if !value.is_empty() {
            mx.close(Some(value));
        }
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<String>>) {
        mx.close(None);
    }

    fn element(&self) -> Element {
        let prompt = self.prompt.clone();

        page! {
            column (padding: 2, gap: 1) style (bg: surface) {
                text (content: "Input Modal") style (bold, fg: primary)
                text (content: {prompt}) style (fg: muted)
                input (state: self.value, id: "input", placeholder: "Type something...")
                row (gap: 2) {
                    button (label: "Cancel [Esc]", id: "cancel") on_activate: cancel()
                    button (label: "Submit [Enter]", id: "submit") on_activate: submit()
                }
            }
        }
    }
}

// ============================================================================
// Name Input Modal (demonstrates typed result)
// ============================================================================

#[modal(size = Sm)]
pub struct NameModal {
    first_name: String,
    last_name: String,
}

/// Result type for NameModal
#[derive(Debug, Clone, Default)]
pub struct NameResult {
    pub first_name: String,
    pub last_name: String,
}

#[modal_impl(Result = Option<NameResult>)]
impl NameModal {
    #[keybinds]
    fn keys() {
        bind("escape", cancel);
    }

    #[handler]
    async fn submit(&self, mx: &ModalContext<Option<NameResult>>) {
        let first = self.first_name.get();
        let last = self.last_name.get();
        if !first.is_empty() && !last.is_empty() {
            mx.close(Some(NameResult {
                first_name: first,
                last_name: last,
            }));
        }
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<NameResult>>) {
        mx.close(None);
    }

    fn element(&self) -> Element {
        page! {
            column (padding: 2, gap: 1) style (bg: surface) {
                text (content: "Enter Your Name") style (bold, fg: primary)
                column (gap: 1) {
                    text (content: "First Name:") style (fg: muted)
                    input (state: self.first_name, id: "first", placeholder: "John")
                }
                column (gap: 1) {
                    text (content: "Last Name:") style (fg: muted)
                    input (state: self.last_name, id: "last", placeholder: "Doe")
                }
                row (gap: 2) {
                    button (label: "Cancel", id: "cancel") on_activate: cancel()
                    button (label: "Submit", id: "submit") on_activate: submit()
                }
            }
        }
    }
}
