//! List view component for the Explorer app.

use rafter::prelude::*;

use crate::FileEntry;

/// Renders the list view of files.
pub struct ListView;

impl ListView {
    /// Render the list view.
    pub fn render(files: &[FileEntry], selected: usize, status: &str) -> Node {
        let file_count = files.len();
        let status_str = status.to_string();

        // Prepare file data for iteration
        let indexed_files: Vec<(usize, &FileEntry)> = files.iter().enumerate().collect();

        view! {
            column (padding: 1, gap: 1, bg: background) {
                // Header
                row {
                    text (bold, fg: primary) { "Explorer" }
                    text (fg: muted) { " - File Browser" }
                }

                // File list header
                row (gap: 2) {
                    text (fg: muted, width: 24) { "Name" }
                    text (fg: muted, width: 10) { "Size" }
                }

                // File entries
                column (border: rounded) {
                    for (i, file) in indexed_files {
                        if i == selected {
                            // Selected row
                            row (gap: 2, bg: surface) {
                                if file.is_dir {
                                    text (bold, fg: secondary, width: 24) { format!("📁 {}", file.name) }
                                } else {
                                    text (bold, fg: primary, width: 24) { format!("📄 {}", file.name) }
                                }
                                text (fg: muted, width: 10) { file.size_display() }
                            }
                        } else {
                            // Normal row
                            row (gap: 2) {
                                if file.is_dir {
                                    text (fg: secondary, width: 24) { format!("📁 {}", file.name) }
                                } else {
                                    text (width: 24) { format!("📄 {}", file.name) }
                                }
                                text (fg: muted, width: 10) { file.size_display() }
                            }
                        }
                    }
                }

                // Status bar
                row (gap: 2) {
                    text (fg: muted) { format!("{} items", file_count) }
                    text (fg: info) { status_str }
                }

                // Help
                text (fg: muted) { "j/k:move  enter:open  d:delete  r:rename  n:new  ?:help  q:quit" }
            }
        }
    }
}
