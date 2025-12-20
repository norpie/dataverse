//! Rename/input modal dialog.

use rafter::prelude::*;

/// A modal for entering or editing a name.
#[modal]
pub struct RenameModal {
    #[state(skip)]
    title: String,
    #[state(skip)]
    initial_value: String,
}

impl RenameModal {
    /// Create a rename modal with an existing name.
    pub fn new(current_name: impl Into<String>) -> Self {
        Self {
            title: "Rename".to_string(),
            initial_value: current_name.into(),
        }
    }

    /// Create a modal with a custom title and initial value.
    pub fn with_title(title: impl Into<String>, initial: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            initial_value: initial.into(),
        }
    }
}

#[modal_impl]
impl RenameModal {
    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "escape" => cancel,
        }
    }

    #[handler]
    async fn submit(&self, cx: &AppContext, mx: &ModalContext<Option<String>>) {
        let value = cx.input_text().unwrap_or_default();
        if value.is_empty() {
            cx.toast("Name cannot be empty");
        } else {
            mx.close(Some(value));
        }
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<String>>) {
        mx.close(None);
    }

    fn view(&self) -> Node {
        let title = self.title.clone();
        let initial = self.initial_value.clone();

        view! {
            column (padding: 2, gap: 1, bg: surface) {
                text (bold, fg: primary) { title }
                input (
                    id: "name_input",
                    value: initial,
                    placeholder: "Enter name...",
                    on_submit: submit
                )
                row (gap: 2) {
                    button(label: "Cancel [esc]", id: "cancel", on_click: cancel)
                    button(label: "Save [enter]", id: "save", on_click: submit)
                }
            }
        }
    }
}
