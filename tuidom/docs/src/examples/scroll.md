# Scrollable Lists

The scroll example demonstrates scrollable containers and scroll state management.

## Running

```bash
cargo run --example scroll
```

## Features Demonstrated

- Scrollable containers with `Overflow::Scroll`
- ScrollState for tracking scroll position
- Mouse wheel scrolling
- Scroll position indicators
- Content size calculations

## Key Patterns

### ScrollState Setup

```rust
let mut term = Terminal::new()?;
let mut focus = FocusState::new();
let mut scroll = ScrollState::new();

loop {
    let ui = build_ui(&scroll);
    term.render(&ui)?;

    let raw = term.poll(None)?;
    let events = focus.process_events(&raw, &ui, term.layout());

    // Update scroll from events
    scroll.process_events(&events, &ui, term.layout());
}
```

### Scrollable List

```rust
fn scrollable_list(items: &[String], scroll: &ScrollState) -> Element {
    let offset = scroll.get("my-list");

    Element::col()
        .id("my-list")
        .width(Size::Fill)
        .height(Size::Fixed(10))
        .overflow(Overflow::Scroll)
        .scroll_offset(offset.x, offset.y)
        .style(Style::new().border(Border::Single))
        .children(items.iter().enumerate().map(|(i, item)| {
            Element::text(format!("{:3}. {}", i + 1, item))
        }))
}
```

### Scroll Position Display

```rust
fn scroll_indicator(scroll: &ScrollState, id: &str) -> Element {
    let offset = scroll.get(id);
    let content = scroll.content_size(id);

    let text = match content {
        Some((_, h)) => format!("Scroll: {}/{}", offset.y, h),
        None => format!("Scroll: {}", offset.y),
    };

    Element::text(text)
        .style(Style::new().dim())
}
```

### Keyboard Scrolling

For keyboard-controlled scrolling:

```rust
Event::Key { target: Some(id), key, .. } if id == "my-list" => {
    match key {
        Key::Up => scroll.scroll_by("my-list", 0, -1),
        Key::Down => scroll.scroll_by("my-list", 0, 1),
        Key::PageUp => scroll.scroll_by("my-list", 0, -10),
        Key::PageDown => scroll.scroll_by("my-list", 0, 10),
        Key::Home => scroll.set("my-list", 0, 0),
        _ => {}
    }
}
```

## Controls

- **Mouse wheel**: Scroll content
- **Tab**: Navigate between scrollable areas
- **q/Escape**: Quit
