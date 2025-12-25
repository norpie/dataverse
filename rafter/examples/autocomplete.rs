//! Autocomplete Widget Example
//!
//! Demonstrates the Autocomplete widget for fuzzy-filtered text input:
//! - Type to filter suggestions
//! - Keyboard navigation (Up/Down to navigate, Enter to select, Escape to close)
//! - Static and dynamic options
//! - Free-form input (can enter values not in the list)

use std::fs::File;

use log::LevelFilter;
use rafter::prelude::*;
use simplelog::{Config, WriteLogger};

// ============================================================================
// Country data implementing AutocompleteItem
// ============================================================================

#[derive(Clone, Debug)]
struct Country {
    code: String,
    name: String,
}

impl AutocompleteItem for Country {
    fn autocomplete_id(&self) -> String {
        self.code.clone()
    }

    fn autocomplete_label(&self) -> String {
        self.name.clone()
    }
}

// ============================================================================
// Autocomplete Demo App
// ============================================================================

#[app]
struct AutocompleteDemo {
    // Simple string autocomplete
    fruit: Autocomplete,
    // Custom struct autocomplete
    country: Autocomplete,
    // Track events for display
    last_event: String,
}

#[app_impl]
impl AutocompleteDemo {
    async fn on_start(&self, _cx: &AppContext) {
        self.fruit.set_placeholder("Type to search fruits...");
        self.country.set_placeholder("Search countries...");
        self.last_event.set("(none)".to_string());
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
        let value = self.fruit.value();
        self.last_event
            .set(format!("Fruit input changed: \"{}\"", value));
    }

    #[handler]
    async fn on_fruit_select(&self, _cx: &AppContext) {
        let value = self.fruit.value();
        self.last_event
            .set(format!("Fruit selected: \"{}\"", value));
    }

    #[handler]
    async fn on_country_change(&self, _cx: &AppContext) {
        let value = self.country.value();
        self.last_event
            .set(format!("Country input changed: \"{}\"", value));
    }

    #[handler]
    async fn on_country_select(&self, _cx: &AppContext) {
        let value = self.country.value();
        self.last_event
            .set(format!("Country selected: \"{}\"", value));
    }

    fn page(&self) -> Node {
        let fruits = vec![
            "Apple",
            "Apricot",
            "Avocado",
            "Banana",
            "Blackberry",
            "Blueberry",
            "Cherry",
            "Coconut",
            "Date",
            "Elderberry",
            "Fig",
            "Grape",
            "Grapefruit",
            "Guava",
            "Kiwi",
            "Lemon",
            "Lime",
            "Mango",
            "Melon",
            "Nectarine",
            "Orange",
            "Papaya",
            "Peach",
            "Pear",
            "Pineapple",
            "Plum",
            "Pomegranate",
            "Raspberry",
            "Strawberry",
            "Watermelon",
        ];

        let countries = get_countries();

        let fruit_value = {
            let v = self.fruit.value();
            if v.is_empty() { "(empty)".to_string() } else { v }
        };
        let country_value = {
            let v = self.country.value();
            if v.is_empty() { "(empty)".to_string() } else { v }
        };
        let last_event = self.last_event.get();

        page! {
            column (padding: 2, gap: 2, bg: background) {
                // Title
                column {
                    text (bold, fg: primary) { "Autocomplete Widget Demo" }
                    text (fg: muted) { "Type to filter, Up/Down to navigate, Enter to select" }
                }

                // Simple string autocomplete
                column (gap: 1) {
                    text (bold) { "Fruit Search:" }
                    autocomplete(
                        bind: self.fruit,
                        options: fruits,
                        on_change: on_fruit_change,
                        on_select: on_fruit_select
                    )
                    row (gap: 1) {
                        text (fg: muted) { "Current value:" }
                        text (fg: success) { fruit_value }
                    }
                }

                // Custom struct autocomplete
                column (gap: 1) {
                    text (bold) { "Country Search:" }
                    autocomplete(
                        bind: self.country,
                        options: countries,
                        on_change: on_country_change,
                        on_select: on_country_select
                    )
                    row (gap: 1) {
                        text (fg: muted) { "Current value:" }
                        text (fg: warning) { country_value }
                    }
                }

                // Event log
                column (gap: 1) {
                    text (bold, fg: info) { "Last Event:" }
                    text (fg: muted) { last_event }
                }

                // Help text
                text (fg: muted) { "Tab to switch fields, q to quit" }
            }
        }
    }
}

fn get_countries() -> Vec<Country> {
    vec![
        Country { code: "US".into(), name: "United States".into() },
        Country { code: "GB".into(), name: "United Kingdom".into() },
        Country { code: "CA".into(), name: "Canada".into() },
        Country { code: "AU".into(), name: "Australia".into() },
        Country { code: "DE".into(), name: "Germany".into() },
        Country { code: "FR".into(), name: "France".into() },
        Country { code: "JP".into(), name: "Japan".into() },
        Country { code: "CN".into(), name: "China".into() },
        Country { code: "IN".into(), name: "India".into() },
        Country { code: "BR".into(), name: "Brazil".into() },
        Country { code: "MX".into(), name: "Mexico".into() },
        Country { code: "ES".into(), name: "Spain".into() },
        Country { code: "IT".into(), name: "Italy".into() },
        Country { code: "NL".into(), name: "Netherlands".into() },
        Country { code: "SE".into(), name: "Sweden".into() },
        Country { code: "NO".into(), name: "Norway".into() },
        Country { code: "DK".into(), name: "Denmark".into() },
        Country { code: "FI".into(), name: "Finland".into() },
        Country { code: "PL".into(), name: "Poland".into() },
        Country { code: "RU".into(), name: "Russia".into() },
    ]
}

#[tokio::main]
async fn main() {
    // Initialize file logging
    if let Ok(log_file) = File::create("autocomplete.log") {
        let _ = WriteLogger::init(LevelFilter::Debug, Config::default(), log_file);
    }

    if let Err(e) = rafter::Runtime::new()
        .initial::<AutocompleteDemo>()
        .run()
        .await
    {
        eprintln!("Error: {}", e);
    }
}
