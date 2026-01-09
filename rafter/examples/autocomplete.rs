//! Autocomplete Example
//!
//! Demonstrates the Autocomplete widget with fuzzy filtering:
//! - Type to filter options
//! - Fuzzy matching (e.g., "us" matches "United States")
//! - Selection updates the text field
//! - Shows selected value

use std::fs::File;

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Autocomplete, AutocompleteState, Text};
use simplelog::{Config, LevelFilter, WriteLogger};

#[app]
struct AutocompleteDemo {
    country: AutocompleteState<String>,
    message: String,
}

#[app_impl]
impl AutocompleteDemo {
    async fn on_start(&self) {
        self.message.set("Type to search for a country...".into());

        // Initialize autocomplete with many countries
        self.country.set(AutocompleteState::new([
            ("us".to_string(), "United States"),
            ("uk".to_string(), "United Kingdom"),
            ("de".to_string(), "Germany"),
            ("fr".to_string(), "France"),
            ("es".to_string(), "Spain"),
            ("it".to_string(), "Italy"),
            ("nl".to_string(), "Netherlands"),
            ("be".to_string(), "Belgium"),
            ("se".to_string(), "Sweden"),
            ("no".to_string(), "Norway"),
            ("dk".to_string(), "Denmark"),
            ("fi".to_string(), "Finland"),
            ("pl".to_string(), "Poland"),
            ("pt".to_string(), "Portugal"),
            ("at".to_string(), "Austria"),
            ("ch".to_string(), "Switzerland"),
            ("ie".to_string(), "Ireland"),
            ("gr".to_string(), "Greece"),
            ("cz".to_string(), "Czech Republic"),
            ("hu".to_string(), "Hungary"),
            ("jp".to_string(), "Japan"),
            ("cn".to_string(), "China"),
            ("kr".to_string(), "South Korea"),
            ("au".to_string(), "Australia"),
            ("nz".to_string(), "New Zealand"),
            ("ca".to_string(), "Canada"),
            ("mx".to_string(), "Mexico"),
            ("br".to_string(), "Brazil"),
            ("ar".to_string(), "Argentina"),
            ("za".to_string(), "South Africa"),
        ]));
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
    async fn country_selected(&self) {
        let state = self.country.get();
        if let Some(code) = &state.value {
            let label = state
                .options
                .iter()
                .find(|(v, _)| v == code)
                .map(|(_, l)| l.as_str())
                .unwrap_or("Unknown");
            self.message.set(format!("Selected: {} ({})", label, code));
        }
    }

    #[handler]
    async fn country_changed(&self) {
        let state = self.country.get();
        self.message.set(format!(
            "Searching: '{}' ({} matches)",
            state.text,
            state.filtered.len()
        ));
    }

    fn element(&self) -> Element {
        let message = self.message.get();
        let state = self.country.get();

        page! {
            column (padding: 2, gap: 2) style (bg: background) {
                // Header
                text (content: "Autocomplete Demo") style (bold, fg: primary)
                text (content: "Type to fuzzy-search countries") style (fg: muted)

                // Status message
                column (padding: 1) style (bg: surface) {
                    text (content: {message}) style (fg: secondary)
                }

                // Autocomplete widget
                column (gap: 1) {
                    text (content: "Country:") style (fg: muted)
                    autocomplete (state: self.country, id: "country", placeholder: "Search countries...", width: 30)
                        style (bg: surface)
                        on_select: country_selected()
                        on_change: country_changed()
                }

                // Show selected value
                column (gap: 1) {
                    text (content: "Selected Value:") style (fg: muted)
                    text (content: {state.value.clone().unwrap_or_else(|| "(none)".to_string())}) style (fg: accent)
                }

                // Footer
                text (content: "Press 'q' to quit") style (fg: muted)
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Set up file logging
    let log_file = File::create("autocomplete.log").expect("Failed to create log file");
    WriteLogger::init(LevelFilter::Trace, Config::default(), log_file)
        .expect("Failed to initialize logger");

    if let Err(e) = Runtime::new()
        .expect("Failed to create runtime")
        .run(AutocompleteDemo::default())
        .await
    {
        eprintln!("Error: {}", e);
    }
}
