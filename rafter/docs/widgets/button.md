# Button

A clickable button widget.

## Basic Usage

```rust
fn page(&self) -> Node {
    page! {
        button(id: "submit", label: "Submit", on_click: handle_click)
    }
}

#[handler]
async fn handle_click(&self, cx: &AppContext) {
    cx.toast("Button clicked!");
}
```

## Attributes

| Attribute | Type | Description |
|-----------|------|-------------|
| `id` | string | Widget identifier (required) |
| `label` | string | Button text |
| `on_click` | handler | Click handler |

## Dynamic Labels

Use expressions for dynamic text:

```rust
fn page(&self) -> Node {
    let count = self.count.get();
    page! {
        button(
            id: "counter",
            label: format!("Count: {}", count),
            on_click: increment
        )
    }
}
```

## Multiple Buttons

Use `cx.trigger_widget_id()` to identify which button was clicked:

```rust
fn page(&self) -> Node {
    page! {
        row (gap: 1) {
            button(id: "save", label: "Save", on_click: handle_action)
            button(id: "cancel", label: "Cancel", on_click: handle_action)
            button(id: "delete", label: "Delete", on_click: handle_action)
        }
    }
}

#[handler]
async fn handle_action(&self, cx: &AppContext) {
    match cx.trigger_widget_id().as_deref() {
        Some("save") => self.save(),
        Some("cancel") => self.cancel(),
        Some("delete") => self.delete(),
        _ => {}
    }
}
```

## Focus and Keyboard

- Buttons are focusable via Tab navigation
- Press Enter or Space to activate a focused button
- Focused buttons are visually highlighted

## Styling

Buttons use theme colors for styling:
- Normal: surface background
- Focused: primary color highlight
- Disabled: muted colors (when implemented)

## Complete Example

```rust
#[app]
struct ButtonDemo {
    count: i32,
}

#[app_impl]
impl ButtonDemo {
    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "q" => quit,
        }
    }

    #[handler]
    async fn increment(&self, _cx: &AppContext) {
        self.count.update(|v| *v += 1);
    }

    #[handler]
    async fn decrement(&self, _cx: &AppContext) {
        self.count.update(|v| *v -= 1);
    }

    #[handler]
    async fn reset(&self, cx: &AppContext) {
        self.count.set(0);
        cx.toast("Counter reset");
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        cx.exit();
    }

    fn page(&self) -> Node {
        let count = self.count.get();
        page! {
            column (padding: 2, gap: 1) {
                text (bold) { "Button Demo" }
                text { format!("Count: {}", count) }
                row (gap: 1) {
                    button(id: "dec", label: "-", on_click: decrement)
                    button(id: "inc", label: "+", on_click: increment)
                    button(id: "reset", label: "Reset", on_click: reset)
                }
            }
        }
    }
}
```
