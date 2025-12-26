# Core Concepts

This guide covers the fundamental building blocks of rafter applications.

## Apps

An app is the primary unit of a rafter application. Define one with the `#[app]` and `#[app_impl]` macros:

```rust
#[app]
struct MyApp {
    counter: i32,
    name: String,
    data: Resource<Vec<Item>>,
}

#[app_impl]
impl MyApp {
    fn page(&self) -> Node {
        // ...
    }
}
```

### App Configuration

Customize app behavior with attributes:

```rust
#[app(name = "My App", on_blur = Sleep, singleton)]
struct MyApp { ... }
```

- `name` - Display name for the app
- `on_blur` - Behavior when losing focus: `Continue` (default), `Sleep`, or `Close`
- `singleton` - Only one instance allowed

## State

`State<T>` provides reactive state with automatic re-rendering.

### Basic Usage

```rust
#[app]
struct Counter {
    value: i32,  // Becomes State<i32>
}

#[app_impl]
impl Counter {
    #[handler]
    async fn increment(&self, _cx: &AppContext) {
        // Get current value
        let current = self.value.get();

        // Set new value
        self.value.set(current + 1);

        // Or update in place
        self.value.update(|v| *v += 1);
    }
}
```

### Skipping State Transformation

For fields that shouldn't be wrapped in `State<T>`:

```rust
#[app]
struct MyApp {
    #[state(skip)]
    config: AppConfig,  // Remains as AppConfig, not State<AppConfig>
}
```

### Thread Safety

`State<T>` uses `Arc<RwLock<T>>` internally, making it:
- Cheap to clone
- Safe to use across async boundaries
- Safe to share between handlers

## Resource

`Resource<T>` manages async-loadable data with loading states.

### States

```rust
pub enum ResourceState<T> {
    Idle,              // Not started
    Loading,           // In progress (indeterminate)
    Progress(ProgressState),  // In progress with progress info
    Ready(T),          // Successfully loaded
    Error(ResourceError),  // Failed
}
```

### Usage

```rust
#[app]
struct DataApp {
    items: Resource<Vec<Item>>,
}

#[app_impl]
impl DataApp {
    #[handler]
    async fn load(&self, cx: &AppContext) {
        self.items.set_loading();

        match fetch_items().await {
            Ok(data) => self.items.set_ready(data),
            Err(e) => self.items.set_error(e.to_string()),
        }
    }

    fn page(&self) -> Node {
        let state = self.items.get();
        page! {
            column {
                match state {
                    ResourceState::Idle => {
                        text { "Press 'l' to load" }
                    }
                    ResourceState::Loading => {
                        text (fg: warning) { "Loading..." }
                    }
                    ResourceState::Ready(items) => {
                        text (fg: success) { format!("{} items loaded", items.len()) }
                    }
                    ResourceState::Error(e) => {
                        text (fg: error) { e.to_string() }
                    }
                }
            }
        }
    }
}
```

### Progress Tracking

```rust
#[handler]
async fn load_with_progress(&self, _cx: &AppContext) {
    self.data.set_loading();

    for i in 1..=10 {
        // Simulate work
        tokio::time::sleep(Duration::from_millis(100)).await;

        self.data.set_progress(ProgressState {
            current: i,
            total: Some(10),
            message: Some(format!("Step {}/10", i)),
        });
    }

    self.data.set_ready(result);
}
```

## App Lifecycle

Apps have several lifecycle hooks:

```rust
#[app_impl]
impl MyApp {
    /// Called once when the app instance is created
    async fn on_start(&self, cx: &AppContext) {
        // Initialize state, load data, etc.
    }

    /// Called when the app gains focus
    async fn on_foreground(&self, cx: &AppContext) {
        // Refresh data, resume animations, etc.
    }

    /// Called when the app loses focus
    async fn on_background(&self, cx: &AppContext) {
        // Pause work, save state, etc.
    }

    /// Called before close - return false to cancel
    fn on_close_request(&self, cx: &AppContext) -> bool {
        // Check for unsaved changes
        true  // Allow close
    }

    /// Called during cleanup after close is confirmed
    async fn on_close(&self, cx: &AppContext) {
        // Final cleanup
    }
}
```

## AppContext

`AppContext` is passed to handlers and provides framework functionality.

### Common Methods

```rust
#[handler]
async fn example(&self, cx: &AppContext) {
    // Exit the application
    cx.exit();

    // Show toast notifications
    cx.toast("Simple message");
    cx.toast(Toast::success("Operation completed"));
    cx.toast(Toast::error("Something went wrong")
        .with_body("Details here")
        .with_duration(Duration::from_secs(10)));

    // Focus a specific widget
    cx.focus("my-input");

    // Spawn a new app instance
    let id = cx.spawn_and_focus(OtherApp::default())?;

    // Open a modal and wait for result
    let confirmed = cx.modal(ConfirmModal {
        message: "Are you sure?".into()
    }).await;

    // Access global data
    let client = cx.data::<ApiClient>();

    // Publish an event to all apps
    cx.publish(MyEvent { data: 42 });

    // Send a request to another app
    let result = cx.request::<OtherApp, MyRequest>(MyRequest).await?;
}
```

### Toast Levels

```rust
cx.toast(Toast::info("Information"));
cx.toast(Toast::success("Success!"));
cx.toast(Toast::warning("Warning"));
cx.toast(Toast::error("Error"));
```

## Runtime

The `Runtime` is the entry point for your application:

```rust
#[tokio::main]
async fn main() {
    rafter::Runtime::new()
        .theme(MyTheme::new())           // Custom theme
        .data(ApiClient::new())          // Global data
        .initial::<MyApp>()              // Starting app
        .run()
        .await
        .unwrap();
}
```

### Global Data

Register shared data accessible to all apps:

```rust
let client = ApiClient::new("https://api.example.com");

rafter::Runtime::new()
    .data(client)
    .initial::<MyApp>()
    .run()
    .await?;

// In handlers:
let client = cx.data::<ApiClient>();
let response = client.fetch("/users").await;
```

## Next Steps

- [Page Macro](page-macro.md) - Learn the UI DSL
- [Keybinds and Handlers](keybinds-and-handlers.md) - Input handling
- [Widgets](widgets/overview.md) - Available widgets
