# List

A selectable list of items with keyboard navigation.

## State Field

```rust
#[app]
struct MyApp {
    items: List<MyItem>,
}
```

## Item Trait

Implement `ListItem` for your data type:

```rust
#[derive(Clone)]
struct MyItem {
    id: String,
    name: String,
}

impl ListItem for MyItem {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn render(&self, focused: bool, selected: bool) -> Node {
        let marker = if selected { "[x]" } else { "[ ]" };
        page! {
            row (gap: 1) {
                text { marker }
                text (bold: focused) { self.name.clone() }
            }
        }
    }
}
```

## Basic Usage

```rust
fn page(&self) -> Node {
    page! {
        list(bind: self.items, height: fill)
    }
}
```

## Initialization

```rust
async fn on_start(&self, _cx: &AppContext) {
    let items = vec![
        MyItem { id: "1".into(), name: "First".into() },
        MyItem { id: "2".into(), name: "Second".into() },
        MyItem { id: "3".into(), name: "Third".into() },
    ];
    self.items.set_items(items);
    self.items.set_selection_mode(SelectionMode::Single);
}
```

## Attributes

| Attribute | Type | Description |
|-----------|------|-------------|
| `bind` | List<T> | The List field to bind |
| `on_activate` | handler | Called on Enter key |
| `on_selection_change` | handler | Called when selection changes |
| `on_cursor_move` | handler | Called when cursor moves |
| `height` | number/fill | List height |
| `width` | number/fill | List width |

## Methods

| Method | Description |
|--------|-------------|
| `set_items(vec)` | Set list items |
| `items()` | Get all items |
| `len()` | Item count |
| `set_selection_mode(mode)` | Single/Multiple/None |
| `set_cursor(index)` | Move cursor to index |
| `cursor_index()` | Get cursor index |
| `cursor_id()` | Get cursor item ID |
| `select(id)` | Select item by ID |
| `deselect(id)` | Deselect item by ID |
| `toggle_selection(id)` | Toggle selection |
| `select_all()` | Select all items |
| `deselect_all()` | Clear selection |
| `selected_ids()` | Get selected IDs |
| `selected_items()` | Get selected items |

## Selection Modes

```rust
// Single selection (default)
self.items.set_selection_mode(SelectionMode::Single);

// Multiple selection
self.items.set_selection_mode(SelectionMode::Multiple);

// No selection (navigation only)
self.items.set_selection_mode(SelectionMode::None);
```

## Event Handlers

### on_activate

Called when Enter is pressed:

```rust
#[handler]
async fn on_item_activate(&self, cx: &AppContext) {
    if let Some(id) = cx.activated_id() {
        cx.toast(format!("Opened: {}", id));
    }
    // Or use index
    if let Some(index) = cx.activated_index() {
        let item = &self.items.items()[index];
    }
}
```

### on_selection_change

Called when selection changes:

```rust
#[handler]
async fn on_selection(&self, cx: &AppContext) {
    if let Some(ids) = cx.selected_ids() {
        cx.toast(format!("{} items selected", ids.len()));
    }
}
```

### on_cursor_move

Called when cursor moves:

```rust
#[handler]
async fn on_cursor(&self, cx: &AppContext) {
    if let Some(id) = cx.cursor_id() {
        // Preview the item
    }
}
```

## Keyboard Navigation

- Up/Down or j/k: Move cursor
- Space: Toggle selection (Multiple mode)
- Enter: Activate item
- a: Select all (Multiple mode)
- Ctrl+a: Deselect all

## Complete Example

```rust
#[derive(Clone)]
struct Task {
    id: String,
    title: String,
    done: bool,
}

impl ListItem for Task {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn render(&self, focused: bool, selected: bool) -> Node {
        let check = if self.done { "[x]" } else { "[ ]" };
        let style = if self.done { "muted" } else { "text" };
        page! {
            row (gap: 1) {
                text { check }
                text (fg: {style}, bold: focused) { self.title.clone() }
            }
        }
    }
}

#[app]
struct TaskList {
    tasks: List<Task>,
}

#[app_impl]
impl TaskList {
    async fn on_start(&self, _cx: &AppContext) {
        self.tasks.set_items(vec![
            Task { id: "1".into(), title: "Buy groceries".into(), done: false },
            Task { id: "2".into(), title: "Walk the dog".into(), done: true },
            Task { id: "3".into(), title: "Write code".into(), done: false },
        ]);
    }

    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "q" => quit,
        }
    }

    #[handler]
    async fn on_activate(&self, cx: &AppContext) {
        if let Some(index) = cx.activated_index() {
            // Toggle done status
            let mut items = self.tasks.items();
            if let Some(task) = items.get_mut(index) {
                task.done = !task.done;
            }
            self.tasks.set_items(items);
        }
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        cx.exit();
    }

    fn page(&self) -> Node {
        let count = self.tasks.len();
        page! {
            column (padding: 1, gap: 1) {
                text (bold) { format!("Tasks ({})", count) }
                list(bind: self.tasks, on_activate: on_activate, height: fill)
                text (fg: muted) { "Enter to toggle, q to quit" }
            }
        }
    }
}
```
