# Collapsible

An expandable/collapsible section with a header.

## State Field

```rust
#[app]
struct MyApp {
    details: Collapsible,
}
```

## Basic Usage

```rust
fn page(&self) -> Node {
    page! {
        collapsible(bind: self.details) {
            text { "Hidden content here" }
        }
    }
}
```

## Initialization

```rust
async fn on_start(&self, _cx: &AppContext) {
    self.details.set_title("Details");
    self.details.expand();  // Start expanded
    // or
    self.details.collapse();  // Start collapsed (default)
}
```

## Attributes

| Attribute | Type | Description |
|-----------|------|-------------|
| `bind` | Collapsible | The Collapsible field to bind |
| `on_expand` | handler | Called when expanded |
| `on_collapse` | handler | Called when collapsed |

## Methods

| Method | Description |
|--------|-------------|
| `set_title(s)` | Set header title |
| `expand()` | Expand the section |
| `collapse()` | Collapse the section |
| `toggle()` | Toggle expand/collapse |
| `is_expanded()` | Check if expanded |

## Event Handlers

```rust
page! {
    collapsible(bind: self.advanced, on_expand: on_show_advanced) {
        // content
    }
}

#[handler]
async fn on_show_advanced(&self, cx: &AppContext) {
    cx.toast("Advanced options revealed!");
}
```

## Keyboard Navigation

- Tab: Move focus to header
- Space/Enter: Toggle expand/collapse

## Complete Example

```rust
#[app]
struct CollapsibleDemo {
    basic: Collapsible,
    advanced: Collapsible,
    help: Collapsible,
}

#[app_impl]
impl CollapsibleDemo {
    async fn on_start(&self, _cx: &AppContext) {
        self.basic.set_title("Basic Settings");
        self.basic.expand();

        self.advanced.set_title("Advanced Settings");
        // Starts collapsed

        self.help.set_title("Help & Documentation");
    }

    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "q" => quit,
        }
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        cx.exit();
    }

    fn page(&self) -> Node {
        page! {
            column (padding: 2, gap: 1) {
                text (bold) { "Collapsible Demo" }

                collapsible(bind: self.basic) {
                    column (padding: 1, gap: 1) {
                        text { "Option 1: Enabled" }
                        text { "Option 2: Disabled" }
                    }
                }

                collapsible(bind: self.advanced) {
                    column (padding: 1, gap: 1) {
                        text (fg: warning) { "Warning: Advanced settings!" }
                        text { "Debug mode: Off" }
                        text { "Verbose logging: Off" }
                    }
                }

                collapsible(bind: self.help) {
                    column (padding: 1) {
                        text { "Use Tab to navigate between sections." }
                        text { "Press Space or Enter to expand/collapse." }
                    }
                }

                text (fg: muted) { "Tab to navigate, Space to toggle, q to quit" }
            }
        }
    }
}
```
