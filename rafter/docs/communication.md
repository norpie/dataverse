# Communication

Rafter provides three mechanisms for inter-app communication: events, requests, and global data.

## Events (Pub/Sub)

Events are fire-and-forget broadcasts to all non-sleeping apps.

### Defining an Event

```rust
#[derive(Event, Clone)]
struct UserLoggedIn {
    user_id: u64,
    username: String,
}
```

### Publishing Events

```rust
#[handler]
async fn login(&self, cx: &AppContext) {
    // ... perform login ...

    cx.publish(UserLoggedIn {
        user_id: 123,
        username: "alice".to_string(),
    });
}
```

### Subscribing to Events

Use `#[event_handler]` in any app:

```rust
#[app_impl]
impl DashboardApp {
    #[event_handler]
    async fn on_login(&self, event: UserLoggedIn, cx: &AppContext) {
        cx.toast(format!("Welcome, {}!", event.username));
        self.current_user.set(Some(event.user_id));
    }
}
```

### Event Behavior

- Events are delivered to all non-sleeping instances
- Sleeping apps (`BlurPolicy::Sleep`) do not receive events
- Handlers run concurrently (fire-and-forget)
- The publisher continues immediately

## Requests (Request/Response)

Requests are for when you need a response from another app.

### Defining a Request

```rust
#[derive(Request)]
#[response(i32)]  // Return type
struct GetCounter;

#[derive(Request)]
#[response(Vec<String>)]
struct GetRecentFiles {
    limit: usize,
}
```

### Sending Requests

Target by app type (first non-sleeping instance):

```rust
#[handler]
async fn check_counter(&self, cx: &AppContext) {
    match cx.request::<CounterApp, GetCounter>(GetCounter).await {
        Ok(value) => {
            cx.toast(format!("Counter is: {}", value));
        }
        Err(RequestError::NoInstance) => {
            cx.toast(Toast::warning("Counter app not running"));
        }
        Err(e) => {
            cx.toast(Toast::error(format!("Request failed: {:?}", e)));
        }
    }
}
```

Target by instance ID:

```rust
let result = cx.request_to::<GetCounter>(instance_id, GetCounter).await?;
```

### Handling Requests

Use `#[request_handler]` in the target app:

```rust
#[app_impl]
impl CounterApp {
    #[request_handler]
    async fn handle_get_counter(&self, _req: GetCounter, _cx: &AppContext) -> i32 {
        self.value.get()
    }

    #[request_handler]
    async fn handle_recent_files(&self, req: GetRecentFiles, _cx: &AppContext) -> Vec<String> {
        self.files.get()
            .iter()
            .take(req.limit)
            .cloned()
            .collect()
    }
}
```

### Request Errors

```rust
pub enum RequestError {
    NoInstance,          // No awake instance of target app type
    InstanceNotFound,    // Target instance ID doesn't exist
    InstanceSleeping(InstanceId),  // Target is sleeping
    NoHandler,           // Target has no handler for this request
    HandlerPanicked,     // Handler panicked during execution
}
```

### Requests vs Events

| Aspect | Events | Requests |
|--------|--------|----------|
| Direction | One-to-many | One-to-one |
| Response | None | Typed response |
| Blocking | No | Yes (awaits response) |
| Sleeping apps | Skipped | Returns error |

## Global Data

Share data across all apps via the runtime.

### Registering Global Data

```rust
struct ApiClient {
    base_url: String,
}

impl ApiClient {
    async fn fetch(&self, path: &str) -> Result<Response, Error> {
        // ...
    }
}

#[tokio::main]
async fn main() {
    let client = ApiClient {
        base_url: "https://api.example.com".to_string(),
    };

    rafter::Runtime::new()
        .data(client)  // Register global data
        .initial::<MyApp>()
        .run()
        .await
        .unwrap();
}
```

### Accessing Global Data

```rust
#[handler]
async fn load_data(&self, cx: &AppContext) {
    let client = cx.data::<ApiClient>();
    let response = client.fetch("/users").await?;
    self.users.set_ready(response);
}
```

### Optional Access

```rust
if let Some(client) = cx.try_data::<ApiClient>() {
    // Use client
}
```

### Thread Safety

Global data is stored as `Arc<T>`, so it's shared immutably. For mutation, use interior mutability:

```rust
use std::sync::atomic::{AtomicU32, Ordering};

pub struct RequestTracker {
    count: AtomicU32,
}

impl RequestTracker {
    pub fn increment(&self) -> u32 {
        self.count.fetch_add(1, Ordering::SeqCst)
    }

    pub fn count(&self) -> u32 {
        self.count.load(Ordering::SeqCst)
    }
}
```

## Common Patterns

### Notifying on State Change

```rust
// In SettingsApp
#[handler]
async fn change_theme(&self, cx: &AppContext) {
    self.theme.set(Theme::Dark);

    // Notify all apps
    cx.publish(ThemeChanged {
        theme: Theme::Dark,
    });
}

// In other apps
#[event_handler]
async fn on_theme_change(&self, event: ThemeChanged, cx: &AppContext) {
    cx.set_theme(event.theme);
}
```

### Background Worker Pattern

```rust
// Worker app with BlurPolicy::Continue
#[app(on_blur = Continue)]
struct DownloadWorker {
    progress: i32,
}

#[derive(Request)]
#[response(i32)]
struct GetDownloadProgress;

#[app_impl]
impl DownloadWorker {
    #[request_handler]
    async fn get_progress(&self, _req: GetDownloadProgress, _cx: &AppContext) -> i32 {
        self.progress.get()
    }
}

// In main app
#[handler]
async fn check_download(&self, cx: &AppContext) {
    if let Ok(progress) = cx.request::<DownloadWorker, GetDownloadProgress>(GetDownloadProgress).await {
        cx.toast(format!("Download: {}%", progress));
    }
}
```

## Next Steps

- [Validation](validation.md) - Form validation
- [Advanced: Systems](advanced/systems.md) - Global keybinds
