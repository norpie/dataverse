//! Error modal for displaying errors to the user.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, List, ListItem, ListState, Text};

/// Wrapper for error messages to use in List widget.
#[derive(Debug, Clone)]
struct ErrorItem {
    index: usize,
    message: String,
}

impl ListItem for ErrorItem {
    type Key = usize;

    fn key(&self) -> Self::Key {
        self.index
    }

    fn render(&self) -> Element {
        Element::text(&self.message)
    }
}

/// Modal for displaying a single error or list of errors.
#[modal(size = Md)]
pub struct ErrorModal {
    /// Title of the error modal.
    #[state(skip)]
    title: String,

    /// Error messages to display.
    #[state(skip)]
    errors: Vec<String>,

    /// List state for scrolling through errors.
    list: ListState<ErrorItem>,
}

impl ErrorModal {
    /// Create an error modal with a single error message.
    pub fn with_message(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self::with_errors(title, vec![message.into()])
    }

    /// Create a new error modal with multiple error messages.
    pub fn with_errors(title: impl Into<String>, errors: Vec<String>) -> Self {
        let error_items: Vec<ErrorItem> = errors
            .iter()
            .enumerate()
            .map(|(index, message)| ErrorItem {
                index,
                message: message.clone(),
            })
            .collect();

        Self {
            title: title.into(),
            errors,
            list: State::new(ListState::new(error_items)),
            __handler_registry: Default::default(),
            __derived_cache: Default::default(),
        }
    }
}

#[modal_impl]
impl ErrorModal {
    fn default_result(&self) -> () {
        ()
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<()>) {
        mx.focus("error-list");
    }

    #[keybinds]
    fn keys() {
        bind("escape", close);
        bind("enter", close);
    }

    #[handler]
    async fn close(&self, mx: &ModalContext<()>) {
        mx.close(());
    }

    fn element(&self) -> Element {
        let error_count = self.errors.len();
        let title = if error_count == 1 {
            self.title.clone()
        } else {
            format!("{} ({} errors)", self.title, error_count)
        };

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                // Title
                text (content: {title}) style (bold, fg: interact)

                // Error list
                if self.errors.len() == 1 {
                    // Single error - just show text
                    text (content: {self.errors[0].clone()}) style (fg: primary)
                } else {
                    // Multiple errors - show scrollable list
                    column (gap: 1, height: fill) {
                        text (content: "Errors:") style (fg: primary)
                        list (
                            state: self.list,
                            id: "error-list",
                            height: fill
                        )
                    }
                }

                // Button
                row (width: fill, justify: end) {
                    button (label: "OK", hint: "enter", id: "ok") on_activate: close()
                }
            }
        }
    }
}
