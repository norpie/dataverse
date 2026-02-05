//! Modal for adding a new variable.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::Input;
use rafter::widgets::Text;
use tuidom::Element;

/// Result of the add variable modal.
#[derive(Debug, Clone)]
pub struct AddVariableResult {
    pub name: String,
}

/// Modal for adding a new variable.
#[modal(size = Sm)]
pub struct AddVariableModal {
    name: String,
    error: Option<String>,
}

impl AddVariableModal {
    /// Create a new add variable modal.
    pub fn new_modal() -> Self {
        Self::new(String::new(), None)
    }

    /// Create an edit variable modal with initial name.
    pub fn edit_modal(name: &str) -> Self {
        Self::new(name.to_string(), None)
    }
}

#[modal_impl]
impl AddVariableModal {
    fn default_result(&self) -> Option<AddVariableResult> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<AddVariableResult>>) {
        mx.focus("variable-name-input");
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<AddVariableResult>>) {
        mx.close(None);
    }

    #[handler]
    async fn submit(&self, mx: &ModalContext<Option<AddVariableResult>>) {
        let name = self.name.get().trim().to_string();
        if name.is_empty() {
            self.error.set(Some("Name is required".to_string()));
            return;
        }

        // Validate: no spaces, no $ prefix (we add it in display)
        if name.contains(' ') {
            self.error
                .set(Some("Variable name cannot contain spaces".to_string()));
            return;
        }

        // Remove leading $ if user typed it
        let name = name.strip_prefix('$').unwrap_or(&name).to_string();

        mx.close(Some(AddVariableResult { name }));
    }

    fn element(&self) -> Element {
        let error = self.error.get();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Add Variable") style (bold, fg: interact)

                if let Some(err) = error {
                    text (content: {err}) style (fg: error)
                }

                column (gap: 1, width: fill, height: fill) {
                    text (content: "Name") style (fg: muted)
                    input (state: self.name, id: "variable-name-input", placeholder: "variable_name")
                        on_submit: submit()
                    text (content: "Will be accessible as $name in transforms") style (fg: muted)
                }

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn") on_activate: cancel()
                    button (label: "Add", id: "add-btn") on_activate: submit()
                }
            }
        }
    }
}
