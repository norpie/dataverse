# Apps

Apps are the primary unit of organization in Rafter. Each app is a self-contained module with its own state, views, and handlers.

## Defining an App

```rust
use rafter::prelude::*;

#[app]
struct ExplorerApp {
    records: Resource<Vec<Record>>,
    selected: usize,
    filter: String,
}

#[app_impl]
impl ExplorerApp {
    fn view(&self) -> Node {
        view! {
            column {
                text { "Explorer" }
            }
        }
    }
}
```

The `#[app]` macro:
- Wraps fields in `State<T>` for change tracking (see [State](./state.md))
- Registers the app via `inventory` (automatic discovery)
- Generates boilerplate for the app lifecycle

## Automatic Registration

Apps are registered automatically. No need to manually add them to a registry:

```rust
// Just define the app - it's automatically available
#[app]
struct MyApp { }

// The runtime discovers all registered apps
rafter::Runtime::new()
    .start_with::<LauncherApp>()  // Can start any registered app
    .await;
```

## Handlers

Handlers are the actor-style message handlers for your app. They respond to events, keybinds, and pub/sub messages.

### Sync Handlers

Sync handlers get `&mut self` and are used for fast UI updates:

```rust
#[handler]
fn select_next(&mut self) {
    self.selected += 1;
}

#[handler]
fn select_prev(&mut self) {
    self.selected = self.selected.saturating_sub(1);
}
```

### Async Handlers

Async handlers get `&self` and can only mutate `Resource<T>` or `#[async_state]` fields:

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

See [Async](./async.md) for more details on async patterns.

### Event Handlers

Handlers can receive events from UI interactions:

```rust
#[handler]
async fn handle_click(&mut self, event: ClickEvent, cx: AppContext) {
    match event.kind {
        ClickKind::Primary => {
            cx.navigate(RecordView { id: self.selected });
        }
        ClickKind::Secondary => {
            self.show_context_menu(cx).await;
        }
    }
}
```

### Pub/Sub Handlers

Handlers that accept a specific event type are automatically subscribed:

```rust
// This handler subscribes to RecordUpdated events automatically
#[handler]
async fn on_record_updated(&mut self, event: RecordUpdated, cx: AppContext) {
    self.refresh_record(event.id).await;
}

// Publishing from another app
cx.publish(RecordUpdated { id: record.id });
```

## Keybinds

Define keybinds using the `#[keybinds]` attribute:

```rust
#[app_impl]
impl ExplorerApp {
    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "j" | "down" => select_next,
            "k" | "up" => select_prev,
            "enter" => open_record,
            "q" => quit,
            "/" => search,
            "gg" => jump_to_top,
            "G" => jump_to_bottom,
            "ctrl+d" => page_down,
            "ctrl+u" => page_up,
        }
    }
}
```

### View-Scoped Keybinds

Keybinds can be scoped to specific views:

```rust
#[keybinds(view = ListView)]
fn list_keys() -> Keybinds {
    keybinds! {
        "enter" => open_record,
        "n" => new_record,
    }
}

#[keybinds(view = RecordView)]
fn record_keys() -> Keybinds {
    keybinds! {
        "e" => edit_field,
        "s" => save,
        "escape" => back_to_list,
    }
}
```

### Global Keybinds

Global keybinds apply across all views:

```rust
#[keybinds(global)]
fn global_keys() -> Keybinds {
    keybinds! {
        "ctrl+q" => quit_app,
        "?" => show_help,
    }
}
```

## App Context

The `AppContext` provides access to framework functionality:

```rust
#[handler]
async fn some_handler(&mut self, cx: AppContext) {
    // Navigation
    cx.navigate(RecordView { id: self.selected });
    
    // Modals
    let result = cx.modal(ConfirmModal { message: "Delete?" }).await;
    
    // Toasts
    cx.toast("Record saved");
    
    // Pub/sub
    cx.publish(RecordDeleted { id });
    
    // Focus
    cx.focus("search_input");
    
    // Theme
    cx.set_theme(MyTheme::dark());
    
    // Exit
    cx.exit();
}
```

## Panic Behavior

Configure per-app panic handling:

```rust
#[app(on_panic = RestartApp)]
struct QueueApp { }

#[app(on_panic = ShowError)]  // Default
struct ExplorerApp { }
```

Options:
- `ShowError` - Display error, app continues in degraded state
- `RestartApp` - Kill and restart the app fresh
- `CrashRuntime` - Propagate panic, crash everything
