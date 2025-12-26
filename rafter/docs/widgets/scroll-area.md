# ScrollArea

A scrollable container for content that exceeds the viewport.

## State Field

```rust
#[app]
struct MyApp {
    content: ScrollArea,
}
```

## Basic Usage

```rust
fn page(&self) -> Node {
    page! {
        scroll_area(bind: self.content, height: 10) {
            column {
                for i in 1..=50 {
                    text { format!("Line {}", i) }
                }
            }
        }
    }
}
```

## Attributes

| Attribute | Type | Description |
|-----------|------|-------------|
| `bind` | ScrollArea | The ScrollArea field to bind |
| `height` | number/fill | Viewport height |
| `width` | number/fill | Viewport width |
| `direction` | ScrollDirection | Vertical/Horizontal/Both |
| `scrollbar` | ScrollbarVisibility | When to show scrollbar |

## Scroll Direction

```rust
page! {
    // Vertical scrolling (default)
    scroll_area(bind: self.area, direction: Vertical) { ... }

    // Horizontal scrolling
    scroll_area(bind: self.area, direction: Horizontal) { ... }

    // Both directions
    scroll_area(bind: self.area, direction: Both) { ... }
}
```

## Scrollbar Visibility

```rust
page! {
    // Always show scrollbar
    scroll_area(bind: self.area, scrollbar: Always) { ... }

    // Show when scrolling (default)
    scroll_area(bind: self.area, scrollbar: Auto) { ... }

    // Never show scrollbar
    scroll_area(bind: self.area, scrollbar: Never) { ... }
}
```

## Methods

| Method | Description |
|--------|-------------|
| `scroll_to(offset)` | Scroll to position |
| `scroll_top()` | Scroll to top |
| `scroll_bottom()` | Scroll to bottom |
| `scroll_offset()` | Get current offset |

## Keyboard Navigation

When focused:
- Up/Down or j/k: Scroll vertically
- Left/Right or h/l: Scroll horizontally
- Page Up/Down: Scroll by page
- Home: Scroll to top
- End: Scroll to bottom

## Mouse Scrolling

ScrollArea responds to mouse wheel events for scrolling.

## Complete Example

```rust
#[app]
struct ScrollDemo {
    log: ScrollArea,
    messages: Vec<String>,
}

#[app_impl]
impl ScrollDemo {
    async fn on_start(&self, _cx: &AppContext) {
        let messages: Vec<String> = (1..=100)
            .map(|i| format!("Log entry #{}: Something happened", i))
            .collect();
        self.messages.set(messages);
    }

    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "g" => go_top,
            "G" => go_bottom,
            "q" => quit,
        }
    }

    #[handler]
    async fn go_top(&self, _cx: &AppContext) {
        self.log.scroll_top();
    }

    #[handler]
    async fn go_bottom(&self, _cx: &AppContext) {
        self.log.scroll_bottom();
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        cx.exit();
    }

    fn page(&self) -> Node {
        let messages = self.messages.get();

        page! {
            column (padding: 1, gap: 1) {
                text (bold) { format!("Log Viewer ({} entries)", messages.len()) }

                scroll_area(
                    bind: self.log,
                    height: fill,
                    direction: Vertical,
                    scrollbar: Auto
                ) {
                    column {
                        for (i, msg) in messages.iter().enumerate() {
                            row (gap: 2) {
                                text (fg: muted) { format!("{:>4}", i + 1) }
                                text { msg.clone() }
                            }
                        }
                    }
                }

                text (fg: muted) { "j/k scroll, g top, G bottom, q quit" }
            }
        }
    }
}
```

## Scrollbar Configuration

For more control over scrollbar appearance:

```rust
let config = ScrollbarConfig {
    visibility: ScrollbarVisibility::Auto,
    track_symbol: Some('|'),
    thumb_symbol: Some('#'),
};

page! {
    scroll_area(bind: self.area, scrollbar_config: config) { ... }
}
```

## With Lists and Trees

List, Tree, and Table widgets have built-in scrolling. Use ScrollArea for custom scrollable content:

```rust
page! {
    column {
        // List handles its own scrolling
        list(bind: self.items, height: 10)

        // Custom content needs ScrollArea
        scroll_area(bind: self.details, height: 5) {
            text { self.long_description.get() }
        }
    }
}
```
