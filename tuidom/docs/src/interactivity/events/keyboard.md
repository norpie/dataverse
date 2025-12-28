# Keyboard Events

Keyboard events are delivered via `Event::Key` with a `Key` enum and `Modifiers` struct.

## The Key Enum

```rust
pub enum Key {
    Char(char),      // Regular character
    Enter,
    Backspace,
    Delete,
    Tab,
    BackTab,         // Shift+Tab
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    F(u8),           // Function keys F1-F12
}
```

## Matching Keys

```rust
Event::Key { key, .. } => {
    match key {
        Key::Char('q') => quit(),
        Key::Char(c) => handle_char(c),
        Key::Enter => submit(),
        Key::Escape => cancel(),
        Key::Up => move_up(),
        Key::Down => move_down(),
        Key::F(1) => show_help(),
        _ => {}
    }
}
```

## Modifiers

The `Modifiers` struct tracks Shift, Ctrl, and Alt:

```rust
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}
```

### Checking Modifiers

```rust
Event::Key { key, modifiers, .. } => {
    match (key, modifiers.ctrl, modifiers.shift) {
        (Key::Char('s'), true, false) => save(),      // Ctrl+S
        (Key::Char('S'), true, true) => save_as(),    // Ctrl+Shift+S
        (Key::Char('c'), true, false) => copy(),      // Ctrl+C
        (Key::Char('v'), true, false) => paste(),     // Ctrl+V
        _ => {}
    }
}
```

### Modifier Helpers

```rust
// Check if no modifiers are pressed
if modifiers.none() {
    // Plain key press
}

// Create modifier combinations
Modifiers::shift()  // Shift only
Modifiers::ctrl()   // Ctrl only
Modifiers::alt()    // Alt only
Modifiers::new()    // No modifiers
```

## Tab Navigation

Tab and BackTab (Shift+Tab) are handled automatically by `FocusState::process_events`:

- **Tab**: Focus next element
- **BackTab**: Focus previous element

These generate `Focus` and `Blur` events but are not passed through as `Key` events.

## Common Patterns

### Quit on 'q' or Escape

```rust
Event::Key { key: Key::Char('q'), .. }
| Event::Key { key: Key::Escape, .. } => {
    return Ok(());
}
```

### Text Input

```rust
fn handle_input(key: Key, buffer: &mut String) {
    match key {
        Key::Char(c) => buffer.push(c),
        Key::Backspace => { buffer.pop(); }
        Key::Enter => submit_input(buffer),
        _ => {}
    }
}
```

### Arrow Key Navigation

```rust
match key {
    Key::Up => selected = selected.saturating_sub(1),
    Key::Down => selected = (selected + 1).min(items.len() - 1),
    Key::PageUp => selected = selected.saturating_sub(10),
    Key::PageDown => selected = (selected + 10).min(items.len() - 1),
    Key::Home => selected = 0,
    Key::End => selected = items.len() - 1,
    _ => {}
}
```

### Vim-style Navigation

```rust
match key {
    Key::Char('h') | Key::Left => move_left(),
    Key::Char('j') | Key::Down => move_down(),
    Key::Char('k') | Key::Up => move_up(),
    Key::Char('l') | Key::Right => move_right(),
    Key::Char('g') => go_to_start(),
    Key::Char('G') => go_to_end(),
    _ => {}
}
```

## Key Event Flow

1. User presses a key
2. `term.poll()` returns a crossterm `KeyEvent`
3. `focus.process_events()` converts it to `Event::Key`
4. `target` is set to the currently focused element
5. Your event handler receives the event

```rust
// Key goes to focused element
Element::text("Input")
    .id("my-input")
    .focusable(true)

// ...

Event::Key { target: Some(ref id), key, .. } if id == "my-input" => {
    // Handle input for this specific element
}
```
