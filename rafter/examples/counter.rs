// Example: Counter App
//
// A minimal app to test rafter's core features:
// - App definition with state
// - Keybinds
// - View rendering
// - Event handling

use std::fs::File;

use log::LevelFilter;
use rafter::prelude::*;
use simplelog::{Config, WriteLogger};

#[app]
struct CounterApp {
    count: i32,
}

#[app_impl]
impl CounterApp {
    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "j" | "down" => decrement,
            "k" | "up" => increment,
            "q" => quit,
        }
    }

    #[handler]
    fn increment(&mut self) {
        log::info!("increment called, count was {}", *self.count);
        *self.count += 1;
    }

    #[handler]
    fn decrement(&mut self) {
        log::info!("decrement called, count was {}", *self.count);
        *self.count -= 1;
    }

    #[handler]
    fn quit(&mut self, cx: &mut AppContext) {
        log::info!("quit called");
        cx.exit();
    }

    fn view(&self) -> Node {
        view! {
            column (padding: 1, gap: 1, align: center) {
                text (bold) { "Counter" }
                text { self.count.to_string() }
                text (dim) { "j/k to change, q to quit" }
            }
        }
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
