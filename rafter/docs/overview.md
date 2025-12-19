# Rafter

Rafter is a web-inspired TUI framework for Rust, designed for building complex terminal applications with modern patterns.

## Goals

- **Declarative views** - HTML-like syntax with semantic elements (`column`, `row`, `text`) via macros
- **CSS-inspired styling** - Inline styling with optional attributes, flexbox-like layout
- **First-class async** - Async handlers, resource management, proper cancellation
- **Multi-app architecture** - Multiple apps running concurrently, pub/sub communication
- **Modern interactions** - Keyboard and mouse on equal footing, focus management
- **Theming** - Runtime-switchable themes with compile-time validation

## Quick Example

```rust
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

    fn increment(&mut self) {
        self.count += 1;
    }

    fn decrement(&mut self) {
        self.count -= 1;
    }

    fn quit(&mut self, cx: AppContext) {
        cx.exit();
    }

    fn view(&self) -> Node {
        view! {
            column (padding: 1, gap: 1, align: center) {
                text (bold, color: primary) { "Counter" }
                text { self.count.to_string() }
                text (color: text_muted) { "j/k to change, q to quit" }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    rafter::Runtime::new()
        .start_with::<CounterApp>()
        .await;
}
```

## Documentation

- [Architecture](./architecture.md) - Runtime, apps, and layer system
- [Apps](./apps.md) - App lifecycle, registration, and handlers
- [Views](./views.md) - View syntax, components, and primitives
- [State](./state.md) - State management and reactivity
- [Styling](./styling.md) - Layout, colors, and theming
- [Interactions](./interactions.md) - Keybinds, focus, and mouse support
- [Overlays](./overlays.md) - Modals, toasts, and system overlays
- [Async](./async.md) - Async handlers and resources
- [Animations](./animations.md) - Animation system
- [Errors](./errors.md) - Error handling and panic recovery
