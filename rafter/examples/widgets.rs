//! Widget Showcase Example
//!
//! Demonstrates the available rafter widgets:
//! - Text: Static text display
//! - Button: Clickable buttons with on_activate
//! - Checkbox: Toggleable checkboxes with on_change
//! - Input: Text input fields with on_change and on_submit
//! - Select: Dropdown selection with on_change

use std::fs::File;

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Checkbox, Input, Select, SelectState, Text};
use simplelog::{Config, LevelFilter, WriteLogger};

#[app]
struct WidgetShowcase {
    // Checkbox state
    agree: bool,
    notifications: bool,

    // Input state
    username: String,
    password: String,

    // Select state
    country: SelectState<String>,

    // Display state
    message: String,
}

#[app_impl]
impl WidgetShowcase {
    async fn on_start(&self) {
        self.message.set("Welcome! Try the widgets below.".into());

        // Initialize select options
        let state = SelectState::new([
            ("us".to_string(), "United States"),
            ("uk".to_string(), "United Kingdom"),
            ("de".to_string(), "Germany"),
            ("fr".to_string(), "France"),
        ]);
        log::debug!("on_start: setting country with {} options", state.options.len());
        self.country.set(state);
        log::debug!("on_start: country now has {} options", self.country.get().options.len());
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
    async fn toggle_agree(&self) {
        let value = self.agree.get();
        self.message
            .set(format!("Terms accepted: {}", if value { "Yes" } else { "No" }));
    }

    #[handler]
    async fn toggle_notifications(&self) {
        let value = self.notifications.get();
        self.message.set(format!(
            "Notifications: {}",
            if value { "Enabled" } else { "Disabled" }
        ));
    }

    #[handler]
    async fn country_changed(&self) {
        let state = self.country.get();
        if let Some(code) = &state.value {
            let label = state
                .options
                .iter()
                .find(|(v, _)| v == code)
                .map(|(_, l)| l.as_str())
                .unwrap_or("Unknown");
            self.message.set(format!("Country: {}", label));
        }
    }

    #[handler]
    async fn username_changed(&self) {
        let username = self.username.get();
        self.message.set(format!("Username: {}", username));
    }

    #[handler]
    async fn submit_form(&self, gx: &GlobalContext) {
        let username = self.username.get();
        let agree = self.agree.get();

        if username.is_empty() {
            gx.toast(Toast::error("Username cannot be empty"));
        } else if !agree {
            gx.toast(Toast::warning("Please accept the terms"));
        } else {
            gx.toast(Toast::success(format!("Welcome, {}!", username)));
            self.message.set(format!("Logged in as: {}", username));
        }
    }

    #[handler]
    async fn clear_form(&self) {
        self.username.set(String::new());
        self.password.set(String::new());
        self.agree.set(false);
        self.notifications.set(false);
        self.country.update(|s| s.value = None);
        self.message.set("Form cleared.".into());
    }

    fn element(&self) -> Element {
        let message = self.message.get();

        page! {
            column (padding: 2, gap: 1) style (bg: background) {
                // Header
                text (content: "Widget Showcase") style (bold, fg: primary)
                text (content: "Demonstrating rafter's built-in widgets") style (fg: muted)

                // Status message
                column (padding: 1) style (bg: surface) {
                    row (gap: 1) {
                        text (content: "Status:") style (fg: muted)
                        text (content: {message}) style (fg: secondary)
                    }
                }

                // Input widgets section
                column (gap: 1) {
                    text (content: "Text Inputs") style (bold, fg: primary)
                    row (gap: 2) {
                        text (content: "Username:") style (fg: muted)
                        input (state: self.username, id: "username", placeholder: "Enter username...", width: 30)
                            style (bg: surface)
                            on_change: username_changed()
                            on_submit: submit_form()
                    }
                    row (gap: 2) {
                        text (content: "Password:") style (fg: muted)
                        input (state: self.password, id: "password", placeholder: "Enter password...", width: 30)
                            style (bg: surface)
                    }
                }

                // Checkbox widgets section
                column (gap: 1) {
                    text (content: "Checkboxes") style (bold, fg: primary)
                    checkbox (state: self.agree, id: "agree", label: "I accept the terms", big)
                        on_change: toggle_agree()
                    checkbox (state: self.notifications, id: "notify", label: "Enable notifications", small)
                        on_change: toggle_notifications()
                }

                // Select widgets section
                column (gap: 1) {
                    text (content: "Select") style (bold, fg: primary)
                    row (gap: 2) {
                        text (content: "Country:") style (fg: muted)
                        select (state: self.country, id: "country", placeholder: "Choose country...")
                            style (bg: surface)
                            on_change: country_changed()
                    }
                }

                // Button widgets section
                column (gap: 1) {
                    text (content: "Buttons") style (bold, fg: primary)
                    row (gap: 2) {
                        button (label: "Submit", id: "submit") on_activate: submit_form()
                        button (label: "Clear", id: "clear") on_activate: clear_form()
                    }
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
    let log_file = File::create("widgets.log").expect("Failed to create log file");
    WriteLogger::init(LevelFilter::Debug, Config::default(), log_file)
        .expect("Failed to initialize logger");

    if let Err(e) = Runtime::new()
        .expect("Failed to create runtime")
        .run(WidgetShowcase::default())
        .await
    {
        eprintln!("Error: {}", e);
    }
}
