//! Import app for importing data from CSV/Excel files into Dataverse.

mod io;
mod modals;

use std::path::PathBuf;

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Text};

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
        gx.toast(Toast::info("File browser not implemented yet"));
    }

    fn element(&self) -> Element {
        let has_file = self.file_path.with_ref(|p| p.is_some());

        page! {
            column (padding: (1, 2), gap: 1, height: fill, width: fill) style (bg: background) {
                // Title
                text (content: "Import") style (fg: interact)

                // Content
                box_ (height: fill, width: fill) style (bg: surface) {
                    if has_file {
                        column (padding: (1, 2), gap: 1) {
                            text (content: "File loaded (implementation pending)") style (fg: muted)
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
