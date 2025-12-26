# Multi-App

Rafter supports running multiple app instances simultaneously with configurable behavior.

## Blur Policy

Control what happens when an app loses focus:

```rust
#[app(on_blur = Continue)]  // Keep running (default)
struct AppA { ... }

#[app(on_blur = Sleep)]     // Pause until refocused
struct AppB { ... }

#[app(on_blur = Close)]     // Close when losing focus
struct AppC { ... }
```

### BlurPolicy Options

| Policy | Description |
|--------|-------------|
| `Continue` | App keeps running in background, receives events |
| `Sleep` | App pauses, resumes when refocused |
| `Close` | App closes when focus moves away |

## Singleton Apps

Limit an app to one instance:

```rust
#[app(singleton)]
struct SettingsApp { ... }
```

With singleton, calling `spawn` when an instance exists will return an error. Use the built-in helper:

```rust
#[handler]
async fn open_settings(&self, cx: &AppContext) {
    // Gets existing instance or spawns new one
    match SettingsApp::get_or_spawn_and_focus(cx) {
        Ok(id) => log::info!("Settings app: {}", id),
        Err(e) => cx.toast(Toast::error(e.to_string())),
    }
}
```

## Spawning Apps

### Spawn and Focus

```rust
#[handler]
async fn open_editor(&self, cx: &AppContext) {
    let id = cx.spawn_and_focus(EditorApp {
        file_path: "example.txt".into(),
    })?;
}
```

### Spawn in Background

```rust
#[handler]
async fn start_worker(&self, cx: &AppContext) {
    let id = cx.spawn(WorkerApp::default())?;
    // Worker runs in background, current app stays focused
}
```

## Instance Management

### Focusing an Instance

```rust
#[handler]
async fn switch_to(&self, cx: &AppContext) {
    cx.focus_instance(target_id);
}
```

### Closing Instances

```rust
// Request close (respects on_close_request)
cx.close(instance_id);

// Force close (skips on_close_request)
cx.force_close(instance_id);
```

### Finding Instances

```rust
// List all running instances
let instances = cx.instances();

// Find instances of a specific type
let editors = cx.instances_of::<EditorApp>();

// Find first instance of a type
if let Some(id) = cx.instance_of::<SettingsApp>() {
    cx.focus_instance(id);
}

// Count instances
let count = cx.instance_count::<EditorApp>();
```

## Instance Info

`cx.instances()` returns `Vec<InstanceInfo>`:

```rust
struct InstanceInfo {
    pub id: InstanceId,        // Unique instance ID
    pub app_name: &'static str, // App type name
    pub title: String,          // Instance-specific title
    pub is_focused: bool,       // Currently has focus
    pub is_sleeping: bool,      // Paused (BlurPolicy::Sleep)
}
```

## Instance Discovery Pattern

```rust
#[handler]
async fn go_to_app_b(&self, cx: &AppContext) {
    // Check if instance exists
    if let Some(id) = cx.instance_of::<AppB>() {
        // Focus existing
        cx.focus_instance(id);
    } else {
        // Spawn new
        cx.spawn_and_focus(AppB::default())?;
    }
}
```

## Lifecycle Hooks

```rust
#[app_impl]
impl MyApp {
    /// Called when gaining focus
    async fn on_foreground(&self, cx: &AppContext) {
        cx.toast("Welcome back!");
        self.refresh_data().await;
    }

    /// Called when losing focus
    async fn on_background(&self, cx: &AppContext) {
        self.save_draft().await;
    }

    /// Called before close - return false to cancel
    fn on_close_request(&self, cx: &AppContext) -> bool {
        if self.has_unsaved_changes.get() {
            // Could show a modal here
            false  // Block close
        } else {
            true   // Allow close
        }
    }
}
```

## App Names and Titles

```rust
#[app(name = "File Editor")]
struct EditorApp {
    file_name: String,
}

#[app_impl]
impl EditorApp {
    /// Dynamic title for this instance
    fn title(&self) -> String {
        format!("Editor - {}", self.file_name.get())
    }
}
```

## Max Instances

Limit how many instances of an app can run:

```rust
#[app(max_instances = 5)]
struct WorkerApp { ... }
```

Attempting to spawn beyond the limit returns `SpawnError::MaxInstancesReached`.

## Cycling Between Apps

```rust
#[handler]
fn next_app(&self, cx: &AppContext) {
    let instances = cx.instances();
    if instances.len() <= 1 {
        return;
    }

    let current = instances.iter().position(|i| i.is_focused);
    if let Some(idx) = current {
        let next_idx = (idx + 1) % instances.len();
        cx.focus_instance(instances[next_idx].id);
    }
}
```

## Next Steps

- [Communication](communication.md) - Send messages between apps
- [Advanced: Systems](advanced/systems.md) - Global keybinds across all apps
