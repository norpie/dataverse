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
    #[allow(dead_code)]
    fn increment(&mut self) {
        *self.count += 1;
    }

    #[handler]
    #[allow(dead_code)]
    fn decrement(&mut self) {
        *self.count -= 1;
    }

    #[handler]
    #[allow(dead_code)]
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

fn main() {
    // TODO: implement runtime
    println!("Counter example - macros work!");

    // Test that the app can be created via Default
    let app = CounterApp::default();
    println!("Initial count: {}", *app.count);

    // Test that it implements App trait
    let _name = app.name();
    println!("App name: {}", _name);

    // Test keybinds
    let keybinds = app.keybinds();
    println!("Keybinds registered: {}", keybinds.all().len());

    // Test view
    let node = app.view();
    println!("View created: {:?}", node);
}
