# Select

A dropdown selection widget.

## State Field

```rust
#[app]
struct MyApp {
    country: Select,
}
```

## Item Trait

For custom types, implement `SelectItem`:

```rust
#[derive(Clone)]
struct Country {
    code: String,
    name: String,
}

impl SelectItem for Country {
    fn select_id(&self) -> String {
        self.code.clone()
    }

    fn select_label(&self) -> String {
        self.name.clone()
    }
}
```

## Basic Usage

With strings:

```rust
fn page(&self) -> Node {
    let options = vec!["Red", "Green", "Blue"];
    page! {
        select(bind: self.color, options: options)
    }
}
```

With custom types:

```rust
fn page(&self) -> Node {
    let countries = vec![
        Country { code: "US".into(), name: "United States".into() },
        Country { code: "UK".into(), name: "United Kingdom".into() },
    ];
    page! {
        select(bind: self.country, options: countries)
    }
}
```

## Initialization

```rust
async fn on_start(&self, _cx: &AppContext) {
    self.color.set_placeholder("Select a color");
    self.color.select_index(0);  // Select first option
}
```

## Attributes

| Attribute | Type | Description |
|-----------|------|-------------|
| `bind` | Select | The Select field to bind |
| `options` | Vec<T> | Available options |
| `on_change` | handler | Called when selection changes |

## Methods

| Method | Description |
|--------|-------------|
| `set_placeholder(s)` | Set placeholder text |
| `select_index(i)` | Select by index |
| `selected_index()` | Get selected index |
| `clear()` | Clear selection |

## Event Handler

```rust
page! {
    select(bind: self.priority, options: priorities, on_change: on_priority_change)
}

#[handler]
async fn on_priority_change(&self, cx: &AppContext) {
    if let Some(idx) = self.priority.selected_index() {
        cx.toast(format!("Priority set to index {}", idx));
    }
}
```

## Keyboard Navigation

- Tab: Move focus to/from select
- Enter/Space: Open dropdown
- Up/Down: Navigate options
- Enter: Confirm selection
- Escape: Close without selecting

## Complete Example

```rust
#[derive(Clone)]
struct Priority {
    level: u8,
    name: String,
}

impl SelectItem for Priority {
    fn select_id(&self) -> String { self.level.to_string() }
    fn select_label(&self) -> String { self.name.clone() }
}

#[app]
struct SelectDemo {
    fruit: Select,
    priority: Select,
}

#[app_impl]
impl SelectDemo {
    async fn on_start(&self, _cx: &AppContext) {
        self.fruit.set_placeholder("Choose a fruit");
        self.priority.set_placeholder("Select priority");
    }

    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "q" => quit,
        }
    }

    #[handler]
    async fn on_fruit_change(&self, cx: &AppContext) {
        let fruits = vec!["Apple", "Banana", "Cherry"];
        if let Some(idx) = self.fruit.selected_index() {
            cx.toast(format!("Fruit: {}", fruits[idx]));
        }
    }

    #[handler]
    async fn on_priority_change(&self, cx: &AppContext) {
        let priorities = get_priorities();
        if let Some(idx) = self.priority.selected_index() {
            cx.toast(format!("Priority: {}", priorities[idx].name));
        }
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        cx.exit();
    }

    fn page(&self) -> Node {
        let fruits = vec!["Apple", "Banana", "Cherry", "Date"];
        let priorities = get_priorities();

        page! {
            column (padding: 2, gap: 2) {
                text (bold) { "Select Demo" }

                column (gap: 1) {
                    text { "Fruit:" }
                    select(bind: self.fruit, options: fruits, on_change: on_fruit_change)
                }

                column (gap: 1) {
                    text { "Priority:" }
                    select(bind: self.priority, options: priorities, on_change: on_priority_change)
                }

                text (fg: muted) { "Tab to switch, Enter to open, q to quit" }
            }
        }
    }
}

fn get_priorities() -> Vec<Priority> {
    vec![
        Priority { level: 1, name: "Low".into() },
        Priority { level: 2, name: "Medium".into() },
        Priority { level: 3, name: "High".into() },
        Priority { level: 4, name: "Critical".into() },
    ]
}
```
