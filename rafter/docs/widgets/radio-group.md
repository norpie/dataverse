# RadioGroup

Mutually exclusive option selection.

## State Field

```rust
#[app]
struct MyApp {
    priority: RadioGroup,
}
```

## Basic Usage

```rust
fn page(&self) -> Node {
    page! {
        radio_group(bind: self.priority)
    }
}
```

## Initialization

```rust
async fn on_start(&self, _cx: &AppContext) {
    self.priority.set_options(vec!["Low", "Medium", "High"]);
    self.priority.select(1);  // Select "Medium"
}
```

## Attributes

| Attribute | Type | Description |
|-----------|------|-------------|
| `bind` | RadioGroup | The RadioGroup field to bind |
| `on_change` | handler | Called when selection changes |

## Methods

| Method | Description |
|--------|-------------|
| `set_options(vec)` | Set available options |
| `select(index)` | Select option by index |
| `selected_index()` | Get selected index (Option<usize>) |
| `selected_label()` | Get selected label (Option<String>) |

## Event Handler

```rust
page! {
    radio_group(bind: self.theme, on_change: on_theme_change)
}

#[handler]
async fn on_theme_change(&self, cx: &AppContext) {
    if let Some(label) = self.theme.selected_label() {
        cx.toast(format!("Theme: {}", label));
    }
}
```

## Validation

```rust
let result = Validator::new()
    .field(&self.priority, "priority")
        .selected("Please select a priority")
    .validate();
```

## Keyboard Navigation

- Tab: Move focus to/from the group
- Up/Down: Navigate between options
- Space/Enter: Select current option

## Complete Example

```rust
#[app]
struct RadioDemo {
    size: RadioGroup,
    color: RadioGroup,
}

#[app_impl]
impl RadioDemo {
    async fn on_start(&self, _cx: &AppContext) {
        self.size.set_options(vec!["Small", "Medium", "Large"]);
        self.size.select(1);

        self.color.set_options(vec!["Red", "Green", "Blue"]);
        self.color.select(0);
    }

    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "q" => quit,
        }
    }

    #[handler]
    async fn on_size_change(&self, cx: &AppContext) {
        if let Some(size) = self.size.selected_label() {
            cx.toast(format!("Size: {}", size));
        }
    }

    #[handler]
    async fn on_color_change(&self, cx: &AppContext) {
        if let Some(color) = self.color.selected_label() {
            cx.toast(format!("Color: {}", color));
        }
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        cx.exit();
    }

    fn page(&self) -> Node {
        page! {
            column (padding: 2, gap: 2) {
                text (bold) { "RadioGroup Demo" }

                row (gap: 4) {
                    column (gap: 1) {
                        text { "Size:" }
                        radio_group(bind: self.size, on_change: on_size_change)
                    }

                    column (gap: 1) {
                        text { "Color:" }
                        radio_group(bind: self.color, on_change: on_color_change)
                    }
                }

                text (fg: muted) { "Up/Down to navigate, q to quit" }
            }
        }
    }
}
```
