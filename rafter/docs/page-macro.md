# Page Macro

The `page!` macro provides a declarative DSL for building UI trees.

## Basic Syntax

```rust
fn page(&self) -> Node {
    page! {
        column {
            text { "Hello, world!" }
        }
    }
}
```

## Containers

### Column

Arranges children vertically:

```rust
page! {
    column {
        text { "First" }
        text { "Second" }
        text { "Third" }
    }
}
```

### Row

Arranges children horizontally:

```rust
page! {
    row {
        text { "Left" }
        text { "Right" }
    }
}
```

### Stack

Overlays children (last child on top):

```rust
page! {
    stack {
        text { "Background" }
        text { "Foreground" }
    }
}
```

## Layout Attributes

Apply layout properties to containers:

```rust
page! {
    column (padding: 2, gap: 1, border: rounded) {
        text { "Content" }
    }
}
```

### Available Attributes

| Attribute | Values | Description |
|-----------|--------|-------------|
| `padding` | number | Padding on all sides |
| `gap` | number | Space between children |
| `border` | `single`, `double`, `rounded`, `thick` | Border style |
| `width` | number or `fill` | Fixed width or fill available space |
| `height` | number or `fill` | Fixed height or fill available space |
| `flex` | number | Flex grow factor |
| `id` | string | Element identifier |

### Sizing Examples

```rust
page! {
    // Fixed size
    column (width: 40, height: 10) { ... }

    // Fill available space
    column (width: fill, height: fill) { ... }

    // Flex layout
    row {
        column (flex: 1) { text { "1/3" } }
        column (flex: 2) { text { "2/3" } }
    }
}
```

## Text Elements

### Basic Text

```rust
page! {
    text { "Plain text" }
    text { format!("Value: {}", self.value.get()) }
}
```

### Styled Text

```rust
page! {
    text (bold) { "Bold text" }
    text (italic) { "Italic text" }
    text (underline) { "Underlined" }
    text (fg: primary) { "Primary color" }
    text (fg: error, bold) { "Bold red text" }
    text (bg: surface) { "With background" }
}
```

### Named Colors

Theme colors available by name:
- `primary`, `secondary`
- `background`, `surface`
- `text`, `muted`
- `error`, `success`, `warning`, `info`

## Style Attributes

Containers and text support styling:

```rust
page! {
    column (bg: surface, border: rounded) {
        text (fg: primary, bold) { "Title" }
        text (fg: muted) { "Subtitle" }
    }
}
```

### Transitions

Animate style changes with `transition`:

```rust
page! {
    // id is required for transitions to track the element
    column (id: "panel", bg: {bg_color}, transition: 300, easing: ease_out) {
        text { "Animated background" }
    }
}
```

## Expressions

Embed Rust expressions with braces:

```rust
fn page(&self) -> Node {
    let count = self.count.get();
    let label = if count == 1 { "item" } else { "items" };

    page! {
        text { format!("{} {}", count, label) }
        text (fg: {if count > 10 { "success" } else { "warning" }}) {
            "Status"
        }
    }
}
```

## Conditionals

### If Statements

```rust
page! {
    column {
        if self.loading.get() {
            text { "Loading..." }
        }

        if self.error.get().is_some() {
            text (fg: error) { "Error occurred" }
        } else {
            text (fg: success) { "All good" }
        }
    }
}
```

### Match Expressions

```rust
page! {
    column {
        match self.status.get() {
            Status::Idle => {
                text { "Idle" }
            }
            Status::Loading => {
                text (fg: warning) { "Loading..." }
            }
            Status::Ready(data) => {
                text (fg: success) { format!("Loaded: {}", data) }
            }
            Status::Error(e) => {
                text (fg: error) { e.to_string() }
            }
        }
    }
}
```

## Loops

Iterate over collections:

```rust
page! {
    column {
        for item in self.items.get() {
            text { item.name.clone() }
        }
    }
}
```

With index:

```rust
page! {
    column {
        for (i, item) in self.items.get().iter().enumerate() {
            text { format!("{}. {}", i + 1, item.name) }
        }
    }
}
```

## Widgets

Widgets are embedded with their specific syntax:

```rust
page! {
    column {
        // Button
        button(id: "submit", label: "Submit", on_click: handle_submit)

        // Input
        input(bind: self.name)

        // List
        list(bind: self.items, on_activate: on_item_select)

        // Table
        table(bind: self.users, on_sort: on_sort)
    }
}
```

See [Widget Overview](widgets/overview.md) for all available widgets.

## Combining Patterns

A complete example:

```rust
fn page(&self) -> Node {
    let items = self.items.get();
    let count = items.len();
    let loading = self.loading.get();

    page! {
        column (padding: 1, gap: 1, bg: background) {
            // Header
            row {
                text (bold, fg: primary) { "My App" }
                column (flex: 1) {}  // Spacer
                text (fg: muted) { format!("{} items", count) }
            }

            // Content
            if loading {
                text (fg: warning) { "Loading..." }
            } else {
                column (border: rounded, flex: 1) {
                    for item in items {
                        row (gap: 2) {
                            text { item.id.to_string() }
                            text { item.name.clone() }
                        }
                    }
                }
            }

            // Actions
            row (gap: 1) {
                button(id: "refresh", label: "Refresh", on_click: refresh)
                button(id: "add", label: "Add", on_click: add_item)
            }
        }
    }
}
```

## Next Steps

- [Keybinds and Handlers](keybinds-and-handlers.md) - Handle user input
- [Styling](styling.md) - Customize colors and themes
- [Widgets](widgets/overview.md) - Available widgets
