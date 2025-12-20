//! Detail view component for the Explorer app.

use rafter::prelude::*;

use crate::FileEntry;

/// Renders the detail view of a single file.
pub struct DetailView;

impl DetailView {
    /// Render the detail view.
    pub fn render(file: &FileEntry, status: &str) -> Node {
        let file_type = if file.is_dir { "Directory" } else { "File" };
        let icon = if file.is_dir { "📁" } else { "📄" };

        view! {
            column (padding: 1, gap: 1, bg: background) {
                // Header with back indicator
                row (gap: 1) {
                    text (fg: muted) { "←" }
                    text (bold, fg: primary) { format!("{} {}", icon, file.name) }
                }

                // File details card
                column (padding: 1, border: rounded, gap: 1) {
                    row (gap: 2) {
                        text (fg: muted, width: 12) { "Type:" }
                        text (fg: text) { file_type }
                    }

                    row (gap: 2) {
                        text (fg: muted, width: 12) { "Size:" }
                        text (fg: text) { file.size_display() }
                    }

                    row (gap: 2) {
                        text (fg: muted, width: 12) { "Name:" }
                        text (fg: text) { file.name.clone() }
                    }
                }

                // Actions hint
                text (fg: muted) { "backspace:back  d:delete  r:rename" }

                // Status bar
                row {
                    text (fg: info) { status }
                }
            }
        }
    }
}
