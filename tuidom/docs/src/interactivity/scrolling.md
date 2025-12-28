# Scrolling

tuidom provides scrollable containers with automatic scrollbar rendering.

## Making Elements Scrollable

Set `overflow` to `Scroll` or `Auto`:

```rust
Element::col()
    .id("my-list")
    .height(Size::Fixed(10))
    .overflow(Overflow::Scroll)
    .children(items.iter().map(|item| list_item(item)))
```

### Overflow Modes

| Mode | Behavior |
|------|----------|
| `Overflow::Visible` | Content extends beyond bounds (default) |
| `Overflow::Hidden` | Content is clipped, no scrolling |
| `Overflow::Scroll` | Always show scrollbar, enable scrolling |
| `Overflow::Auto` | Show scrollbar only when content overflows |

## Managing Scroll State

Use `ScrollState` to track scroll positions:

```rust
let mut scroll = ScrollState::new();

loop {
    let ui = build_ui(&scroll);
    term.render(&ui)?;

    let raw = term.poll(None)?;
    let events = focus.process_events(&raw, &ui, term.layout());

    // Update scroll state from events
    scroll.process_events(&events, &ui, term.layout());
}
```

## ScrollState Methods

### `get(id) -> ScrollOffset`

Get the current scroll offset:

```rust
let offset = scroll.get("my-list");
println!("Scrolled to ({}, {})", offset.x, offset.y);
```

### `set(id, x, y)`

Set scroll position directly:

```rust
scroll.set("my-list", 0, 10);  // Scroll to row 10
```

### `scroll_by(id, dx, dy) -> bool`

Scroll by a delta. Returns `true` if position changed:

```rust
scroll.scroll_by("my-list", 0, 5);  // Scroll down 5 rows
```

### `process_events(...)`

Automatically handle scroll events:

```rust
scroll.process_events(&events, &root, &layout);
```

This finds the scrollable element under the mouse and updates its scroll offset.

### `content_size(id) -> Option<(u16, u16)>`

Get the content size (after layout):

```rust
if let Some((width, height)) = scroll.content_size("my-list") {
    println!("Content is {}x{}", width, height);
}
```

## Building Scrollable UI

Apply scroll offset when building the element:

```rust
fn build_ui(scroll: &ScrollState) -> Element {
    let offset = scroll.get("my-list");

    Element::col()
        .id("my-list")
        .height(Size::Fixed(10))
        .overflow(Overflow::Scroll)
        .scroll_offset(offset.x, offset.y)  // Apply current offset
        .children(items.iter().map(|i| list_item(i)))
}
```

## Scrollbar Appearance

When `overflow` is `Scroll` or `Auto`, tuidom renders a scrollbar:

- Scrollbar appears on the right edge
- Uses `█` for the thumb and `░` for the track
- Thumb size is proportional to visible content

## Common Patterns

### Scrollable List

```rust
fn scrollable_list(items: &[String], scroll: &ScrollState) -> Element {
    let offset = scroll.get("list");

    Element::col()
        .id("list")
        .width(Size::Fill)
        .height(Size::Fixed(10))
        .overflow(Overflow::Scroll)
        .scroll_offset(offset.x, offset.y)
        .style(Style::new().border(Border::Single))
        .children(items.iter().enumerate().map(|(i, item)| {
            Element::text(format!("{}: {}", i + 1, item))
        }))
}
```

### Scroll to Selected

```rust
fn scroll_to_item(scroll: &mut ScrollState, selected: usize, visible_rows: u16) {
    let offset = scroll.get("list");
    let item_row = selected as u16;

    // Scroll up if selected is above viewport
    if item_row < offset.y {
        scroll.set("list", offset.x, item_row);
    }
    // Scroll down if selected is below viewport
    else if item_row >= offset.y + visible_rows {
        scroll.set("list", offset.x, item_row - visible_rows + 1);
    }
}
```

### Horizontal Scrolling

```rust
Element::row()
    .id("wide-content")
    .width(Size::Fixed(40))
    .overflow(Overflow::Scroll)
    .scroll_offset(scroll.get("wide-content").x, 0)
    .child(Element::text("A very long line of text that extends beyond the container width..."))
```

### Nested Scrollables

Inner scroll containers take priority:

```rust
Element::col()
    .id("outer")
    .overflow(Overflow::Scroll)
    .child(
        Element::col()
            .id("inner")
            .overflow(Overflow::Scroll)  // Mouse wheel scrolls this first
            .children(...)
    )
```

## ScrollOffset Struct

```rust
pub struct ScrollOffset {
    pub x: u16,
    pub y: u16,
}

ScrollOffset::new(0, 0)  // Create with values
ScrollOffset::default()  // Create at (0, 0)
```

## Utility Functions

### `collect_scrollable(element) -> Vec<String>`

Get all scrollable element IDs:

```rust
use tuidom::collect_scrollable;

let scrollable_ids = collect_scrollable(&root);
```
