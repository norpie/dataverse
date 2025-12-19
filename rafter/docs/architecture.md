# Architecture

Rafter uses a layered architecture with a runtime managing multiple concurrent apps.

## Overview

```
Rafter Runtime
├── App Manager (foreground/background switching)
├── Event Bus (pub/sub between apps)
├── Theme (shared across all apps)
└── Apps[]
    ├── Launcher App
    ├── Explorer App
    ├── Queue App
    └── ...
```

## Runtime

The runtime is the top-level container that owns the terminal, async runtime, and event loop.

```rust
#[tokio::main]
async fn main() {
    rafter::Runtime::new()
        .system_keybind("ctrl+space", SystemAction::AppLauncher)
        .system_keybind("ctrl+tab", SystemAction::NextApp)
        .on_panic(PanicBehavior::ShowError)
        .on_error(|error, cx| {
            cx.toast(Toast {
                message: error.to_string(),
                level: ToastLevel::Error,
            });
        })
        .start_with::<LauncherApp>()
        .await;
}
```

### Runtime Responsibilities

- Terminal initialization and teardown
- Event loop (keyboard, mouse, resize)
- App lifecycle management
- System-level keybinds
- Theme management
- Error/panic handling

## Apps

Apps are self-contained units with their own state and views. Multiple apps can run concurrently - one in the foreground (visible), others in the background (still processing events).

```rust
#[app]
struct ExplorerApp {
    records: Resource<Vec<Record>>,
    selected: usize,
}
```

Apps are registered automatically via the `#[app]` macro using the `inventory` crate. No manual registration needed.

### App Communication

Apps communicate through a pub/sub event bus:

```rust
// Publisher
cx.publish(RecordUpdated { id: record.id });

// Subscriber (handler exists = subscribed)
#[handler]
async fn on_record_updated(&mut self, event: RecordUpdated, cx: AppContext) {
    self.refresh_record(event.id).await;
}
```

### Foreground vs Background

- **Foreground**: Visible, receives user input
- **Background**: Not visible, but still processes pub/sub events

Use case: A queue app runs in the background, receiving operations from other apps and processing them.

## Layer System

Rafter uses a layer stack for rendering and input handling:

```
┌─────────────────────────────────────────┐
│ System Overlays (taskbar, app launcher) │  Layer 4: Always top
├─────────────────────────────────────────┤
│ Toasts (corner stack, no input capture) │  Layer 3: Visual only
├─────────────────────────────────────────┤
│ Modal Stack (captures input)            │  Layer 2: Blocking interaction
├─────────────────────────────────────────┤
│ Persistent Overlays (progress, etc)     │  Layer 1: Non-blocking
├─────────────────────────────────────────┤
│ App View                                │  Layer 0: Main content
└─────────────────────────────────────────┘
```

### Layer Behaviors

| Layer | Renders | Captures Input | Blocks Below |
|-------|---------|----------------|--------------|
| System Overlays | Yes | When active | Yes |
| Toasts | Yes | Dismiss only | No |
| Modal Stack | Yes | Yes | Yes |
| Persistent Overlays | Yes | No | No |
| App View | Yes | When no modal | N/A |

### Input Priority

Input events flow top-down through the layer stack:

1. **System keybinds** - Always checked first (`ctrl+space`, etc.)
2. **System overlays** - When active (app launcher open)
3. **Modal stack** - Top modal captures all input
4. **View keybinds** - Current view's keybinds
5. **App keybinds** - App-level keybinds
6. **Global keybinds** - Fallback keybinds

## Render Loop

Rafter uses an efficient render strategy:

- **Idle**: No renders when nothing changes
- **User input**: Re-render on keyboard/mouse events
- **State change**: Re-render when state mutates
- **Animation**: Active render loop (30-60fps) during animations
- **Background events**: Re-render when pub/sub events modify state

This ensures responsiveness while minimizing CPU usage.
