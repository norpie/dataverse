//! List Widget Example
//!
//! Demonstrates the List widget with selectable items.

use std::fs::File;

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{List, ListItem, ListState, SelectionMode, Text};
use simplelog::{Config, LevelFilter, WriteLogger};
use tuidom::Element;

/// A simple file item for the list.
#[derive(Clone, Debug)]
struct FileItem {
    path: String,
    name: String,
    size: u64,
}

impl ListItem for FileItem {
    type Key = String;

    fn key(&self) -> String {
        self.path.clone()
    }

    fn render(&self) -> Element {
        Element::row()
            .width(tuidom::Size::Fill)
            .justify(tuidom::Justify::SpaceBetween)
            .children(vec![
                Element::text(&self.name),
                Element::text(&format!("{}KB", self.size / 1024)),
            ])
    }
}

#[app]
struct ListExample {
    files: ListState<FileItem>,
    files2: ListState<FileItem>,
    message: String,
}

#[app_impl]
impl ListExample {
    async fn on_start(&self) {
        // Initialize file list with sample data
        let files: Vec<FileItem> = (1..=100)
            .map(|i| FileItem {
                path: format!("/home/user/file{}.txt", i),
                name: format!("file{}.txt", i),
                size: i as u64 * 1024,
            })
            .collect();

        self.files
            .set(ListState::new(files.clone()).with_selection(SelectionMode::Multi));
        self.files2
            .set(ListState::new(files).with_selection(SelectionMode::Multi));
        self.message.set("Select files from the list".into());
    }

    #[keybinds]
    fn keys() {
        bind("q", quit);
    }

    #[handler]
    async fn quit(&self, gx: &GlobalContext) {
        gx.shutdown();
    }

    #[handler]
    async fn file_selected(&self) {
        let state = self.files.get();
        if let Some(key) = &state.last_activated {
            let selected_count = state.selection.selected.len();
            self.message
                .set(format!("Toggled: {} ({} selected)", key, selected_count));
        }
    }

    #[handler]
    async fn file_activated(&self, gx: &GlobalContext) {
        let state = self.files.get();
        if let Some(key) = &state.last_activated {
            gx.toast(Toast::info(format!("Activated: {}", key)));
        }
    }

    fn element(&self) -> Element {
        let message = self.message.get();

        page! {
            column (padding: 2, gap: 1, height: fill, width: fill) style (bg: background) {
                // Header - testing derived colors
                text (content: "List Widget Demo") style (bold, fg: accent | lighten(0.1))
                text (content: "Use Tab/arrows to navigate, Enter/Space to select") style (fg: primary | darken(0.3))

                // Status
                row (gap: 1) {
                    text (content: "Status:") style (fg: muted)
                    text (content: {message}) style (fg: accent | darken(0.1))
                }

                // Top list - shrink to content width
                box_ (id: "file-scroll", height: fill, overflow: auto) style (bg: surface) {
                    list (state: self.files, id: "file-list")
                        on_select: file_selected()
                        on_activate: file_activated()
                }

                // Bottom list - fill entire width
                box_ (id: "file-scroll-2", height: fill, width: fill, overflow: auto) style (bg: surface) {
                    list (state: self.files2, id: "file-list-2")
                        on_select: file_selected()
                        on_activate: file_activated()
                }

                // Footer
                text (content: "Press 'q' to quit") style (fg: primary | darken(0.4))
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Set up file logging
    let log_file = File::create("list.log").expect("Failed to create log file");
    WriteLogger::init(LevelFilter::Debug, Config::default(), log_file)
        .expect("Failed to initialize logger");

    if let Err(e) = Runtime::new()
        .expect("Failed to create runtime")
        .run(ListExample::default())
        .await
    {
        eprintln!("Error: {}", e);
    }
}
