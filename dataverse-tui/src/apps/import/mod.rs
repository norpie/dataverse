//! Import app for importing data from CSV/Excel files into Dataverse.

mod io;
mod modals;

use std::path::PathBuf;

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Text};

use crate::file_io::list_sheets;
use crate::modals::FileBrowserModal;
use crate::paths;
use crate::systems::client_management::ActiveClientInfo;

/// Import app: load file, configure, send to queue.
#[app(name = "Import")]
pub struct Import {
    /// Client connection info.
    #[state(skip)]
    client_info: ActiveClientInfo,

    /// Currently loaded file path.
    file_path: Option<PathBuf>,

    /// Available sheets (Excel only).
    available_sheets: Vec<String>,
}

impl Import {
    pub fn new(client_info: ActiveClientInfo) -> Self {
        Self {
            client_info,
            file_path: State::default(),
            available_sheets: State::default(),
            __handler_registry: Default::default(),
            __derived_cache: Default::default(),
        }
    }
}

#[app_impl]
impl Import {
    fn title(&self) -> String {
        "Import".to_string()
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", close);
        bind("o", open_file);
    }

    #[handler]
    async fn close(&self, _gx: &GlobalContext, cx: &AppContext) {
        cx.close();
    }

    #[handler]
    async fn open_file(&self, gx: &GlobalContext) {
        // Show file browser modal for CSV and Excel files
        let start_dir = paths::downloads_dir().unwrap_or_else(|| PathBuf::from("."));
        let result = gx
            .modal(
                FileBrowserModal::new(&start_dir, vec!["csv".to_string(), "xlsx".to_string()])
                    .require_existing(),
            )
            .await;

        let Some(file_result) = result else {
            return; // User cancelled
        };

        // Store the file path
        self.file_path.set(Some(file_result.path.clone()));

        // If it's an Excel file, list available sheets
        if file_result.file_type == "xlsx" {
            match list_sheets(&file_result.path) {
                Ok(sheets) => {
                    self.available_sheets.set(sheets);
                    // TODO: Show sheet selection UI
                    gx.toast(Toast::info(format!(
                        "Loaded Excel file with {} sheets",
                        self.available_sheets.with_ref(|s| s.len())
                    )));
                }
                Err(e) => {
                    gx.toast(Toast::error(format!("Failed to read Excel file: {}", e)));
                    self.file_path.set(None);
                }
            }
        } else {
            // CSV file - can proceed directly to settings
            self.available_sheets.set(vec![]);
            gx.toast(Toast::info("CSV file loaded"));
            // TODO: Proceed to settings modal
        }
    }

    fn element(&self) -> Element {
        let (file_display, sheets_display, has_sheets, has_file) =
            self.file_path.with_ref(|p| match p.as_ref() {
                Some(path) => {
                    let filename = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");
                    let sheets = self.available_sheets.with_ref(|s| s.clone());
                    let has_sheets = !sheets.is_empty();
                    let file_str = format!("File: {}", filename);
                    let sheets_str = format!("Sheets: {}", sheets.len());
                    (file_str, sheets_str, has_sheets, true)
                }
                None => (String::new(), String::new(), false, false),
            });

        page! {
            column (padding: (1, 2), gap: 1, height: fill, width: fill) style (bg: background) {
                // Title
                text (content: "Import") style (fg: interact)

                // Content
                box_ (height: fill, width: fill) style (bg: surface) {
                    if has_file {
                        column (padding: (1, 2), gap: 1) {
                            text (content: file_display) style (fg: primary)
                            if has_sheets {
                                column (gap: 1) {
                                    text (content: sheets_display) style (fg: muted)
                                    text (content: "TODO: Select a sheet to import") style (fg: muted)
                                }
                            } else {
                                text (content: "Ready to configure import") style (fg: muted)
                            }
                        }
                    } else {
                        column (height: fill, width: fill, align: center, justify: center, gap: 1) {
                            text (content: "No file loaded") style (fg: muted)
                            text (content: "Press 'o' to open a file") style (fg: primary)
                        }
                    }
                }

                // Footer
                row (width: fill, justify: between) {
                    row (gap: 2) {
                        text (content: "o") style (fg: primary)
                        text (content: "open file") style (fg: muted)
                    }
                    row (gap: 2) {
                        button (label: "Close", hint: "esc", id: "close") on_activate: close()
                    }
                }
            }
        }
    }
}
