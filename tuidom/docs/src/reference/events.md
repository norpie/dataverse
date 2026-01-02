# Event Reference

## Event Enum

```rust
pub enum Event {
    Key { target: Option<String>, key: Key, modifiers: Modifiers },
    Click { target: Option<String>, x: u16, y: u16, button: MouseButton },
    Scroll { target: Option<String>, x: u16, y: u16, delta_x: i16, delta_y: i16 },
    MouseMove { x: u16, y: u16 },
    Drag { target: Option<String>, x: u16, y: u16, button: MouseButton },
    Release { target: Option<String>, x: u16, y: u16, button: MouseButton },
    Focus { target: String },
    Blur { target: String },
    Resize { width: u16, height: u16 },
}
```

## Key Enum

```rust
pub enum Key {
    Char(char),
    Enter,
    Backspace,
    Delete,
    Tab,
    BackTab,
    Escape,
    Up, Down, Left, Right,
    Home, End,
    PageUp, PageDown,
    Insert,
    F(u8),
}
```

## Modifiers

```rust
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

Modifiers::new()    // No modifiers
Modifiers::shift()  // Shift only
Modifiers::ctrl()   // Ctrl only
Modifiers::alt()    // Alt only
modifiers.none()    // Check if no modifiers
```

## MouseButton

```rust
pub enum MouseButton {
    Left,
    Right,
    Middle,
}
```

## FocusState

```rust
FocusState::new()
focus.focused() -> Option<&str>
focus.focus(id) -> bool
focus.blur() -> bool
focus.focus_next(root) -> Option<String>
focus.focus_prev(root) -> Option<String>
focus.process_events(raw, root, layout) -> Vec<Event>
```

## ScrollState

```rust
ScrollState::new()
scroll.get(id) -> ScrollOffset
scroll.set(id, x, y)
scroll.scroll_by(id, dx, dy) -> bool
scroll.clamp(id, container, content_w, content_h)
scroll.content_size(id) -> Option<(u16, u16)>
scroll.process_events(events, root, layout) -> Vec<Event>
```

## ScrollOffset

```rust
pub struct ScrollOffset {
    pub x: u16,
    pub y: u16,
}

ScrollOffset::new(x, y)
```

## Utility Functions

```rust
collect_focusable(element) -> Vec<String>
collect_scrollable(element) -> Vec<String>
hit_test(layout, root, x, y) -> Option<String>
hit_test_any(layout, root, x, y) -> Option<String>
hit_test_focusable(layout, root, x, y) -> Option<String>
```
