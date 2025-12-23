//! Select Widget Example
//!
//! Demonstrates the Select widget for dropdown selection:
//! - Basic select with static options
//! - Dynamic options
//! - Selection change handling
//! - Keyboard navigation (Up/Down to navigate, Enter to select, Escape to close)

use std::fs::File;

use log::LevelFilter;
use rafter::prelude::*;
use simplelog::{Config, WriteLogger};

// ============================================================================
// Priority enum implementing SelectItem
// ============================================================================

#[derive(Clone, Debug)]
struct Priority {
    id: u32,
    name: String,
}

impl SelectItem for Priority {
    fn select_id(&self) -> String {
        self.id.to_string()
    }

    fn select_label(&self) -> String {
        self.name.clone()
    }
}

// ============================================================================
// Select Demo App
// ============================================================================

#[app]
struct SelectDemo {
    // Simple string select
    fruit: Select,
    // Custom struct select
    priority: Select,
    // Track selected values for display
    selected_fruit: String,
    selected_priority: String,
}

#[app_impl]
impl SelectDemo {
    async fn on_start(&self, _cx: &AppContext) {
        // Set placeholders
        self.fruit.set_placeholder("Choose a fruit");
        self.priority.set_placeholder("Select priority");
        self.selected_fruit.set("(none)".to_string());
        self.selected_priority.set("(none)".to_string());
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
    async fn on_fruit_change(&self, _cx: &AppContext) {
        let fruits = vec!["Apple", "Banana", "Cherry", "Date", "Elderberry"];
        if let Some(idx) = self.fruit.selected_index() {
            if let Some(fruit) = fruits.get(idx) {
                self.selected_fruit.set(fruit.to_string());
            }
        } else {
            self.selected_fruit.set("(none)".to_string());
        }
    }

    #[handler]
    async fn on_priority_change(&self, _cx: &AppContext) {
        let priorities = get_priorities();
        if let Some(idx) = self.priority.selected_index() {
            if let Some(p) = priorities.get(idx) {
                self.selected_priority
                    .set(format!("{} (id: {})", p.name, p.id));
            }
        } else {
            self.selected_priority.set("(none)".to_string());
        }
    }

    fn page(&self) -> Node {
        let fruits = vec!["Apple", "Banana", "Cherry", "Date", "Elderberry"];
        let priorities = get_priorities();

        let selected_fruit = self.selected_fruit.get();
        let selected_priority = self.selected_priority.get();

        page! {
            column (padding: 2, gap: 2, bg: background) {
                // Title
                column {
                    text (bold, fg: primary) { "Select Widget Demo" }
                    text (fg: muted) { "Tab to navigate, Enter/Space to open, Up/Down to select" }
                }

                // Simple string select
                column (gap: 1) {
                    text (bold) { "Fruit Selection:" }
                    select(bind: self.fruit, options: fruits, on_change: on_fruit_change)
                    row (gap: 1) {
                        text (fg: muted) { "Selected:" }
                        text (fg: success) { selected_fruit }
                    }
                }

                // Custom struct select
                column (gap: 1) {
                    text (bold) { "Priority Selection:" }
                    select(bind: self.priority, options: priorities, on_change: on_priority_change)
                    row (gap: 1) {
                        text (fg: muted) { "Selected:" }
                        text (fg: warning) { selected_priority }
                    }
                }

                // Help text
                text (fg: muted) { "q to quit" }
            }
        }
    }
}

fn get_priorities() -> Vec<Priority> {
    vec![
        Priority {
            id: 1,
            name: "Low".to_string(),
        },
        Priority {
            id: 2,
            name: "Medium".to_string(),
        },
        Priority {
            id: 3,
            name: "High".to_string(),
        },
        Priority {
            id: 4,
            name: "Critical".to_string(),
        },
    ]
}

#[tokio::main]
async fn main() {
    // Initialize file logging
    if let Ok(log_file) = File::create("select.log") {
        let _ = WriteLogger::init(LevelFilter::Debug, Config::default(), log_file);
    }

    if let Err(e) = rafter::Runtime::new().initial::<SelectDemo>().run().await {
        eprintln!("Error: {}", e);
    }
}
