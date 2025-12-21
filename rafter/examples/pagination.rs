//! Pagination example - demonstrates infinite scroll with List<T>
//!
//! This example shows how to implement cursor-based pagination with a List component.
//! Records are loaded in pages of 20, with more loaded automatically when scrolling
//! near the end of the list.
//!
//! Controls:
//! - j/k or arrows: Navigate up/down
//! - PageUp/PageDown: Navigate by page
//! - Mouse wheel: Scroll
//! - r: Reload from start
//! - q: Quit

use std::fs::File;

use log::LevelFilter;
use rafter::color::Color;
use rafter::prelude::*;
use rafter::theme::{DefaultTheme, Theme};
use simplelog::{Config, WriteLogger};

// =============================================================================
// Data types
// =============================================================================

/// A record from our "API"
#[derive(Debug, Clone)]
struct Record {
    id: u32,
    name: String,
    value: i32,
}

impl ListItem for Record {
    fn render(&self, focused: bool, _selected: bool) -> Node {
        let content = format!("#{:04}  {:30}  {:>8}", self.id, self.name, self.value);

        if focused {
            view! {
                row (flex: 1, bg: surface) {
                    text (fg: on_surface) { content }
                }
            }
        } else {
            view! {
                row (flex: 1) {
                    text { content }
                }
            }
        }
    }
}

// =============================================================================
// Simulated API
// =============================================================================

const PAGE_SIZE: u32 = 20;
const TOTAL_RECORDS: u32 = 150;

/// Simulated API that returns paginated results
async fn fetch_page(offset: u32) -> (Vec<Record>, Option<String>) {
    // Simulate network delay
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let items: Vec<Record> = (offset..offset + PAGE_SIZE)
        .filter(|&id| id < TOTAL_RECORDS)
        .map(|id| Record {
            id,
            name: format!("Record {}", id),
            value: ((id as i32 * 17) % 1000) - 500,
        })
        .collect();

    let next_offset = offset + PAGE_SIZE;
    let next_cursor = if next_offset < TOTAL_RECORDS {
        Some(next_offset.to_string())
    } else {
        None
    };

    (items, next_cursor)
}

// =============================================================================
// Theme
// =============================================================================

#[derive(Debug, Clone)]
struct PaginationTheme {
    inner: DefaultTheme,
}

impl PaginationTheme {
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
            },
        }
    }
}

impl Theme for PaginationTheme {
    fn resolve(&self, name: &str) -> Option<Color> {
        // Add on_surface for focused items
        if name == "on_surface" {
            return Some(Color::rgb(255, 255, 255));
        }
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
struct PaginationApp {
    records: List<Record>,
    next_cursor: Option<String>,
    loading: bool,
    total_loaded: usize,
}

#[app_impl]
impl PaginationApp {
    async fn on_start(&self, _cx: &AppContext) {
        self.load_initial().await;
    }

    async fn load_initial(&self) {
        self.loading.set(true);
        self.records.clear();

        let (items, next_cursor) = fetch_page(0).await;

        let count = items.len();
        for item in items {
            self.records.push(item);
        }
        self.records.set_cursor(0);
        self.next_cursor.set(next_cursor);
        self.total_loaded.set(count);
        self.loading.set(false);
    }

    async fn maybe_load_more(&self) {
        // Don't load if already loading
        if self.loading.get() {
            return;
        }

        // Don't load if no more pages
        if self.next_cursor.get().is_none() {
            return;
        }

        // Load more when within 5 items of the end
        if self.records.is_near_end(5) {
            self.load_more().await;
        }
    }

    async fn load_more(&self) {
        let cursor = match self.next_cursor.get().clone() {
            Some(c) => c,
            None => return,
        };

        self.loading.set(true);

        let offset: u32 = cursor.parse().unwrap_or(0);
        let (items, next_cursor) = fetch_page(offset).await;

        let count = items.len();
        for item in items {
            self.records.push(item);
        }
        self.next_cursor.set(next_cursor);
        self.total_loaded.update(|n| *n += count);
        self.loading.set(false);
    }

    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "q" => quit,
            "r" => reload,
        }
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        cx.exit();
    }

    #[handler]
    async fn reload(&self, cx: &AppContext) {
        cx.toast("Reloading...");
        self.load_initial().await;
        cx.toast("Reloaded");
    }

    #[handler]
    async fn on_scroll(&self, _cx: &AppContext) {
        self.maybe_load_more().await;
    }

    #[handler]
    async fn on_cursor_move(&self, _cx: &AppContext) {
        self.maybe_load_more().await;
    }

    fn view(&self) -> Node {
        let loading = self.loading.get();
        let has_more = self.next_cursor.get().is_some();
        let total = self.total_loaded.get();

        // Status text
        let status = if loading && total == 0 {
            "Loading...".to_string()
        } else if loading {
            format!("{} records (loading more...)", total)
        } else if has_more {
            format!("{} records (scroll for more)", total)
        } else {
            format!("{} records (end)", total)
        };

        view! {
            column (bg: background, height: fill, width: fill, padding: 1) {
                // Header
                text (fg: primary, bold: true) { "Pagination Demo" }
                text (fg: muted) { "j/k to navigate, scroll for more, r to reload, q to quit" }
                text { "" }

                // Column headers
                text (fg: muted, bold: true) {
                    format!("{:6}  {:30}  {:>8}", "ID", "NAME", "VALUE")
                }

                // List with scroll handler
                list (
                    bind: self.records,
                    flex: 1,
                    on_scroll: on_scroll,
                    on_cursor_move: on_cursor_move
                )

                // Footer
                text { "" }
                if loading && total == 0 {
                    text (fg: primary) { status.clone() }
                } else if loading {
                    text (fg: warning) { status.clone() }
                } else if has_more {
                    text (fg: muted) { status.clone() }
                } else {
                    text (fg: success) { status.clone() }
                }
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
    if let Ok(log_file) = File::create("pagination.log") {
        let _ = WriteLogger::init(LevelFilter::Debug, Config::default(), log_file);
    }

    if let Err(e) = rafter::Runtime::new()
        .theme(PaginationTheme::new())
        .start_with::<PaginationApp>()
        .await
    {
        eprintln!("Error: {}", e);
    }
}
