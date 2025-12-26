# Getting Started

This guide walks you through creating your first rafter application.

## Prerequisites

- Rust 1.75 or later
- A terminal emulator

## Project Setup

Create a new Rust project:

```bash
cargo new my-tui-app
cd my-tui-app
```

Add dependencies to `Cargo.toml`:

```toml
[dependencies]
rafter = { path = "../rafter" }  # or your path/version
tokio = { version = "1", features = ["full"] }
```

## Your First App

Replace `src/main.rs` with:

```rust
use rafter::prelude::*;

#[app]
struct HelloApp {
    message: String,
}

#[app_impl]
impl HelloApp {
    async fn on_start(&self, _cx: &AppContext) {
        self.message.set("Hello, rafter!".to_string());
    }

    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "q" | "escape" => quit,
        }
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        cx.exit();
    }

    fn page(&self) -> Node {
        let msg = self.message.get();
        page! {
            column (padding: 2) {
                text (bold, fg: primary) { "My First App" }
                text { msg }
                text (fg: muted) { "Press q to quit" }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    rafter::Runtime::new()
        .initial::<HelloApp>()
        .run()
        .await
        .unwrap();
}
```

Run your app:

```bash
cargo run
```

## Understanding the Code

### The `#[app]` Macro

```rust
#[app]
struct HelloApp {
    message: String,
}
```

The `#[app]` macro transforms your struct fields into reactive state. The `message: String` field becomes `message: State<String>`, which provides:
- `.get()` - Get a clone of the current value
- `.set(value)` - Set a new value (triggers re-render)
- `.update(|v| ...)` - Modify the value in place

### The `#[app_impl]` Macro

```rust
#[app_impl]
impl HelloApp { ... }
```

This macro processes your impl block, setting up:
- Lifecycle hooks (`on_start`, `on_foreground`, etc.)
- Keybind registration
- Handler dispatch

### Lifecycle Hooks

```rust
async fn on_start(&self, _cx: &AppContext) {
    self.message.set("Hello, rafter!".to_string());
}
```

`on_start` is called once when the app instance is created. Use it to initialize state.

### Keybinds

```rust
#[keybinds]
fn keys() -> Keybinds {
    keybinds! {
        "q" | "escape" => quit,
    }
}
```

The `keybinds!` macro defines keyboard shortcuts. Multiple keys can trigger the same handler using `|`.

### Handlers

```rust
#[handler]
async fn quit(&self, cx: &AppContext) {
    cx.exit();
}
```

Handlers are async functions that respond to keybinds or widget events. They receive `&AppContext` for framework interactions.

### The Page

```rust
fn page(&self) -> Node {
    let msg = self.message.get();
    page! {
        column (padding: 2) {
            text (bold, fg: primary) { "My First App" }
            text { msg }
        }
    }
}
```

The `page` method returns a `Node` tree describing your UI. The `page!` macro provides a declarative DSL for building the tree.

## Next Steps

- [Core Concepts](core-concepts.md) - Deeper dive into State, Resource, and AppContext
- [Page Macro](page-macro.md) - Full reference for the page! DSL
- [Keybinds and Handlers](keybinds-and-handlers.md) - Input handling patterns
