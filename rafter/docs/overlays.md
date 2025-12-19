# Overlays

Rafter provides several overlay types for different use cases.

## Layer Hierarchy

```
┌─────────────────────────────────────────┐
│ System Overlays (taskbar, app launcher) │  Always top
├─────────────────────────────────────────┤
│ Toasts (corner stack)                   │  Visual only, click to dismiss
├─────────────────────────────────────────┤
│ Modal Stack                             │  Blocking, captures input
├─────────────────────────────────────────┤
│ Persistent Overlays (progress, etc)     │  Non-blocking
├─────────────────────────────────────────┤
│ App View                                │  Main content
└─────────────────────────────────────────┘
```

## Modals

Modals are blocking overlays that capture all input until closed.

### Opening a Modal

```rust
#[handler]
async fn delete_record(&mut self, cx: AppContext) {
    let result = cx.modal(ConfirmModal {
        title: "Delete Record".into(),
        message: format!("Delete {}?", self.selected.name),
    }).await;  // Suspends until modal closes
    
    if result == ConfirmResult::Confirmed {
        cx.publish(DeleteOperation { id: self.selected.id });
    }
}
```

### Defining a Modal

```rust
#[modal]
struct ConfirmModal {
    title: String,
    message: String,
}

#[modal_impl]
impl ConfirmModal {
    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "y" | "enter" => confirm,
            "n" | "escape" => cancel,
        }
    }

    fn confirm(&mut self, cx: ModalContext) {
        cx.emit(ConfirmResult::Confirmed);
        cx.close();
    }

    fn cancel(&mut self, cx: ModalContext) {
        cx.emit(ConfirmResult::Cancelled);
        cx.close();
    }

    fn view(&self) -> Node {
        view! {
            column (border: rounded, padding: 2, bg: surface) {
                text (bold) { self.title }
                text { self.message }
                row (gap: 2, justify: end) {
                    button { "[N]o" }
                    button (focused) { "[Y]es" }
                }
            }
        }
    }
}
```

### Modal Stacking

Modals can open other modals - they stack:

```rust
// TaskManager modal
#[handler]
async fn kill_task(&mut self, cx: ModalContext) {
    // Opens on top of TaskManager
    let result = cx.modal(ConfirmModal {
        message: "Kill this task?".into(),
    }).await;
    
    if result == Confirmed {
        cx.publish(KillTask { id: self.selected });
    }
}
```

When the inner modal closes, focus returns to the outer modal.

### Reusable Modals

Modals are self-contained with their own keybinds:

```rust
// Generic confirm modal - use anywhere
let result = cx.modal(ConfirmModal {
    title: "Confirm",
    message: "Are you sure?",
}).await;

// Generic input modal
let name = cx.modal(InputModal {
    title: "Rename",
    placeholder: "Enter new name",
    initial: self.selected.name.clone(),
}).await;

// Generic select modal
let choice = cx.modal(SelectModal {
    title: "Choose action",
    options: vec!["Edit", "Delete", "Copy"],
}).await;
```

## Toasts

Toasts are non-blocking notifications that appear in a corner.

### Simple Toast

```rust
cx.toast("Record saved");
```

### Configured Toast

```rust
cx.toast(Toast {
    message: "Operation failed".into(),
    level: ToastLevel::Error,
    duration: Duration::from_secs(5),
});
```

### Toast Levels

```rust
enum ToastLevel {
    Info,     // Default
    Success,
    Warning,
    Error,
}
```

### Toast Behavior

- Appear in corner (bottom-right by default)
- Stack upward as more appear
- Auto-dismiss after duration
- Can be clicked to dismiss
- Animate in/out with swipe (respects reduce_motion)

## Persistent Overlays

Non-blocking overlays for ongoing operations.

### Progress Overlay

```rust
#[handler]
async fn export_data(&mut self, cx: AppContext) {
    let progress = cx.overlay(ProgressOverlay::new("Exporting..."));
    
    for (i, record) in self.records.iter().enumerate() {
        export_record(record).await;
        progress.update((i + 1) as f32 / self.records.len() as f32);
    }
    
    progress.close();
    cx.toast("Export complete");
}
```

### Custom Overlay

```rust
let overlay = cx.overlay(MyCustomOverlay { ... });
// User can still interact with app
overlay.update(new_state);
overlay.close();
```

## System Overlays

System overlays are runtime-level and always on top.

### Taskbar

```rust
#[system_overlay]
struct Taskbar {
    position: Position::Bottom,
    height: 1,
}
```

### App Launcher

```rust
#[system_overlay]
struct AppLauncher {
    trigger: "ctrl+space",
    style: Overlay::Centered { width: 50%, height: 60% },
}
```

### Configuring System Overlays

```rust
rafter::Runtime::new()
    .system_keybind("ctrl+space", SystemAction::AppLauncher)
    .system_keybind("ctrl+tab", SystemAction::NextApp)
```

## Focus and Input

| Overlay Type | Captures Input | Blocks Below |
|--------------|----------------|--------------|
| System (active) | Yes | Yes |
| Toast | Dismiss only | No |
| Modal | Yes | Yes |
| Persistent | No | No |

When a modal opens:
1. Focus moves to first focusable element in modal (or specified element)
2. Tab/Shift+Tab cycles within modal only
3. Modal keybinds take precedence
4. When closed, focus returns to previous location
