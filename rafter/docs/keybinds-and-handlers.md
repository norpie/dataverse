# Keybinds and Handlers

This guide covers keyboard input handling in rafter.

## Defining Keybinds

Use the `#[keybinds]` attribute and `keybinds!` macro:

```rust
#[app_impl]
impl MyApp {
    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "q" => quit,
            "r" => refresh,
            "ctrl+s" => save,
        }
    }
}
```

## Key Notation

### Single Keys

```rust
keybinds! {
    "a" => handler_a,       // Letter
    "1" => handler_1,       // Number
    "space" => handler_space,
    "enter" => handler_enter,
    "escape" => handler_escape,
    "tab" => handler_tab,
    "backspace" => handler_backspace,
    "delete" => handler_delete,
}
```

### Arrow Keys

```rust
keybinds! {
    "up" => move_up,
    "down" => move_down,
    "left" => move_left,
    "right" => move_right,
}
```

### Modifiers

```rust
keybinds! {
    "ctrl+s" => save,
    "ctrl+shift+s" => save_as,
    "alt+f" => file_menu,
    "shift+tab" => focus_prev,
}
```

Available modifiers: `ctrl`, `alt`, `shift`

### Multiple Keys

Map multiple keys to the same handler with `|`:

```rust
keybinds! {
    "q" | "escape" => quit,
    "j" | "down" => move_down,
    "k" | "up" => move_up,
    "enter" | "space" => activate,
}
```

## Handlers

Handlers are async functions marked with `#[handler]`:

```rust
#[handler]
async fn my_handler(&self, cx: &AppContext) {
    // Handle the event
    cx.toast("Handler called!");
}
```

### Handler Signatures

```rust
// With AppContext
#[handler]
async fn with_context(&self, cx: &AppContext) { ... }

// Without AppContext (if you don't need it)
#[handler]
async fn without_context(&self) { ... }
```

### State Access

Handlers can read and modify state:

```rust
#[handler]
async fn increment(&self, _cx: &AppContext) {
    // Read
    let current = self.counter.get();

    // Write
    self.counter.set(current + 1);

    // Or update in place
    self.counter.update(|v| *v += 1);
}
```

### Async Operations

Handlers are async and can await:

```rust
#[handler]
async fn load_data(&self, cx: &AppContext) {
    self.loading.set(true);

    let result = fetch_data().await;

    self.loading.set(false);
    self.data.set(result);
}
```

## Widget Event Handlers

Widgets emit events that can be handled:

### Button

```rust
page! {
    button(id: "submit", label: "Submit", on_click: handle_submit)
}

#[handler]
async fn handle_submit(&self, cx: &AppContext) {
    cx.toast("Button clicked!");
}
```

### Input

```rust
page! {
    input(bind: self.name, on_change: on_name_change, on_submit: on_name_submit)
}

#[handler]
async fn on_name_change(&self, _cx: &AppContext) {
    // Called on every keystroke
    let value = self.name.value();
}

#[handler]
async fn on_name_submit(&self, cx: &AppContext) {
    // Called when Enter is pressed
    cx.toast(format!("Submitted: {}", self.name.value()));
}
```

### List/Tree/Table

```rust
page! {
    list(
        bind: self.items,
        on_activate: on_item_activate,
        on_selection_change: on_selection,
        on_cursor_move: on_cursor
    )
}

#[handler]
async fn on_item_activate(&self, cx: &AppContext) {
    if let Some(id) = cx.activated_id() {
        cx.toast(format!("Activated: {}", id));
    }
}

#[handler]
async fn on_selection(&self, cx: &AppContext) {
    if let Some(ids) = cx.selected_ids() {
        cx.toast(format!("Selected {} items", ids.len()));
    }
}
```

### Checkbox

```rust
page! {
    checkbox(bind: self.agree, on_change: on_agree_change)
}

#[handler]
async fn on_agree_change(&self, _cx: &AppContext) {
    let checked = self.agree.is_checked();
}
```

## Widget ID Access

When a handler is triggered by a widget, you can get the widget's ID:

```rust
#[handler]
async fn on_button_click(&self, cx: &AppContext) {
    if let Some(id) = cx.trigger_widget_id() {
        match id.as_str() {
            "btn-save" => self.save(),
            "btn-cancel" => self.cancel(),
            _ => {}
        }
    }
}
```

## Event Context Data

Different widgets provide context data:

```rust
// List/Tree/Table activation
cx.activated_id()      // String ID
cx.activated_index()   // usize index (List only)

// Selection changes
cx.selected_ids()      // Vec<String>

// Cursor movement
cx.cursor_id()         // String ID
cx.cursor_index()      // usize index (List only)

// Tree expand/collapse
cx.expanded_id()       // String ID of expanded node
cx.collapsed_id()      // String ID of collapsed node

// Table sorting
cx.sorted_column()     // (column_index, ascending)
```

## Focus Management

### Tab Navigation

Widgets are automatically focusable. Tab/Shift+Tab cycles through them.

### Programmatic Focus

```rust
#[handler]
async fn focus_input(&self, cx: &AppContext) {
    cx.focus("my-input");
}
```

## Keybind Priority

Keybinds are matched in this order (highest to lowest):

1. **System keybinds** - Global, always active
2. **Modal keybinds** - When a modal is open
3. **App keybinds** - The focused app's keybinds
4. **Widget keybinds** - Built-in widget navigation

## Next Steps

- [Styling](styling.md) - Customize appearance
- [Modals](modals.md) - Dialog overlays
- [Widgets](widgets/overview.md) - Available widgets
