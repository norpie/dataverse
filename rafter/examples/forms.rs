//! Forms Example
//!
//! A demo showcasing rafter's form widgets:
//! - Checkbox with label and custom indicators
//! - RadioGroup for mutually exclusive options
//! - Input fields
//! - Form state management
//!
//! Use Tab to navigate between fields, Space/Enter to toggle checkboxes,
//! and Up/Down arrows to navigate radio options.

use std::fs::File;

use log::LevelFilter;
use rafter::prelude::*;
use simplelog::{Config, WriteLogger};

// ============================================================================
// Forms App
// ============================================================================

#[app]
struct FormsApp {
    // Text inputs
    name: Input,
    email: Input,

    // Checkboxes with different configurations
    accept_terms: Checkbox,
    newsletter: Checkbox,
    custom_checkbox: Checkbox,

    // Radio groups
    theme: RadioGroup,
    priority: RadioGroup,

    // Form state
    submitted: bool,
}

#[app_impl]
impl FormsApp {
    async fn on_start(&self, _cx: &AppContext) {
        // Set up initial values
        self.name.set_placeholder("Enter your name");
        self.email.set_placeholder("Enter your email");

        // Set up checkbox labels
        self.accept_terms
            .set_label("I accept the terms and conditions");
        self.newsletter.set_label("Subscribe to newsletter");
        self.custom_checkbox.set_label("Custom indicators");
        self.custom_checkbox.set_indicators('✓', '✗');

        // Set up radio groups
        self.theme
            .set_options(vec!["Light Mode", "Dark Mode", "System Default"]);
        self.theme.select(2); // Default to "System Default"

        self.priority.set_options(vec!["Low", "Medium", "High"]);
        self.priority.select(1); // Default to "Medium"
    }

    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "ctrl+s" => submit,
            "ctrl+r" => reset,
            "q" | "ctrl+c" => quit,
        }
    }

    #[handler]
    async fn submit(&self, cx: &AppContext) {
        // Validate
        if self.name.is_empty() {
            cx.toast_error("Name is required");
            return;
        }

        if self.email.is_empty() {
            cx.toast_error("Email is required");
            return;
        }

        if !self.accept_terms.is_checked() {
            cx.toast_error("You must accept the terms");
            return;
        }

        // Success
        self.submitted.set(true);
        cx.toast_success("Form submitted successfully!");
    }

    #[handler]
    async fn reset(&self, cx: &AppContext) {
        self.name.clear();
        self.email.clear();
        self.accept_terms.set_checked(false);
        self.newsletter.set_checked(false);
        self.custom_checkbox.set_checked(false);
        self.theme.select(2);
        self.priority.select(1);
        self.submitted.set(false);
        cx.toast("Form reset");
    }

    #[handler]
    async fn on_terms_change(&self, cx: &AppContext) {
        if self.accept_terms.is_checked() {
            cx.toast("Terms accepted");
        }
    }

    #[handler]
    async fn on_theme_change(&self, cx: &AppContext) {
        if let Some(label) = self.theme.selected_label() {
            cx.toast(format!("Theme: {}", label));
        }
    }

    #[handler]
    async fn on_priority_change(&self, cx: &AppContext) {
        if let Some(label) = self.priority.selected_label() {
            cx.toast(format!("Priority: {}", label));
        }
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        cx.exit();
    }

    fn page(&self) -> Node {
        let submitted = self.submitted.get();
        let divider = "─".repeat(50);

        page! {
            column (padding: 2, gap: 1) {
                // Header
                text (bold, fg: primary) { "Form Components Demo" }
                text (fg: muted) { "Tab to navigate, Space/Enter to toggle, Up/Down for radios" }

                // Divider
                text (fg: muted) { divider.clone() }

                // Text inputs
                column (gap: 1) {
                    text (bold) { "Text Inputs" }

                    row (gap: 2) {
                        text (fg: muted) { "Name:     " }
                        input(bind: self.name)
                    }

                    row (gap: 2) {
                        text (fg: muted) { "Email:    " }
                        input(bind: self.email)
                    }
                }

                // Divider
                text (fg: muted) { divider.clone() }

                // Checkboxes
                column (gap: 1) {
                    text (bold) { "Checkboxes" }

                    checkbox(bind: self.accept_terms, on_change: on_terms_change)
                    checkbox(bind: self.newsletter)
                    checkbox(bind: self.custom_checkbox)
                }

                // Divider
                text (fg: muted) { divider.clone() }

                // Radio Groups
                row (gap: 4) {
                    column (gap: 1) {
                        text (bold) { "Theme" }
                        radio_group(bind: self.theme, on_change: on_theme_change)
                    }

                    column (gap: 1) {
                        text (bold) { "Priority" }
                        radio_group(bind: self.priority, on_change: on_priority_change)
                    }
                }

                // Divider
                text (fg: muted) { divider.clone() }

                // Status
                row (gap: 2) {
                    text (fg: muted) { "Status:" }
                    if submitted {
                        text (fg: success, bold) { "Submitted!" }
                    } else {
                        text (fg: warning) { "Not submitted" }
                    }
                }

                // Help
                text (fg: muted) { "Ctrl+S submit  Ctrl+R reset  q quit" }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Initialize file logging
    if let Ok(log_file) = File::create("forms.log") {
        let _ = WriteLogger::init(LevelFilter::Debug, Config::default(), log_file);
    }

    if let Err(e) = rafter::Runtime::new().start_with::<FormsApp>().await {
        eprintln!("Error: {}", e);
    }
}
