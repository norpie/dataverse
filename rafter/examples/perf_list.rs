//! Performance List Example
//!
//! Demonstrates virtualization performance with large lists:
//! - Three buttons to populate list with 1K, 10K, or 100K items
//! - Shows render timing in status bar
//! - Tests scrolling performance with different list sizes

use std::fs::File;

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, List, ListItem, ListState, Text};
use simplelog::{Config, LevelFilter, WriteLogger};
use tuidom::{Element, Style};

/// A simple list item for performance testing.
#[derive(Clone, Debug)]
struct PerfItem {
    id: u64,
    label: String,
}

impl ListItem for PerfItem {
    type Key = u64;

    fn key(&self) -> u64 {
        self.id
    }

    fn render(&self) -> Element {
        Element::row()
            .gap(2)
            .child(Element::text(&format!("#{:>6}", self.id + 1)).style(Style::new().bold()))
            .child(Element::text(&self.label))
    }
}

/// Generate a list of items.
fn generate_items(count: usize) -> Vec<PerfItem> {
    (0..count as u64)
        .map(|id| PerfItem {
            id,
            label: format!("Item {} {}", id + 1, "X".repeat(500)),
        })
        .collect()
}

#[app]
struct PerfListExample {
    items: ListState<PerfItem>,
    item_count: usize,
    status: String,
}

#[app_impl]
impl PerfListExample {
    async fn on_start(&self) {
        self.status.set("Select a button to populate the list".into());
    }

    #[keybinds]
    fn keys() {
        bind("q", quit);
        bind("1", load_1k);
        bind("2", load_10k);
        bind("3", load_100k);
        bind("4", load_50);
        bind("c", clear);
    }

    #[handler]
    async fn quit(&self, gx: &GlobalContext) {
        gx.shutdown();
    }

    #[handler]
    async fn load_50(&self) {
        self.load_items(50);
    }

    #[handler]
    async fn load_1k(&self) {
        self.load_items(1_000);
    }

    #[handler]
    async fn load_10k(&self) {
        self.load_items(10_000);
    }

    #[handler]
    async fn load_100k(&self) {
        self.load_items(100_000);
    }

    #[handler]
    async fn clear(&self) {
        self.items.set(ListState::default());
        self.item_count.set(0);
        self.status.set("List cleared".into());
    }

    fn load_items(&self, count: usize) {
        self.status.set(format!("Generating {} items...", count));
        let items = generate_items(count);
        self.items.set(ListState::new(items));
        self.item_count.set(count);
        self.status.set(format!("Loaded {} items - scroll to test performance", count));
    }

    fn element(&self) -> Element {
        let count = self.item_count.get();
        let status = self.status.get();

        page! {
            column (padding: 0, gap: 1, height: fill, width: fill) {
                // Header
                text (content: "Performance List Example") style (bold)
                text (content: "Test virtualization with large lists")

                // Buttons row
                row (gap: 2) {
                    button (id: "btn-1k", label: "1K Items") on_activate: load_1k()
                    button (id: "btn-10k", label: "10K Items") on_activate: load_10k()
                    button (id: "btn-100k", label: "100K Items") on_activate: load_100k()
                    button (id: "btn-50", label: "50 Items") on_activate: load_50()
                    button (id: "btn-clear", label: "Clear") on_activate: clear()
                }

                // Status bar
                row (gap: 2) {
                    text (content: "Status:")
                    text (content: {status})
                    text (content: {format!("[{} items]", count)})
                }

                // Virtualized list (has its own scrollbar, horizontal scroll enabled)
                list (state: self.items, id: "perf-list", height: fill, horizontal_scroll: true)

                // Footer
                row (gap: 2) {
                    text (content: "Keys: q=quit, 1/2/3=load 1K/10K/100K, c=clear")
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Set up file logging
    let log_file = File::create("perf_list.log").expect("Failed to create log file");
    WriteLogger::init(LevelFilter::Debug, Config::default(), log_file)
        .expect("Failed to initialize logger");

    if let Err(e) = Runtime::new()
        .expect("Failed to create runtime")
        .run(PerfListExample::default())
        .await
    {
        eprintln!("Error: {}", e);
    }
}
