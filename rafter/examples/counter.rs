// Example: Counter App
//
// A demo app showcasing rafter's Phase 4 features:
// - App definition with state
// - Keybinds
// - View rendering
// - Event handling
// - Focus system with Tab navigation
// - Text input fields
// - Buttons
// - Toast notifications

use std::fs::File;

use log::LevelFilter;
use rafter::prelude::*;
use simplelog::{Config, WriteLogger};

#[app]
struct CounterApp {
    count: i32,
    step: String,
    message: String,
}

#[app_impl]
impl CounterApp {
    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "j" | "down" => decrement,
            "k" | "up" => increment,
            "t" => show_toast,
            "e" => show_error,
            "r" => reset,
            "q" => quit,
        }
    }

    #[handler]
    fn increment(&mut self, cx: &mut AppContext) {
        let step = self.step.parse::<i32>().unwrap_or(1);
        log::info!("increment by {}, count was {}", step, *self.count);
        *self.count += step;
        cx.toast(format!("Count increased to {}", *self.count));
    }

    #[handler]
    fn decrement(&mut self, cx: &mut AppContext) {
        let step = self.step.parse::<i32>().unwrap_or(1);
        log::info!("decrement by {}, count was {}", step, *self.count);
        *self.count -= step;
        cx.toast(format!("Count decreased to {}", *self.count));
    }

    #[handler]
    fn reset(&mut self, cx: &mut AppContext) {
        log::info!("reset count");
        *self.count = 0;
        *self.step = "1".to_string();
        cx.toast("Counter reset!");
    }

    #[handler]
    fn show_toast(&mut self, cx: &mut AppContext) {
        if self.message.is_empty() {
            cx.toast("Hello from rafter!");
        } else {
            cx.toast((*self.message).clone());
        }
    }

    #[handler]
    fn show_error(&mut self, cx: &mut AppContext) {
        cx.show_toast(Toast::error("This is an error toast!"));
    }

    #[handler]
    fn quit(&mut self, cx: &mut AppContext) {
        log::info!("quit called");
        cx.exit();
    }

    // Handler for step input changes
    #[handler]
    fn step_input_change(&mut self, cx: &mut AppContext) {
        if let Some(text) = cx.input_text() {
            log::info!("step input changed: {}", text);
            *self.step = text.to_string();
        }
    }

    // Handler for step input submit (Enter)
    #[handler]
    fn step_input_submit(&mut self, cx: &mut AppContext) {
        if let Some(text) = cx.input_text() {
            log::info!("step input submitted: {}", text);
            *self.step = text.to_string();
            if let Ok(step) = text.parse::<i32>() {
                cx.toast(format!("Step size set to {}", step));
            } else {
                cx.show_toast(Toast::error("Invalid step value"));
            }
        }
    }

    // Handler for message input changes
    #[handler]
    fn message_input_change(&mut self, cx: &mut AppContext) {
        if let Some(text) = cx.input_text() {
            log::info!("message input changed: {}", text);
            *self.message = text.to_string();
        }
    }

    // Handler for message input submit
    #[handler]
    fn message_input_submit(&mut self, cx: &mut AppContext) {
        if let Some(text) = cx.input_text() {
            log::info!("message input submitted: {}", text);
            *self.message = text.to_string();
            cx.toast(format!("Message set: {}", text));
        }
    }

    // Handler for increment button click
    #[handler]
    fn inc_btn_submit(&mut self, cx: &mut AppContext) {
        log::info!("increment button clicked");
        self.increment(cx);
    }

    // Handler for decrement button click
    #[handler]
    fn dec_btn_submit(&mut self, cx: &mut AppContext) {
        log::info!("decrement button clicked");
        self.decrement(cx);
    }

    // Handler for reset button click
    #[handler]
    fn reset_btn_submit(&mut self, cx: &mut AppContext) {
        log::info!("reset button clicked");
        self.reset(cx);
    }

    fn focusable_ids(&self) -> Vec<String> {
        vec![
            "step_input".to_string(),
            "message_input".to_string(),
            "inc_btn".to_string(),
            "dec_btn".to_string(),
            "reset_btn".to_string(),
        ]
    }

    fn captures_input(&self, id: &str) -> bool {
        // Only input fields capture text input, not buttons
        matches!(id, "step_input" | "message_input")
    }

    fn view_with_focus(&self, focus: &FocusState) -> Node {
        let step_focused = focus.is_focused("step_input");
        let message_focused = focus.is_focused("message_input");
        let inc_focused = focus.is_focused("inc_btn");
        let dec_focused = focus.is_focused("dec_btn");
        let reset_focused = focus.is_focused("reset_btn");

        view! {
            column (padding: 1, gap: 1) {
                text (bold) { "Counter Demo - Phase 4 Features" }
                text { "" }

                // Counter display
                row (gap: 1) {
                    text { "Count: " }
                    text (bold, fg: cyan) { self.count.to_string() }
                }

                text { "" }

                // Step input
                row (gap: 1) {
                    text { "Step size: " }
                    input(
                        value: self.step.clone(),
                        placeholder: "1",
                        id: "step_input",
                        focused: step_focused
                    )
                }

                // Message input
                row (gap: 1) {
                    text { "Message: " }
                    input(
                        value: self.message.clone(),
                        placeholder: "Enter toast message...",
                        id: "message_input",
                        focused: message_focused
                    )
                }

                text { "" }

                // Buttons row
                row (gap: 2) {
                    button(label: "+", id: "inc_btn", focused: inc_focused)
                    button(label: "-", id: "dec_btn", focused: dec_focused)
                    button(label: "Reset", id: "reset_btn", focused: reset_focused)
                }

                text { "" }

                // Help text
                text (dim) { "Keybinds:" }
                text (dim) { "  j/k or down/up - decrement/increment" }
                text (dim) { "  t - show toast, e - show error toast" }
                text (dim) { "  r - reset counter" }
                text (dim) { "  Tab/Shift+Tab - navigate focus" }
                text (dim) { "  Enter - activate focused element" }
                text (dim) { "  q - quit" }
            }
        }
    }

    fn view(&self) -> Node {
        // Fallback without focus
        self.view_with_focus(&FocusState::new())
    }
}

#[tokio::main]
async fn main() {
    // Initialize logging to file (truncate on start)
    let log_file = File::create("counter.log").expect("Failed to create log file");
    WriteLogger::init(LevelFilter::Debug, Config::default(), log_file)
        .expect("Failed to initialize logger");

    log::info!("Counter app starting");

    if let Err(e) = rafter::Runtime::new().start_with::<CounterApp>().await {
        log::error!("Runtime error: {}", e);
        eprintln!("Error: {}", e);
    }

    log::info!("Counter app exiting");
}
