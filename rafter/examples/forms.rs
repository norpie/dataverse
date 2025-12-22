//! Forms Example
//!
//! A demo showcasing rafter's form widgets:
//! - Checkbox with label and custom indicators
//! - RadioGroup for mutually exclusive options
//! - Collapsible sections for organizing content
//! - Input fields with validation
//! - Form validation using the Validator API
//!
//! Use Tab to navigate between fields, Space/Enter to toggle checkboxes
//! or expand/collapse sections, and Up/Down arrows to navigate radio options.

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

    // Collapsible sections
    basic_info: Collapsible,
    preferences: Collapsible,
    advanced: Collapsible,

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

        // Set up collapsible sections
        self.basic_info.set_title("Basic Information");
        self.basic_info.expand(); // Start expanded

        self.preferences.set_title("Preferences");
        self.preferences.expand(); // Start expanded

        self.advanced.set_title("Advanced Options");
        // Starts collapsed by default
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
        // Validate using the Validator API
        let result = Validator::new()
            .field(&self.name, "name")
                .required("Name is required")
                .min_length(2, "Name must be at least 2 characters")
            .field(&self.email, "email")
                .required("Email is required")
                .email("Please enter a valid email address")
            .field(&self.accept_terms, "terms")
                .checked("You must accept the terms and conditions")
            .field(&self.priority, "priority")
                .selected("Please select a priority level")
            .validate();

        if result.is_valid() {
            // Success
            self.submitted.set(true);
            cx.toast_success("Form submitted successfully!");
        } else {
            // Show first error and focus that field
            result.focus_first(cx);
            if let Some(err) = result.first_error() {
                cx.toast_error(&err.message);
            }
        }
    }

    #[handler]
    async fn reset(&self, cx: &AppContext) {
        // Clear all form fields (this also auto-clears any validation errors)
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
    async fn on_advanced_expand(&self, cx: &AppContext) {
        cx.toast("Advanced options revealed!");
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        cx.exit();
    }

    fn page(&self) -> Node {
        let submitted = self.submitted.get();

        page! {
            column (padding: 2, gap: 1) {
                // Header
                text (bold, fg: primary) { "Form Components Demo" }
                text (fg: muted) { "Tab to navigate, Space/Enter to toggle sections/checkboxes" }

                // Basic Information (collapsible)
                collapsible(bind: self.basic_info) {
                    column (gap: 1, padding: 1) {
                        row (gap: 2) {
                            text (fg: muted) { "Name:     " }
                            input(bind: self.name)
                        }

                        row (gap: 2) {
                            text (fg: muted) { "Email:    " }
                            input(bind: self.email)
                        }

                        checkbox(bind: self.accept_terms, on_change: on_terms_change)
                        checkbox(bind: self.newsletter)
                    }
                }

                // Preferences (collapsible)
                collapsible(bind: self.preferences) {
                    row (gap: 4, padding: 1) {
                        column (gap: 1) {
                            text (bold) { "Theme" }
                            radio_group(bind: self.theme, on_change: on_theme_change)
                        }

                        column (gap: 1) {
                            text (bold) { "Priority" }
                            radio_group(bind: self.priority, on_change: on_priority_change)
                        }
                    }
                }

                // Advanced Options (collapsible, starts collapsed)
                collapsible(bind: self.advanced, on_expand: on_advanced_expand) {
                    column (gap: 1, padding: 1) {
                        text (fg: warning) { "These options are for power users only!" }
                        checkbox(bind: self.custom_checkbox)
                    }
                }

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
