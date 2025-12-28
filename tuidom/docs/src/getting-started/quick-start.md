# Quick Start

This guide will help you create your first tuidom application in under 5 minutes.

## Installation

Add tuidom to your `Cargo.toml`:

```toml
[dependencies]
tuidom = "0.1"
```

## Minimal Example

Here's the simplest possible tuidom application:

```rust
use tuidom::{Element, Size, Style, Color, Terminal, Event, Key};

fn main() -> std::io::Result<()> {
    // Initialize the terminal
    let mut term = Terminal::new()?;

    loop {
        // Build the UI
        let ui = Element::col()
            .width(Size::Fill)
            .height(Size::Fill)
            .style(Style::new().background(Color::oklch(0.2, 0.02, 250.0)))
            .child(Element::text("Hello, tuidom!"));

        // Render to terminal
        term.render(&ui)?;

        // Poll for events
        let events = term.poll(None)?;

        // Handle quit
        for event in events {
            if let crossterm::event::Event::Key(key_event) = event {
                if key_event.code == crossterm::event::KeyCode::Char('q') {
                    return Ok(());
                }
            }
        }
    }
}
```

When you run this with `cargo run`, you'll see a dark blue background with "Hello, tuidom!" text. Press 'q' to exit.

## Understanding the Basics

### Terminal

`Terminal::new()` initializes the terminal in raw mode with:
- Alternate screen buffer (preserves your shell)
- Hidden cursor
- Mouse capture enabled

When the `Terminal` is dropped, it automatically restores the terminal to its original state.

### Elements

Elements are the building blocks of tuidom UIs. The main types are:

- `Element::text("...")` - Display text content
- `Element::col()` - Vertical flex container (like CSS `flex-direction: column`)
- `Element::row()` - Horizontal flex container (like CSS `flex-direction: row`)
- `Element::box_()` - Generic container

### The Builder Pattern

Elements use a fluent builder API. Chain methods to configure properties:

```rust
Element::text("Click me!")
    .id("my-button")           // Unique identifier
    .width(Size::Fixed(20))    // Fixed width of 20 cells
    .height(Size::Fixed(3))    // Fixed height of 3 rows
    .focusable(true)           // Can receive focus
    .clickable(true)           // Responds to clicks
    .style(Style::new()
        .background(Color::oklch(0.4, 0.1, 140.0))
        .bold())
```

### Colors

tuidom uses OKLCH colors for perceptually uniform results:

```rust
Color::oklch(lightness, chroma, hue)
// lightness: 0.0 (black) to 1.0 (white)
// chroma: 0.0 (gray) to ~0.4 (saturated)
// hue: 0-360 degrees (red=25, yellow=90, green=140, blue=250)
```

RGB is also supported: `Color::rgb(255, 128, 0)`

### The Render Loop

A tuidom application follows this pattern:

```rust
loop {
    // 1. Build UI from current state
    let ui = build_ui(&state);

    // 2. Render to terminal
    term.render(&ui)?;

    // 3. Wait for events
    let events = term.poll(None)?;

    // 4. Update state based on events
    for event in events {
        handle_event(&mut state, event);
    }
}
```

## Next Steps

- [Your First TUI App](./first-app.md) - Build an interactive application with focus and events
- [Understanding the Render Loop](./render-loop.md) - Deep dive into the event loop pattern
