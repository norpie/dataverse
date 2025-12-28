# Your First TUI App

Let's build an interactive menu application with keyboard navigation and focus management.

## The Goal

We'll create a simple app with:
- A header
- A list of menu items that can be navigated with Tab
- Visual feedback for the focused item
- Keyboard handling for selection

## Complete Example

```rust
use tuidom::{
    Color, Element, Event, FocusState, Key, Size, Style, Terminal,
};

fn main() -> std::io::Result<()> {
    let mut term = Terminal::new()?;
    let mut focus = FocusState::new();
    let mut selected: Option<String> = None;

    loop {
        // Build UI with current state
        let ui = build_ui(focus.focused(), selected.as_deref());
        term.render(&ui)?;

        // Poll and process events
        let raw_events = term.poll(None)?;
        let events = focus.process_events(&raw_events, &ui, term.layout());

        // Handle events
        for event in events {
            match event {
                Event::Key { key: Key::Char('q'), .. }
                | Event::Key { key: Key::Escape, .. } => {
                    return Ok(());
                }
                Event::Key { key: Key::Enter, target, .. } => {
                    if let Some(id) = target {
                        selected = Some(id);
                    }
                }
                Event::Click { target, .. } => {
                    if let Some(id) = target {
                        selected = Some(id);
                    }
                }
                _ => {}
            }
        }
    }
}

fn build_ui(focused: Option<&str>, selected: Option<&str>) -> Element {
    Element::col()
        .width(Size::Fill)
        .height(Size::Fill)
        .style(Style::new().background(Color::oklch(0.15, 0.01, 250.0)))
        .child(header())
        .child(menu(focused))
        .child(status(selected))
}

fn header() -> Element {
    Element::text("My First App")
        .width(Size::Fill)
        .style(Style::new()
            .background(Color::oklch(0.3, 0.1, 250.0))
            .bold())
}

fn menu(focused: Option<&str>) -> Element {
    Element::col()
        .width(Size::Fill)
        .height(Size::Fill)
        .gap(1)
        .padding(tuidom::Edges::all(1))
        .child(Element::text("Select an option (Tab to navigate, Enter to select):"))
        .child(menu_item("option_1", "Option 1", focused))
        .child(menu_item("option_2", "Option 2", focused))
        .child(menu_item("option_3", "Option 3", focused))
}

fn menu_item(id: &str, label: &str, focused: Option<&str>) -> Element {
    let is_focused = focused == Some(id);

    let (bg, prefix) = if is_focused {
        (Color::oklch(0.4, 0.12, 140.0), "> ")
    } else {
        (Color::oklch(0.25, 0.02, 250.0), "  ")
    };

    Element::text(format!("{}{}", prefix, label))
        .id(id)
        .focusable(true)
        .clickable(true)
        .width(Size::Fixed(30))
        .style(Style::new().background(bg))
}

fn status(selected: Option<&str>) -> Element {
    let text = match selected {
        Some(id) => format!("Selected: {}", id),
        None => "Nothing selected".to_string(),
    };

    Element::text(text)
        .width(Size::Fill)
        .style(Style::new().background(Color::oklch(0.2, 0.02, 250.0)))
}
```

## Key Concepts

### FocusState

`FocusState` tracks which element has keyboard focus:

```rust
let mut focus = FocusState::new();

// In the event loop:
let events = focus.process_events(&raw_events, &ui, term.layout());
```

The `process_events` method:
1. Converts raw crossterm events to tuidom `Event`s
2. Handles Tab/Shift+Tab for focus navigation
3. Routes keyboard events to the focused element
4. Emits `Focus` and `Blur` events on focus changes

### Focusable Elements

Mark elements as focusable with `.focusable(true)`:

```rust
Element::text("Menu Item")
    .id("item_1")        // Required for focus tracking
    .focusable(true)     // Participates in Tab order
```

Focus order follows document order (the order elements appear in the tree).

### Events

After `process_events`, you get high-level events:

```rust
Event::Key { target, key, modifiers }  // Keyboard input
Event::Click { target, x, y, button }  // Mouse click
Event::Focus { target }                 // Element gained focus
Event::Blur { target }                  // Element lost focus
```

The `target` is the `id` of the element that received the event.

### Clickable Elements

For mouse interaction, mark elements as clickable:

```rust
Element::text("Button")
    .id("my_button")
    .clickable(true)    // Receives click events
```

### Dynamic Styling

Use the focused state to change appearance:

```rust
fn menu_item(id: &str, label: &str, focused: Option<&str>) -> Element {
    let is_focused = focused == Some(id);

    let bg = if is_focused {
        Color::oklch(0.4, 0.12, 140.0)  // Bright green when focused
    } else {
        Color::oklch(0.25, 0.02, 250.0) // Dim gray otherwise
    };

    Element::text(label)
        .id(id)
        .focusable(true)
        .style(Style::new().background(bg))
}
```

## Next Steps

- [Understanding the Render Loop](./render-loop.md) - Learn about the event loop in detail
- [Focus Management](../interactivity/focus.md) - Deep dive into focus handling
- [Events](../interactivity/events.md) - Complete event reference
