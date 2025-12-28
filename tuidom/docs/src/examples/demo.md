# Demo Application

The demo example showcases layout, styling, focus, and events.

## Running

```bash
cargo run --example demo
```

## Features Demonstrated

- Vertical and horizontal layouts (`col`, `row`)
- Header/content/footer structure
- Sidebar navigation with focus highlighting
- Clickable buttons
- Margin and padding
- Cross-axis alignment (`Align::Start`, `Center`, `End`)
- Flex grow ratios
- Wrap behavior
- Z-index overlapping
- Event handling and display

## Key Patterns

### Application Structure

```rust
fn main() -> std::io::Result<()> {
    let mut term = Terminal::new()?;
    let mut focus = FocusState::new();
    let mut last_event: Option<String> = None;

    loop {
        let root = ui(focus.focused(), last_event.as_deref());
        term.render(&root)?;

        let raw_events = term.poll(None)?;
        let events = focus.process_events(&raw_events, &root, term.layout());

        for event in events {
            match &event {
                Event::Key { key: Key::Char('q'), .. }
                | Event::Key { key: Key::Escape, .. } => return Ok(()),
                // Handle other events...
            }
        }
    }
}
```

### UI as Functions

```rust
fn ui(focused: Option<&str>, last_event: Option<&str>) -> Element {
    Element::col()
        .width(Size::Fill)
        .height(Size::Fill)
        .child(header())
        .child(content(focused))
        .child(footer(last_event))
}
```

### Focus-Aware Components

```rust
fn nav_item(id: &str, label: &str, focused: Option<&str>) -> Element {
    let is_focused = focused == Some(id);
    let prefix = if is_focused { ">" } else { " " };
    let bg = if is_focused {
        Color::oklch(0.35, 0.08, 250.0)
    } else {
        Color::oklch(0.25, 0.02, 250.0)
    };

    Element::text(format!("{} {}", prefix, label))
        .id(id)
        .focusable(true)
        .clickable(true)
        .width(Size::Fill)
        .style(Style::new().background(bg))
}
```

### Z-Index Layering

```rust
Element::box_()
    .child(
        Element::box_()
            .position(Position::Absolute)
            .z_index(0)  // Background layer
            .style(Style::new().background(Color::oklch(0.4, 0.15, 25.0)))
    )
    .child(
        Element::box_()
            .position(Position::Absolute)
            .z_index(1)  // Middle layer
            .style(Style::new().background(Color::oklch(0.45, 0.15, 140.0)))
    )
    .child(
        Element::box_()
            .position(Position::Absolute)
            .z_index(2)  // Foreground layer
            .style(Style::new().background(Color::oklch(0.4, 0.15, 250.0)))
    )
```

## Controls

- **Tab**: Navigate between focusable elements
- **Shift+Tab**: Navigate backwards
- **Mouse hover**: Focus follows mouse
- **Click**: Interact with clickable elements
- **q/Escape**: Quit
