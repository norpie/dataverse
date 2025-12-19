# Error Handling

Rafter provides robust error handling at multiple levels.

## Handler Errors

For expected errors, use standard Result handling:

```rust
#[handler]
async fn load_records(&self, cx: AppContext) {
    self.records.set(Resource::Loading);
    
    match cx.api.get_records().await {
        Ok(data) => self.records.set(Resource::Ready(data)),
        Err(e) => self.records.set(Resource::Error(e.into())),
    }
}
```

## Panic Handling

Rafter catches panics at the framework level to prevent crashes.

### Runtime Configuration

```rust
rafter::Runtime::new()
    .on_panic(PanicBehavior::ShowError)  // Default
    .on_error(|error, cx| {
        cx.toast(Toast {
            message: error.to_string(),
            level: ToastLevel::Error,
        });
    })
    .start_with::<MyApp>()
    .await;
```

### Panic Behaviors

```rust
enum PanicBehavior {
    ShowError,     // Show error to user, app continues (degraded)
    RestartApp,    // Kill app, create fresh instance
    CrashRuntime,  // Propagate panic, terminate everything
}
```

| Behavior | Use Case |
|----------|----------|
| `ShowError` | Default. User sees error, can continue using app |
| `RestartApp` | For apps that can't recover from bad state |
| `CrashRuntime` | For critical apps where partial failure is unacceptable |

### Per-App Configuration

Override the default per app:

```rust
#[app(on_panic = RestartApp)]
struct QueueApp {
    // Critical background processor - restart on failure
}

#[app(on_panic = ShowError)]
struct ExplorerApp {
    // UI app - show error but keep running
}
```

### What Happens on Panic

When a panic is caught:

1. Framework catches the panic
2. Logs the error (stack trace, context)
3. Based on `PanicBehavior`:
   - **ShowError**: Calls global error handler, app state unchanged
   - **RestartApp**: Drops app, creates new instance, notifies user
   - **CrashRuntime**: Re-raises panic, runtime terminates

## Global Error Handler

Handle unhandled errors globally:

```rust
rafter::Runtime::new()
    .on_error(|error, cx| {
        // Log to file
        log::error!("{}", error);
        
        // Show to user
        cx.toast(Toast {
            message: format!("Error: {}", error),
            level: ToastLevel::Error,
            duration: Duration::from_secs(5),
        });
        
        // Could also open an error modal
        // cx.modal(ErrorModal { error });
    })
```

## Resource Errors

Resources have built-in error state:

```rust
enum Resource<T> {
    Idle,
    Loading,
    Progress(ProgressState),
    Ready(T),
    Error(ResourceError),  // Built-in error handling
}

// In view
match &self.records {
    Resource::Error(e) => {
        view! {
            column {
                text (color: error) { "Failed to load records" }
                text (color: text_muted) { e.to_string() }
                button (on_click: retry) { "Retry" }
            }
        }
    }
    // ...
}
```

### Retrying Failed Resources

```rust
#[handler]
async fn retry(&self, cx: AppContext) {
    // Just load again - Resource handles the state transitions
    self.load_records(cx).await;
}
```

## Error Boundaries

Catch rendering errors within a view subtree:

```rust
view! {
    column {
        error_boundary (fallback: error_fallback) {
            // If anything here fails to render, show fallback
            risky_component()
        }
    }
}

fn error_fallback(error: &RenderError) -> Node {
    view! {
        column (padding: 1, border: single, border_color: error) {
            text (color: error) { "Component failed to render" }
            text (color: text_muted) { error.message }
        }
    }
}
```

## Network Error Patterns

### Retry with Backoff

```rust
#[handler]
async fn load_with_retry(&self, cx: AppContext) {
    self.records.set(Resource::Loading);
    
    let mut attempts = 0;
    let max_attempts = 3;
    
    loop {
        match cx.api.get_records().await {
            Ok(data) => {
                self.records.set(Resource::Ready(data));
                return;
            }
            Err(e) if attempts < max_attempts => {
                attempts += 1;
                self.records.set(Resource::Progress(ProgressState {
                    current: attempts as u64,
                    total: Some(max_attempts as u64),
                    message: Some(format!("Retry {}/{}...", attempts, max_attempts)),
                }));
                tokio::time::sleep(Duration::from_secs(2_u64.pow(attempts))).await;
            }
            Err(e) => {
                self.records.set(Resource::Error(e.into()));
                return;
            }
        }
    }
}
```

### Graceful Degradation

```rust
#[handler]
async fn load_with_cache_fallback(&self, cx: AppContext) {
    self.records.set(Resource::Loading);
    
    match cx.api.get_records().await {
        Ok(data) => {
            self.cache.save(&data);
            self.records.set(Resource::Ready(data));
        }
        Err(e) => {
            // Try to use cached data
            if let Some(cached) = self.cache.load() {
                cx.toast("Using cached data (offline)");
                self.records.set(Resource::Ready(cached));
            } else {
                self.records.set(Resource::Error(e.into()));
            }
        }
    }
}
```

## Logging

Rafter integrates with standard Rust logging:

```rust
use log::{info, warn, error};

#[handler]
async fn load_records(&self, cx: AppContext) {
    info!("Loading records...");
    
    match cx.api.get_records().await {
        Ok(data) => {
            info!("Loaded {} records", data.len());
            self.records.set(Resource::Ready(data));
        }
        Err(e) => {
            error!("Failed to load records: {}", e);
            self.records.set(Resource::Error(e.into()));
        }
    }
}
```

Configure logging at runtime startup:

```rust
#[tokio::main]
async fn main() {
    env_logger::init();  // Or your preferred logger
    
    rafter::Runtime::new()
        .start_with::<MyApp>()
        .await;
}
```
