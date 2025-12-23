//! Table example - demonstrates the Table widget with sorting and selection.
//!
//! This example shows how to use the Table widget with:
//! - Column definitions with fixed widths
//! - TableRow trait implementation
//! - Sorting with on_sort handler
//! - Row selection (multi-select mode)
//! - Horizontal scrolling with many columns
//!
//! Controls:
//! - j/k or arrows: Navigate up/down
//! - h/l or left/right: Scroll horizontally
//! - Space: Toggle selection
//! - a: Select all
//! - Ctrl+a: Deselect all
//! - Enter: Activate selected row(s)
//! - Click on column header: Sort by that column
//! - q: Quit

use std::cmp::Ordering;
use std::fs::File;

use log::LevelFilter;
use rafter::prelude::*;
use rafter::styling::color::Color;
use rafter::styling::theme::{DefaultTheme, Theme};
use simplelog::{Config, WriteLogger};

// =============================================================================
// Data types
// =============================================================================

/// A user record for the table.
#[derive(Debug, Clone)]
struct User {
    id: u32,
    name: String,
    email: String,
    age: u32,
    status: String,
    score: i32,
}

impl User {
    fn new(id: u32) -> Self {
        let names = [
            "Alice", "Bob", "Charlie", "Diana", "Eve", "Frank", "Grace", "Henry",
        ];
        let domains = ["example.com", "test.org", "demo.net", "sample.io"];
        let statuses = ["Active", "Pending", "Inactive", "Suspended"];

        let name = names[id as usize % names.len()].to_string();
        let domain = domains[id as usize % domains.len()];
        let status = statuses[id as usize % statuses.len()].to_string();

        Self {
            id,
            name: format!("{} {}", name, id),
            email: format!("{}.{}@{}", name.to_lowercase(), id, domain),
            age: 20 + (id % 50),
            status,
            score: ((id as i32 * 17) % 200) - 100,
        }
    }

    /// Get column definitions for the table.
    fn columns() -> Vec<Column> {
        vec![
            Column::new("ID", 8).sortable(),
            Column::new("Name", 20).sortable(),
            Column::new("Email", 30).sortable(),
            Column::new("Age", 8).align(Alignment::Right).sortable(),
            Column::new("Status", 12)
                .align(Alignment::Center)
                .sortable(),
            Column::new("Score", 10).align(Alignment::Right).sortable(),
        ]
    }
}

impl TableRow for User {
    fn id(&self) -> String {
        self.id.to_string()
    }

    fn column_count(&self) -> usize {
        6
    }

    fn render_cell(&self, column_index: usize, _focused: bool, _selected: bool) -> Option<Node> {
        let content = match column_index {
            0 => page! { text { format!("{:04}", self.id) } },
            1 => page! { text { self.name.clone() } },
            2 => page! { text { self.email.clone() } },
            3 => page! { text { self.age.to_string() } },
            4 => {
                // Color-code status
                match self.status.as_str() {
                    "Active" => page! { text (fg: success) { self.status.clone() } },
                    "Pending" => page! { text (fg: warning) { self.status.clone() } },
                    "Inactive" => page! { text (fg: muted) { self.status.clone() } },
                    "Suspended" => page! { text (fg: error) { self.status.clone() } },
                    _ => page! { text { self.status.clone() } },
                }
            }
            5 => {
                // Color-code score (positive=green, negative=red)
                if self.score >= 0 {
                    page! { text (fg: success) { format!("{:+}", self.score) } }
                } else {
                    page! { text (fg: error) { format!("{:+}", self.score) } }
                }
            }
            _ => return None,
        };
        Some(content)
    }
}

// =============================================================================
// Theme
// =============================================================================

#[derive(Debug, Clone)]
struct TableTheme {
    inner: DefaultTheme,
}

impl TableTheme {
    fn new() -> Self {
        Self {
            inner: DefaultTheme {
                primary: Color::rgb(78, 204, 163),     // Teal
                secondary: Color::rgb(100, 150, 255),  // Light blue
                background: Color::rgb(26, 26, 46),    // Dark blue
                surface: Color::rgb(40, 40, 70),       // Lighter blue
                text: Color::rgb(232, 232, 232),       // Off-white
                text_muted: Color::rgb(127, 140, 141), // Gray
                error: Color::rgb(231, 76, 60),        // Red
                success: Color::rgb(46, 204, 113),     // Green
                warning: Color::rgb(241, 196, 15),     // Yellow
                info: Color::rgb(52, 152, 219),        // Blue
                validation_error: Color::rgb(231, 76, 60),
                validation_error_border: Color::rgb(231, 76, 60),
            },
        }
    }
}

