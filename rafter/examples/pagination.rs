//! Pagination Example
//!
//! Demonstrates infinite scroll / cursor-based pagination:
//! - Loading items in batches
//! - Using on_scroll to detect when near bottom
//! - Automatically loading more items
//! - Showing loading indicator
//! - Simulating async data fetching with cursors

use std::fs::File;
use std::time::Duration;

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{List, ListItem, ListState, Text};
use rafter::EventData;
use simplelog::{Config, LevelFilter, WriteLogger};
use tuidom::{Element, Style};

/// Simulated record from an API.
#[derive(Clone, Debug)]
struct Record {
    id: u64,
    title: String,
    description: String,
}

impl ListItem for Record {
    type Key = u64;

    fn key(&self) -> u64 {
        self.id
    }

    fn render(&self) -> Element {
        Element::row()
            .gap(2)
            .child(Element::text(&format!("#{}", self.id + 1)).style(Style::new().bold()))
            .child(Element::text(&self.title))
            .child(Element::text(&self.description))
    }
}

/// Simulated API response with cursor-based pagination.
struct ApiResponse {
    records: Vec<Record>,
    next_cursor: Option<u64>,
}

/// Simulate fetching records from an API.
async fn fetch_records(cursor: Option<u64>, limit: usize) -> ApiResponse {
    // Simulate network latency
    tokio::time::sleep(Duration::from_millis(500)).await;

    let start = cursor.unwrap_or(0);
    let total_records = 1000; // Simulate 1000 total records

    let records: Vec<Record> = (start..start + limit as u64)
        .filter(|&id| id < total_records)
        .map(|id| Record {
            id,
            title: format!("Record #{}", id + 1),
            description: format!("This is the description for record {}", id + 1),
        })
        .collect();

    let next_cursor = if start + (limit as u64) < total_records {
        Some(start + limit as u64)
    } else {
        None
    };

    ApiResponse {
        records,
        next_cursor,
    }
}

#[app]
struct PaginationExample {
    records: ListState<Record>,
    next_cursor: Option<u64>,
    loading: bool,
    total_loaded: usize,
    message: String,
}

#[app_impl]
impl PaginationExample {
    async fn on_start(&self) {
        self.message.set("Loading initial records...".into());
        self.loading.set(true);

        // Load initial batch (50 to ensure scrolling is needed)
        let response = fetch_records(None, 50).await;
        self.records.set(ListState::new(response.records));
        self.next_cursor.set(response.next_cursor);
        self.total_loaded.set(50);
        self.loading.set(false);
        self.message
            .set("Scroll down to load more records".into());
    }

    #[keybinds]
    fn keys() {
        bind("q", quit);
        bind("r", reload);
    }

    #[handler]
    async fn quit(&self, gx: &GlobalContext) {
        gx.shutdown();
    }

    #[handler]
    async fn reload(&self) {
        // Reset and reload from beginning
        self.records.set(ListState::default());
        self.next_cursor.set(None);
        self.loading.set(true);
        self.message.set("Reloading...".into());

        let response = fetch_records(None, 50).await;
        self.records.set(ListState::new(response.records));
        self.next_cursor.set(response.next_cursor);
        self.total_loaded.set(50);
        self.loading.set(false);
        self.message
            .set("Scroll down to load more records".into());
    }

    #[handler]
    async fn on_scroll(&self, event: &EventData) {
        // Check if we're near the bottom and should load more (80% scrolled)
        if event.is_near_bottom(0.8) {
            self.maybe_load_more().await;
        }

        // Update message with scroll progress
        if let EventData::Scroll {
            offset_y,
            content_height,
            viewport_height,
            ..
        } = event
        {
            let max_scroll = content_height.saturating_sub(*viewport_height);
            if max_scroll > 0 {
                let progress = (*offset_y as f32 / max_scroll as f32 * 100.0) as u8;
                if !self.loading.get() {
                    self.message.set(format!(
                        "Scroll: {}% ({} records loaded)",
                        progress,
                        self.total_loaded.get()
                    ));
                }
            }
        }
    }

    async fn maybe_load_more(&self) {
        // Don't load if already loading
        if self.loading.get() {
            return;
        }

        // Don't load if no more pages
        let cursor = self.next_cursor.get();
        if cursor.is_none() {
            self.message.set(format!(
                "All {} records loaded!",
                self.total_loaded.get()
            ));
            return;
        }

        // Start loading
        self.loading.set(true);
        self.message.set("Loading more records...".into());

        // Fetch next page
        let response = fetch_records(cursor, 20).await;
        let new_count = response.records.len();

        // Append to existing records
        self.records.update(|state| {
            state.items.extend(response.records);
        });
        self.next_cursor.set(response.next_cursor);
        self.total_loaded.update(|n| *n += new_count);
        self.loading.set(false);

        let has_more = self.next_cursor.get().is_some();
        if has_more {
            self.message.set(format!(
                "Loaded {} records. Scroll for more...",
                self.total_loaded.get()
            ));
        } else {
            self.message.set(format!(
                "All {} records loaded!",
                self.total_loaded.get()
            ));
        }
    }

    fn element(&self) -> Element {
        let loading = self.loading.get();
        let message = self.message.get();
        let record_count = self.records.get().items.len();

        page! {
            column (padding: 2, gap: 1, height: fill, width: fill) {
                // Header
                text (content: "Pagination Example") style (bold)
                text (content: "Infinite scroll with cursor-based pagination")

                // Status bar
                row (gap: 2) {
                    text (content: "Status:")
                    text (content: {message})
                    text (content: {format!("[{} records]", record_count)})
                    if loading {
                        text (content: "[Loading...]") style (bold)
                    }
                }

                // Scrollable record list
                box_ (id: "record-scroll", height: fill, width: fill, overflow: auto)
                    on_scroll: on_scroll()
                {
                    list (state: self.records, id: "record-list")
                }

                // Footer
                row (gap: 2) {
                    text (content: "Press 'q' to quit, 'r' to reload")
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Set up file logging
    let log_file = File::create("pagination.log").expect("Failed to create log file");
    WriteLogger::init(LevelFilter::Debug, Config::default(), log_file)
        .expect("Failed to initialize logger");

    if let Err(e) = Runtime::new()
        .expect("Failed to create runtime")
        .run(PaginationExample::default())
        .await
    {
        eprintln!("Error: {}", e);
    }
}
