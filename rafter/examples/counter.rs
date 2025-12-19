//! Counter Example
//!
//! A polished demo showcasing rafter's capabilities:
//! - Declarative views with the `view!` macro
//! - State management with automatic reactivity
//! - Keyboard navigation and vim-style keybinds
//! - Focus system with Tab navigation
//! - Async operations with progress feedback
//! - Toast notifications
//! - Theme-aware styling

use std::fs::File;
use std::time::Duration;

use log::LevelFilter;
use rafter::prelude::*;
use simplelog::{Config, WriteLogger};

#[app]
struct Counter {
    value: i32,
    step: i32,
    data: Resource<String>,
}

#[app_impl]
impl Counter {
    async fn on_start(&self, _cx: &AppContext) {
        self.step.set(1);
    }

    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "k" | "up" => increment,
            "j" | "down" => decrement,
            "+" => increment,
            "-" => decrement,
            "r" => reset,
            "l" => load_data,
            "1" => set_step_1,
            "5" => set_step_5,
            "0" => set_step_10,
            "q" => quit,
        }
    }

    #[handler]
    async fn increment(&self, _cx: &AppContext) {
        let step = self.step.get();
        self.value.update(|v| *v += step);
    }

    #[handler]
    async fn decrement(&self, _cx: &AppContext) {
        let step = self.step.get();
        self.value.update(|v| *v -= step);
    }

    #[handler]
    async fn reset(&self, cx: &AppContext) {
        self.value.set(0);
        self.step.set(1);
        self.data.set_idle();
        cx.toast("Reset");
    }

    #[handler]
    async fn set_step_1(&self, _cx: &AppContext) {
        self.step.set(1);
    }

    #[handler]
    async fn set_step_5(&self, _cx: &AppContext) {
        self.step.set(5);
    }

    #[handler]
    async fn set_step_10(&self, _cx: &AppContext) {
        self.step.set(10);
    }

    #[handler]
    async fn load_data(&self, cx: &AppContext) {
        self.data.set_loading();
        cx.toast("Loading...");

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
        self.data.set_ready("API response received".to_string());
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        cx.exit();
    }

    fn view(&self) -> Node {
        let value_str = self.value.get().to_string();
        let step_str = self.step.get().to_string();

        // Build data status display
        let data_state = self.data.get();

        view! {
            column (padding: 1, gap: 1) {
                column {
                    text (bold, fg: primary) { "Counter" }
                    text (fg: muted) { "A rafter demo application" }
                }

                column (border: rounded) {
                    row (gap: 2) {
                        text (fg: muted) { "Value:" }
                        text (bold, fg: primary) { value_str }
                    }
                    row (gap: 2) {
                        text (fg: muted) { "Step: " }
                        text (fg: secondary) { step_str }
                    }
                }

                row (gap: 1) {
                    button(label: "−", id: "dec", on_click: decrement)
                    button(label: "+", id: "inc", on_click: increment)
                    button(label: "Reset", id: "reset", on_click: reset)
                    button(label: "Load", id: "load", on_click: load_data)
                }

                row (gap: 1) {
                    text (fg: muted) { "Data:" }
                    match data_state {
                        ResourceState::Idle => {
                            text (fg: muted) { "Press 'l' to load" }
                        }
                        ResourceState::Loading => {
                            text (fg: warning) { "Loading..." }
                        }
                        ResourceState::Progress(p) => {
                            text (fg: warning) { p.message.clone().unwrap_or_default() }
                        }
                        ResourceState::Ready(s) => {
                            text (fg: success) { s }
                        }
                        ResourceState::Error(e) => {
                            text (fg: error) { e.to_string() }
                        }
                    }
                }

                text (fg: muted) { "↑k/↓j ±value  1/5/0 step  r reset  l load  Tab focus  q quit" }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Initialize file logging
    if let Ok(log_file) = File::create("counter.log") {
        let _ = WriteLogger::init(LevelFilter::Debug, Config::default(), log_file);
    }

    if let Err(e) = rafter::Runtime::new().start_with::<Counter>().await {
        eprintln!("Error: {}", e);
    }
}
