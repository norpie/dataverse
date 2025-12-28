# Mouse Events

tuidom supports click, scroll, and mouse movement events.

## Click Events

### Event::Click

Triggered when the mouse button is pressed:

```rust
Event::Click { target, x, y, button }
```

- `target`: ID of the clicked element (if clickable), or `None`
- `x`, `y`: Mouse position in terminal cells
- `button`: Which mouse button was pressed

### MouseButton Enum

```rust
pub enum MouseButton {
    Left,
    Right,
    Middle,
}
```

### Handling Clicks

```rust
Event::Click { target, button, .. } => {
    if button == MouseButton::Left {
        if let Some(id) = target {
            match id.as_str() {
                "submit" => submit_form(),
                "cancel" => cancel_form(),
                _ => {}
            }
        }
    }
}
```

### Making Elements Clickable

Mark elements as clickable to receive click events:

```rust
Element::text("Click me!")
    .id("my-button")
    .clickable(true)
```

Only elements with `.clickable(true)` will have their ID in the `target` field.

## Scroll Events

### Event::Scroll

Triggered by the mouse scroll wheel:

```rust
Event::Scroll { target, x, y, delta_x, delta_y }
```

- `target`: ID of the element under the mouse
- `x`, `y`: Mouse position
- `delta_x`: Horizontal scroll (-1 left, +1 right)
- `delta_y`: Vertical scroll (-1 up, +1 down)

### Handling Scroll

```rust
Event::Scroll { target, delta_y, .. } => {
    if let Some(id) = target {
        if id == "my-list" {
            scroll_offset = (scroll_offset as i16 + delta_y).max(0) as u16;
        }
    }
}
```

## Mouse Move Events

### Event::MouseMove

Triggered when the mouse moves:

```rust
Event::MouseMove { x, y }
```

Note: `MouseMove` doesn't have a `target` field. Use hit testing if you need to know what's under the mouse.

### Focus Follows Mouse

By default, `FocusState` implements "focus follows mouse"—hovering over a focusable element focuses it:

```rust
// This happens automatically in process_events:
// - Hover over focusable element → Focus event
// - Move away → Blur event
```

To disable this behavior, handle mouse events before calling `process_events` or implement your own event processing.

## Common Patterns

### Button with Visual Feedback

```rust
fn button(id: &str, label: &str, focused: bool, pressed: bool) -> Element {
    let bg = if pressed {
        Color::oklch(0.55, 0.15, 140.0)  // Bright when pressed
    } else if focused {
        Color::oklch(0.45, 0.12, 250.0)  // Medium when focused
    } else {
        Color::oklch(0.3, 0.06, 250.0)   // Dim otherwise
    };

    Element::text(label)
        .id(id)
        .focusable(true)
        .clickable(true)
        .style(Style::new().background(bg))
}
```

### Context Menu on Right-Click

```rust
Event::Click { target, button: MouseButton::Right, x, y } => {
    if let Some(id) = target {
        show_context_menu(id, x, y);
    }
}
```

### Double-Click Detection

tuidom doesn't have built-in double-click detection. Implement it with timing:

```rust
let mut last_click: Option<(String, Instant)> = None;

Event::Click { target: Some(id), button: MouseButton::Left, .. } => {
    let now = Instant::now();
    if let Some((last_id, last_time)) = &last_click {
        if last_id == &id && now.duration_since(*last_time) < Duration::from_millis(300) {
            handle_double_click(&id);
            last_click = None;
            continue;
        }
    }
    last_click = Some((id.clone(), now));
    handle_single_click(&id);
}
```

### Drag Detection

Basic drag tracking:

```rust
let mut drag_start: Option<(u16, u16)> = None;

Event::Click { x, y, button: MouseButton::Left, .. } => {
    drag_start = Some((x, y));
}

Event::MouseMove { x, y } => {
    if let Some((start_x, start_y)) = drag_start {
        let dx = x as i16 - start_x as i16;
        let dy = y as i16 - start_y as i16;
        handle_drag(dx, dy);
    }
}

// Detect mouse release (crossterm MouseEventKind::Up)
// Note: You may need to handle raw events for mouse release
```

## Click Targeting

Click targeting uses hit testing to find the element under the mouse:

1. Click occurs at (x, y)
2. Hit test finds the topmost clickable element at that position
3. Element's ID becomes the `target`

Only elements with `.clickable(true)` are considered for targeting.
