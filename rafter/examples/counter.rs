// Example: Counter App
//
// A minimal app to test rafter's core features:
// - App definition with state
// - Keybinds
// - View rendering
// - Event handling

use rafter::prelude::*;

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
        *self.count += 1;
    }

    #[handler]
    fn decrement(&mut self) {
        *self.count -= 1;
    }

    #[handler]
    fn quit(&mut self, cx: &mut AppContext) {
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
    if let Err(e) = rafter::Runtime::new().start_with::<CounterApp>().await {
        eprintln!("Error: {}", e);
    }
}
