# Interactions

Rafter treats keyboard and mouse as equal first-class input methods.

## Click Events

Attach click handlers to interactive elements:

```rust
view! {
    row (on_click: handle_click) {
        text { record.name }
    }
    
    button (on_click: save) { "Save" }
}
```

### Event Details

Handlers receive event information:

```rust
#[handler]
async fn handle_click(&mut self, event: ClickEvent, cx: AppContext) {
    match event.kind {
        ClickKind::Primary => {
            // Enter key or left mouse click
            self.open_record();
        }
        ClickKind::Secondary => {
            // Shift+Enter or right mouse click
            self.show_context_menu(cx).await;
        }
    }
}
```

### ClickEvent Structure

```rust
struct ClickEvent {
    kind: ClickKind,
    position: Position,
    modifiers: Modifiers,
}

enum ClickKind {
    Primary,    // Enter, left-click
    Secondary,  // Shift+Enter, right-click
}

struct Modifiers {
    ctrl: bool,
    shift: bool,
    alt: bool,
}
```

### Modifier Keys

Use modifiers for power-user features:

```rust
#[handler]
fn handle_click(&mut self, event: ClickEvent) {
    if event.modifiers.ctrl {
        // Ctrl+click: multi-select
        self.selection.toggle(self.focused);
    } else {
        // Normal click: single select
        self.selection.set(self.focused);
    }
}
```

## Focus

### Focus Behavior

- **Tab / Shift+Tab**: Move focus to next/previous focusable element
- **Click**: Focus and activate element
- **Enter**: Activate focused element

### Focusable Elements

By default, interactive elements are focusable:
- `button`
- `input`
- `select`
- `checkbox`
- `radio`
- Elements with `on_click` handler

Text and layout elements are not focusable by default.

### Focus Attributes

```rust
view! {
    // Start with focus
    button (focused) { "Save" }
    
    // Disable focus
    button (focusable: false) { "Not focusable" }
    
    // Custom tab order
    input (tab_index: 2) { }
    input (tab_index: 1) { }  // Focused first
}
```

### Programmatic Focus

```rust
#[handler]
fn handle_error(&mut self, cx: AppContext) {
    cx.toast("Invalid email");
    cx.focus("email_input");  // Move focus to input
}
```

### Focus Restoration

Focus is automatically restored when:
- Modal closes: Focus returns to element that was focused before modal opened
- App switches: Focus returns to last focused element in that app
- View changes: Configurable (first focusable or remember last)

## Scrolling

### Mouse Scroll

- Scroll wheel scrolls the region under the cursor (not based on focus)
- More intuitive for mouse users

### Keyboard Scroll

```rust
#[keybinds]
fn keys() -> Keybinds {
    keybinds! {
        "ctrl+d" => page_down,
        "ctrl+u" => page_up,
        "gg" => scroll_to_top,
        "G" => scroll_to_bottom,
    }
}
```

## Keybinds

### Defining Keybinds

```rust
#[keybinds]
fn keys() -> Keybinds {
    keybinds! {
        "j" | "down" => select_next,
        "k" | "up" => select_prev,
        "enter" => open,
        "q" => quit,
        "/" => search,
        "gg" => jump_to_top,      // Vim-style sequences
        "G" => jump_to_bottom,
    }
}
```

### Key Syntax

```rust
keybinds! {
    // Simple keys
    "a" => action,
    "enter" => action,
    "escape" => action,
    "space" => action,
    "tab" => action,
    
    // With modifiers
    "ctrl+s" => save,
    "ctrl+shift+s" => save_as,
    "alt+enter" => action,
    
    // Alternatives
    "j" | "down" => next,
    
    // Sequences
    "gg" => top,
    "dd" => delete,
}
```

### Keybind Scopes

```rust
// App-level (always active for this app)
#[keybinds]
fn app_keys() -> Keybinds { }

// View-specific
#[keybinds(view = ListView)]
fn list_keys() -> Keybinds { }

#[keybinds(view = RecordView)]
fn record_keys() -> Keybinds { }

// Modal-specific
#[keybinds(modal = ConfirmModal)]
fn confirm_keys() -> Keybinds { }

// Global (all apps, lowest priority)
#[keybinds(global)]
fn global_keys() -> Keybinds { }
```

### Priority Order

Keybinds are checked in this order (first match wins):

1. **System** - Runtime-level (`ctrl+space` for launcher)
2. **Modal** - Active modal's keybinds
3. **View** - Current view's keybinds
4. **App** - App-level keybinds
5. **Global** - Cross-app keybinds

### Reserved System Keybinds

Some keybinds are reserved at the runtime level:

```rust
rafter::Runtime::new()
    .system_keybind("ctrl+space", SystemAction::AppLauncher)
    .system_keybind("ctrl+tab", SystemAction::NextApp)
    .system_keybind("ctrl+alt+q", SystemAction::ForceQuit)
```

These cannot be overridden by apps.

## Input Events

### Text Input

```rust
view! {
    input (
        on_change: handle_input,
        on_submit: handle_submit,
    ) { }
}

#[handler]
fn handle_input(&mut self, event: InputEvent) {
    self.filter = event.value.clone();
}

#[handler]
async fn handle_submit(&mut self, event: SubmitEvent, cx: AppContext) {
    self.search(event.value).await;
}
```

### InputEvent Structure

```rust
struct InputEvent {
    value: String,
    modifiers: Modifiers,
}

struct SubmitEvent {
    value: String,
}
```
