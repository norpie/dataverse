//! Rename/input modal dialog.

use rafter::prelude::*;

/// A modal for entering or editing a name.
#[modal]
pub struct RenameModal {
    #[state(skip)]
    title: String,
    name_input: Input,
}

impl RenameModal {
    /// Create a rename modal with an existing name.
    pub fn new(current_name: impl Into<String>) -> Self {
        Self {
            title: "Rename".to_string(),
            name_input: Input::with_value(current_name),
        }
    }

    /// Create a modal with a custom title and initial value.
    pub fn with_title(title: impl Into<String>, initial: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            name_input: Input::with_value(initial),
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
        let value = self.name_input.value();
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

        view! {
            column (padding: 2, gap: 1, bg: surface) {
                text (bold, fg: primary) { title }
                input (
                    bind: self.name_input,
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
