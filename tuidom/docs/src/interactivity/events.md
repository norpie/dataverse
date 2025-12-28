# Events

tuidom provides high-level events that are targeted to specific elements.

## The Event Enum

```rust
pub enum Event {
    Key { target: Option<String>, key: Key, modifiers: Modifiers },
    Click { target: Option<String>, x: u16, y: u16, button: MouseButton },
    Scroll { target: Option<String>, x: u16, y: u16, delta_x: i16, delta_y: i16 },
    MouseMove { x: u16, y: u16 },
    Focus { target: String },
    Blur { target: String },
    Resize { width: u16, height: u16 },
}
```

## Processing Events

Use `FocusState::process_events` to convert raw crossterm events to high-level events:

```rust
let mut focus = FocusState::new();

loop {
    let ui = build_ui();
    let layout = term.render(&ui)?;

    let raw_events = term.poll(None)?;
    let events = focus.process_events(&raw_events, &ui, &layout);

    for event in events {
        match event {
            Event::Key { target, key, modifiers } => { /* ... */ }
            Event::Click { target, .. } => { /* ... */ }
            // ...
        }
    }
}
```

## Event Targeting

### `target` Field

For `Key`, `Click`, and `Scroll` events, `target` is the ID of the element that should receive the event:

- **Key events**: Target is the currently focused element
- **Click events**: Target is the clicked element (if clickable)
- **Scroll events**: Target is the element under the mouse

```rust
Event::Click { target: Some(id), .. } => {
    if id == "submit-btn" {
        submit_form();
    }
}
```

### `None` Targets

`target` is `None` when:
- No element is focused (for Key events)
- The click/scroll is on empty space or non-interactive elements

## Event Types

### `Event::Key`

Keyboard input, targeted at the focused element:

```rust
Event::Key { target, key, modifiers } => {
    match key {
        Key::Enter => handle_enter(target),
        Key::Char('d') if modifiers.ctrl => handle_delete(),
        _ => {}
    }
}
```

### `Event::Click`

Mouse click on an element:

```rust
Event::Click { target, x, y, button } => {
    if button == MouseButton::Left {
        if let Some(id) = target {
            handle_click(&id);
        }
    }
}
```

### `Event::Scroll`

Mouse scroll wheel:

```rust
Event::Scroll { target, delta_y, .. } => {
    if let Some(id) = target {
        scroll_element(&id, delta_y);
    }
}
```

`delta_y`: Negative = scroll up, Positive = scroll down

### `Event::MouseMove`

Mouse movement (for hover tracking):

```rust
Event::MouseMove { x, y } => {
    hovered_position = (x, y);
}
```

### `Event::Focus` / `Event::Blur`

Focus changes:

```rust
Event::Focus { target } => {
    log::info!("Focused: {}", target);
}
Event::Blur { target } => {
    log::info!("Blurred: {}", target);
}
```

### `Event::Resize`

Terminal resize:

```rust
Event::Resize { width, height } => {
    // UI will be re-laid out on next render
}
```

## Next Steps

- [Keyboard Events](./events/keyboard.md) - Key enum and modifiers
- [Mouse Events](./events/mouse.md) - Click, scroll, and mouse buttons
- [Focus Management](./focus.md) - Handling focus and Tab navigation
