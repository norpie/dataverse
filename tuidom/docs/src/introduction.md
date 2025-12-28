# Introduction

**tuidom** is a declarative TUI (Terminal User Interface) framework for Rust. It provides a component-based architecture inspired by modern web frameworks, bringing familiar concepts like flexbox layout, CSS-like styling, and smooth animations to terminal applications.

## Key Features

- **Declarative UI**: Build interfaces by composing elements with a fluent builder API
- **Flexbox Layout**: Familiar CSS-like layout system with rows, columns, and flexible sizing
- **OKLCH Colors**: Perceptually uniform color space for smooth gradients and accessible palettes
- **Smooth Animations**: Property transitions with configurable easing functions
- **Event Handling**: Keyboard, mouse, focus, and scroll events with element targeting
- **Differential Rendering**: Only redraws changed cells for optimal performance
- **Unicode Support**: Full support for wide characters and emoji

## Design Philosophy

tuidom follows several core principles:

1. **Immediate Mode Rendering**: The UI is rebuilt every frame from your application state. No widget state to manage.

2. **Functional Composition**: UIs are built by composing pure functions that return `Element` trees.

3. **Separation of Concerns**: Layout, styling, and behavior are configured independently on elements.

4. **Accessibility First**: Focus management, reduced motion support, and keyboard navigation built-in.

## Quick Example

```rust
use tuidom::{Element, Size, Style, Color, Terminal};

fn main() -> std::io::Result<()> {
    let mut term = Terminal::new()?;

    loop {
        let ui = Element::col()
            .width(Size::Fill)
            .height(Size::Fill)
            .style(Style::new().background(Color::oklch(0.2, 0.02, 250.0)))
            .child(Element::text("Hello, tuidom!"));

        term.render(&ui)?;

        // Handle events...
        let events = term.poll(None)?;
        // Break on 'q' press, etc.
    }
}
```

## Getting Help

- [GitHub Repository](https://github.com/norpie/tuidom)
- [API Documentation](https://docs.rs/tuidom)

## Next Steps

Start with the [Quick Start](./getting-started/quick-start.md) guide to build your first tuidom application.
