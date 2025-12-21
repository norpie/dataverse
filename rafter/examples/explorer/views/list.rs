//! List view component for the Explorer app.

use rafter::prelude::*;

use crate::FileEntry;

/// Renders the list view of files.
pub struct ListView;

impl ListView {
    /// Render the list view.
    pub fn render(files: &List<FileEntry>, path: &str, status: &str) -> Node {
        let file_count = files.len();
        let status_str = status.to_string();
        let path_str = path.to_string();
        let files = files.clone();

        view! {
            column (padding: 1, gap: 1, bg: background) {
                // Header with path
                row (gap: 1) {
                    text (bold, fg: primary) { "Explorer" }
                    text (fg: muted) { " - " }
                    text (fg: secondary) { path_str }
                }

                // File list header
                row (gap: 2) {
                    text (fg: muted, width: 24) { "Name" }
                    text (fg: muted, width: 10) { "Size" }
                }

                // File list - using the List component
                list (bind: files, border: rounded, on_activate: open_selected)

                // Status bar
                row (gap: 2) {
                    text (fg: muted) { format!("{} items", file_count) }
                    text (fg: info) { status_str }
                }

                // Help
                text (fg: muted) { "j/k:move  space:select  enter/l:open  h/backspace:back  d:delete  r:rename  n:new  q:quit" }
            }
        }
    }
}
