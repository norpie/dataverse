# Rafter

A declarative TUI framework for Rust, built on [ratatui](https://github.com/ratatui-org/ratatui).

## Quick Example

```rust
use rafter::prelude::*;

#[app]
struct Counter {
    value: i32,
}

#[app_impl]
impl Counter {
    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "k" | "up" => increment,
            "j" | "down" => decrement,
            "q" => quit,
        }
    }

    #[handler]
    async fn increment(&self, _cx: &AppContext) {
        self.value.update(|v| *v += 1);
    }

    #[handler]
    async fn decrement(&self, _cx: &AppContext) {
        self.value.update(|v| *v -= 1);
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        cx.exit();
    }

    fn page(&self) -> Node {
        let val = self.value.get();
        page! {
            column (padding: 1) {
                text (bold) { "Counter" }
                text { format!("Value: {}", val) }
                button(id: "inc", label: "+", on_click: increment)
            }
        }
    }
}

#[tokio::main]
async fn main() {
    rafter::Runtime::new()
        .initial::<Counter>()
        .run()
        .await
        .unwrap();
}
```

## Features

- **Declarative UI** - Build pages with the `page!` macro DSL
- **Reactive State** - `State<T>` and `Resource<T>` with automatic re-rendering
- **Async Handlers** - All event handlers are async-first
- **Rich Widgets** - Input, Button, List, Tree, Table, Select, Autocomplete, and more
- **Modal System** - Awaitable modals with typed results
- **Multi-App** - Run multiple app instances with configurable blur policies
- **Theming** - Customizable color themes
- **Form Validation** - Fluent validation API with sync/async rules
- **Inter-App Communication** - Pub/sub events and request/response patterns

## Documentation

See the [full documentation](docs/README.md) for detailed guides and API reference.
