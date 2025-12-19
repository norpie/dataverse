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
    fn on_start(&mut self, _cx: &mut AppContext) {
        *self.step = 1;
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
    fn increment(&mut self, _cx: &mut AppContext) {
        *self.value += *self.step;
    }

    #[handler]
    fn decrement(&mut self, _cx: &mut AppContext) {
        *self.value -= *self.step;
    }

    #[handler]
    fn reset(&mut self, cx: &mut AppContext) {
        *self.value = 0;
        *self.step = 1;
        self.data.set(Resource::Idle);
        cx.toast("Reset");
    }

    #[handler]
    fn set_step_1(&mut self, _cx: &mut AppContext) {
        *self.step = 1;
    }

    #[handler]
    fn set_step_5(&mut self, _cx: &mut AppContext) {
        *self.step = 5;
    }

    #[handler]
    fn set_step_10(&mut self, _cx: &mut AppContext) {
        *self.step = 10;
    }

    #[handler]
    fn load_data(&mut self, cx: &mut AppContext) {
        let data = self.data.clone();
        data.set(Resource::Loading);

        cx.spawn(async move {
            // Simulate network request with progress
            for i in 1..=3 {
                tokio::time::sleep(Duration::from_millis(400)).await;
                data.set(Resource::Progress(ProgressState {
                    current: i,
                    total: Some(3),
                    message: Some(format!("Step {}/3", i)),
                }));
            }
            tokio::time::sleep(Duration::from_millis(400)).await;
            data.set(Resource::Ready("API response received".to_string()));
        });

        cx.toast("Loading...");
    }

    #[handler]
    fn quit(&mut self, cx: &mut AppContext) {
        cx.exit();
    }

    fn view(&self) -> Node {
        let value_str = self.value.to_string();
        let step_str = self.step.to_string();

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
                        Resource::Idle => {
                            text (fg: muted) { "Press 'l' to load" }
                        }
                        Resource::Loading => {
                            text (fg: warning) { "Loading..." }
                        }
                        Resource::Progress(p) => {
                            text (fg: warning) { p.message.clone().unwrap_or_default() }
                        }
                        Resource::Ready(s) => {
                            text (fg: success) { s }
                        }
                        Resource::Error(e) => {
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
