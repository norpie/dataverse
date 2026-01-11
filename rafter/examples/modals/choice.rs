//! Choice modal demonstrating modals that return enum values.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Text};

// ============================================================================
// Choice enum
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    OptionA,
    OptionB,
    OptionC,
}

impl std::fmt::Display for Choice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Choice::OptionA => write!(f, "Option A"),
            Choice::OptionB => write!(f, "Option B"),
            Choice::OptionC => write!(f, "Option C"),
        }
    }
}

// ============================================================================
// Choice Modal
// ============================================================================

#[modal(size = Sm)]
pub struct ChoiceModal {
    #[state(skip)]
    pub title: String,
}

impl ChoiceModal {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            ..Default::default()
        }
    }
}

#[modal_impl(Result = Option<Choice>)]
impl ChoiceModal {
    #[keybinds]
    fn keys() {
        bind("escape", cancel);
        bind("a", "1", choose_a);
        bind("b", "2", choose_b);
        bind("c", "3", choose_c);
    }

    #[handler]
    async fn choose_a(&self, mx: &ModalContext<Option<Choice>>) {
        mx.close(Some(Choice::OptionA));
    }

    #[handler]
    async fn choose_b(&self, mx: &ModalContext<Option<Choice>>) {
        mx.close(Some(Choice::OptionB));
    }

    #[handler]
    async fn choose_c(&self, mx: &ModalContext<Option<Choice>>) {
        mx.close(Some(Choice::OptionC));
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<Choice>>) {
        mx.close(None);
    }

    fn element(&self) -> Element {
        let title = self.title.clone();

        page! {
            column (padding: 2, gap: 1) style (bg: surface) {
                text (content: {title}) style (bold, fg: primary)
                text (content: "Select an option:") style (fg: muted)
                column (gap: 1) {
                    button (label: "[A] Option A", id: "opt-a") on_activate: choose_a()
                    button (label: "[B] Option B", id: "opt-b") on_activate: choose_b()
                    button (label: "[C] Option C", id: "opt-c") on_activate: choose_c()
                }
                text (content: "Press Esc to cancel") style (fg: muted)
            }
        }
    }
}

// ============================================================================
// Confirm Modal (boolean choice)
// ============================================================================

#[modal]
pub struct ConfirmModal {
    #[state(skip)]
    pub message: String,
}

impl ConfirmModal {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            ..Default::default()
        }
    }
}

#[modal_impl(Result = bool)]
impl ConfirmModal {
    #[keybinds]
    fn keys() {
        bind("y", "enter", confirm);
        bind("n", "escape", deny);
    }

    #[handler]
    async fn confirm(&self, mx: &ModalContext<bool>) {
        mx.close(true);
    }

    #[handler]
    async fn deny(&self, mx: &ModalContext<bool>) {
        mx.close(false);
    }

    fn element(&self) -> Element {
        let message = self.message.clone();

        page! {
            column (padding: 2, gap: 1) style (bg: surface) {
                text (content: "Confirm") style (bold, fg: warning)
                text (content: {message})
                row (gap: 2) {
                    button (label: "No [N]", id: "no") on_activate: deny()
                    button (label: "Yes [Y]", id: "yes") on_activate: confirm()
                }
            }
        }
    }
}