impl Theme for TableTheme {
    fn resolve(&self, name: &str) -> Option<Color> {
        self.inner.resolve(name)
    }

    fn color_names(&self) -> Vec<&'static str> {
        self.inner.color_names()
    }

    fn clone_box(&self) -> Box<dyn Theme> {
        Box::new(self.clone())
    }
}

// =============================================================================
// App
// =============================================================================

#[app]
struct TableApp {
    users: Table<User>,
    sort_column: Option<usize>,
    sort_ascending: bool,
}

#[app_impl]
impl TableApp {
    async fn on_start(&self, _cx: &AppContext) {
        // Create initial data
        let users: Vec<User> = (1..=50).map(User::new).collect();

        // Set the data and selection mode
        self.users.set_columns(User::columns());
        self.users.set_rows(users);
        self.users.set_selection_mode(SelectionMode::Multiple);
        self.users.set_cursor(0);
    }

    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "q" => quit,
        }
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        cx.exit();
    }

    #[handler]
    async fn on_activate(&self, cx: &AppContext) {
        // Read from widget state (context is cleared after dispatch)
        if let Some(id) = self.users.cursor_id() {
            cx.show_toast(Toast::info(format!("Activated row: {}", id)));
        }
    }

    #[handler]
    async fn on_selection_change(&self, cx: &AppContext) {
        let selected = self.users.selected_ids();
        cx.toast(format!("{} row(s) selected", selected.len()));
    }

    #[handler]
    async fn on_sort(&self, _cx: &AppContext) {
        // Read sort state from the table widget
        // (Context value is cleared after dispatch, so we read from widget state)
        let Some((column, ascending)) = self.users.sort() else {
            return;
        };

        self.sort_column.set(Some(column));
        self.sort_ascending.set(ascending);

        // Get current rows and sort them
        let mut users = self.users.rows();
        users.sort_by(|a, b| {
            let ord = match column {
                0 => a.id.cmp(&b.id),
                1 => a.name.cmp(&b.name),
                2 => a.email.cmp(&b.email),
                3 => a.age.cmp(&b.age),
                4 => a.status.cmp(&b.status),
                5 => a.score.cmp(&b.score),
                _ => Ordering::Equal,
            };
            if ascending { ord } else { ord.reverse() }
        });

        // Update table with sorted rows
        self.users.set_rows(users);
    }

    fn page(&self) -> Node {
        let selected_count = self.users.selected_ids().len();
        let total_count = self.users.len();

        let sort_info = if let Some(col) = self.sort_column.get() {
            let col_name = ["ID", "Name", "Email", "Age", "Status", "Score"][col];
            let dir = if self.sort_ascending.get() {
                "▲"
            } else {
                "▼"
            };
            format!("Sorted by: {} {}", col_name, dir)
        } else {
            "Not sorted (click header to sort)".to_string()
        };

        page! {
            column (bg: background, height: fill, width: fill, padding: 1) {
                // Header
                text (fg: primary, bold: true) { "Table Demo" }
                text (fg: muted) { "j/k to navigate, Space to select, click header to sort, q to quit" }
                text { "" }

                // Status bar
                row (gap: 4) {
                    text (fg: info) { format!("Rows: {}", total_count) }
                    text (fg: warning) { format!("Selected: {}", selected_count) }
                    text (fg: muted) { sort_info }
                }
                text { "" }

                // The table
                table (
                    bind: self.users,
                    flex: 1,
                    on_activate: on_activate,
                    on_selection_change: on_selection_change,
                    on_sort: on_sort
                )

                // Footer with keybinds
                text { "" }
                text (fg: muted) { "Space: toggle select | a: select all | Enter: activate | h/l: scroll horizontally" }
            }
        }
    }
}

// =============================================================================
// Main
// =============================================================================

#[tokio::main]
async fn main() {
    // Initialize file logging
    if let Ok(log_file) = File::create("table.log") {
        let _ = WriteLogger::init(LevelFilter::Debug, Config::default(), log_file);
    }

    if let Err(e) = rafter::Runtime::new()
        .theme(TableTheme::new())
        .initial::<TableApp>()
        .run()
        .await
    {
        eprintln!("Error: {}", e);
    }
}
