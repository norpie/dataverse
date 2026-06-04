//! Error details modal - displays full error text from a failed execution.

use std::fs;
use std::path::PathBuf;

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Text};

use crate::modals::FileBrowserModal;
use crate::paths;

/// Modal that displays the full error text from a failed queue item execution.
#[modal(default, size = Lg)]
pub struct ErrorDetailsModal {
    #[state(skip)]
    item_id: i64,
    #[state(skip)]
    error_text: String,
}

impl ErrorDetailsModal {
    pub fn with_error(item_id: i64, error_text: String) -> Self {
        Self {
            item_id,
            error_text,
            ..Default::default()
        }
    }
}

#[modal_impl]
impl ErrorDetailsModal {
    fn default_result(&self) {}

    #[keybinds]
    fn keys() {
        bind("escape", close);
        bind("e", export_error);
    }

    #[handler]
    async fn close(&self, mx: &ModalContext<()>) {
        mx.close(());
    }

    #[handler]
    async fn export_error(&self, gx: &GlobalContext) {
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let default_filename = format!("queue_error_{}_{}", self.item_id, timestamp);
        let start_dir = paths::downloads_dir().unwrap_or_else(|| PathBuf::from("."));
        let Some(result) = gx
            .modal(
                FileBrowserModal::browse(&start_dir, vec!["txt".to_string()])
                    .with_filename(default_filename),
            )
            .await
        else {
            return;
        };

        let content = format!(
            "Queue Item: {}\nExported: {}\n\n{}",
            self.item_id,
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            self.error_text
        );

        match fs::write(&result.path, content) {
            Ok(()) => gx.toast(Toast::success(format!(
                "Exported error to {}",
                result.path.display()
            ))),
            Err(e) => {
                log::error!("Failed to export queue error: {}", e);
                gx.toast(Toast::error("Failed to export error"));
            }
        }
    }

    fn element(&self) -> Element {
        let error = self.error_text.clone();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Error Details") style (bold, fg: interact)
                column (id: "error-scroll", height: fill, width: fill, overflow: scroll) {
                    text (content: {error}) style (fg: error)
                }
                row (width: fill, justify: between) {
                    button (label: "Export", hint: "e", id: "export") on_activate: export_error()
                    button (label: "Close", hint: "esc", id: "close") on_activate: close()
                }
            }
        }
    }
}
