//! Counter Example
//!
//! A polished demo showcasing rafter's capabilities:
//! - Declarative pages with the `page!` macro
//! - State management with automatic reactivity
//! - Keyboard navigation and vim-style keybinds
//! - Focus system with Tab navigation
//! - Async operations with progress feedback
//! - Toast notifications
//! - Modal dialogs

use std::fs::File;
use std::time::Duration;

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Text};
use simplelog::{Config, LevelFilter, WriteLogger};

// ============================================================================
// Really Sure Modal (nested confirmation)
// ============================================================================

#[modal]
struct ReallySureModal;

#[modal_impl]
impl ReallySureModal {
    #[keybinds]
    fn keys() {
        bind("y", "enter", confirm);
        bind("n", "escape", cancel);
    }

    #[handler]
    async fn confirm(&self, mx: &ModalContext<bool>) {
        mx.close(true);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<bool>) {
        mx.close(false);
    }

    fn element(&self) -> Element {
        page! {
            column (padding: 2, gap: 1) style (bg: background) {
                text (content: "Are you REALLY sure?") style (bold: true, fg: error)
                text (content: "This action cannot be undone!") style (fg: muted)
                row (gap: 2) {
                    button (label: "No [n]", id: "no") on_activate: cancel()
                    button (label: "Yes [y]", id: "yes") on_activate: confirm()
                }
            }
        }
    }
}

// ============================================================================
// Confirm Modal
// ============================================================================

#[modal]
struct ConfirmModal {
    #[state(skip)]
    message: String,
}

#[modal_impl]
impl ConfirmModal {
    #[keybinds]
    fn keys() {
        bind("y", "enter", confirm);
        bind("n", "escape", cancel);
    }

    #[handler]
    async fn confirm(&self, cx: &AppContext, mx: &ModalContext<bool>) {
        // Show nested confirmation modal
        let really_sure = cx.modal(ReallySureModal::default()).await;
        if really_sure {
            mx.close(true);
        }
        // If not really sure, stay on this modal
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<bool>) {
        mx.close(false);
    }

    fn element(&self) -> Element {
        let message = self.message.clone();
        page! {
            column (padding: 2, gap: 1) style (bg: background) {
                text (content: "Confirm") style (bold: true, fg: warning)
                text (content: {message})
                row (gap: 2) {
                    button (label: "No [n]", id: "no") on_activate: cancel()
                    button (label: "Yes [y]", id: "yes") on_activate: confirm()
                }
            }
        }
    }
}

// ============================================================================
// Counter App
// ============================================================================

#[app]
struct Counter {
    value: i32,
    step: i32,
    data: Resource<String>,
}

#[app_impl]
impl Counter {
    async fn on_start(&self) {
        self.step.set(1);
    }

    #[keybinds]
    fn keys() {
        bind("k", "up", increment);
        bind("j", "down", decrement);
        bind("+", increment);
        bind("-", decrement);
        bind("r", reset);
        bind("l", load_data);
        bind("1", set_step_1);
        bind("5", set_step_5);
        bind("0", set_step_10);
        bind("q", quit);
    }

    #[handler]
    async fn increment(&self) {
        let step = self.step.get();
        self.value.update(|v| *v += step);
    }

    #[handler]
    async fn decrement(&self) {
        let step = self.step.get();
        self.value.update(|v| *v -= step);
    }

    #[handler]
    async fn reset(&self, cx: &AppContext, gx: &GlobalContext) {
        // Show confirmation modal
        let confirmed = cx
            .modal(ConfirmModal {
                message: "Reset the counter to zero?".to_string(),
                ..Default::default()
            })
            .await;

        if confirmed {
            self.value.set(0);
            self.step.set(1);
            self.data.set_idle();
            gx.toast(Toast::success("Counter reset - Value and step restored to defaults."));
        }
    }

    #[handler]
    async fn set_step_1(&self) {
        self.step.set(1);
    }

    #[handler]
    async fn set_step_5(&self) {
        self.step.set(5);
    }

    #[handler]
    async fn set_step_10(&self) {
        self.step.set(10);
    }

    #[handler]
    async fn load_data(&self, gx: &GlobalContext) {
        self.data.set_loading();
        gx.toast(Toast::info("Loading data - Fetching from remote server..."));

        // Simulate network request with progress
        for i in 1..=3 {
            tokio::time::sleep(Duration::from_millis(400)).await;
            self.data.set_progress(ProgressState {
                current: i,
                total: Some(3),
                message: Some(format!("Step {}/3", i)),
            });
        }
        tokio::time::sleep(Duration::from_millis(400)).await;

        // Random chance of error (roughly 30%)
        let random_value = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();

        if random_value % 10 < 3 {
            self.data.set_error("Connection timeout".to_string());
            gx.toast(
                Toast::error("Request failed - Could not connect to the server.")
                    .with_duration(Duration::from_secs(6)),
            );
        } else {
            self.data.set_ready("API response received".to_string());
            gx.toast(Toast::success("Data loaded - Successfully fetched 42 records."));
        }
    }

    #[handler]
    async fn quit(&self, gx: &GlobalContext) {
        gx.shutdown();
    }

    fn element(&self) -> Element {
        let value_str = self.value.get().to_string();
        let step_str = self.step.get().to_string();
        let data_state = self.data.get();

        page! {
            column (padding: 1, gap: 1) style (bg: background) {
                column {
                    text (content: "Counter") style (bold: true, fg: primary)
                    text (content: "A rafter demo with smooth animations") style (fg: muted)
                }

                column (id: "value-display", padding: 1) style (bg: surface) {
                    row (gap: 2) {
                        text (content: "Value:") style (fg: muted)
                        text (content: {value_str}) style (bold: true, fg: primary)
                    }
                    row (gap: 2) {
                        text (content: "Step:") style (fg: muted)
                        text (content: {step_str}) style (fg: secondary)
                    }
                }

                row (gap: 1) {
                    button (label: "−", id: "dec") on_activate: decrement()
                    button (label: "+", id: "inc") on_activate: increment()
                    button (label: "Reset", id: "reset") on_activate: reset()
                    button (label: "Load", id: "load") on_activate: load_data()
                }

                row (gap: 1) {
                    text (content: "Data:") style (fg: muted)
                    match data_state {
                        ResourceState::Idle => text (content: "Press 'l' to load") style (fg: muted),
                        ResourceState::Loading => text (content: "Loading...") style (fg: warning),
                        ResourceState::Progress(p) => text (content: {p.message.clone().unwrap_or_default()}) style (fg: warning),
                        ResourceState::Ready(s) => text (content: {s}) style (fg: success),
                        ResourceState::Error(e) => text (content: {e.to_string()}) style (fg: error),
                    }
                }

                text (content: "↑k/↓j ±value  1/5/0 step  r reset  l load  q quit") style (fg: muted)
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Set up file logging
    let log_file = File::create("counter.log").expect("Failed to create log file");
    WriteLogger::init(LevelFilter::Debug, Config::default(), log_file)
        .expect("Failed to initialize logger");

    if let Err(e) = Runtime::new()
        .expect("Failed to create runtime")
        .run(Counter::default())
        .await
    {
        eprintln!("Error: {}", e);
    }
}
