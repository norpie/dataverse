# Terminal API Reference

## Terminal

```rust
Terminal::new() -> io::Result<Terminal>
```

### Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `size()` | `(u16, u16)` | Terminal dimensions |
| `poll(timeout)` | `Vec<CrosstermEvent>` | Wait for input |
| `render(&root)` | `&LayoutResult` | Render element tree |
| `layout()` | `&LayoutResult` | Get last layout |
| `set_reduced_motion(bool)` | `()` | Enable/disable animations |
| `has_active_transitions()` | `bool` | Check if animating |

## LayoutResult

```rust
layout.get(id) -> Option<&Rect>
layout.content_size(id) -> Option<(u16, u16)>
layout.viewport_size(id) -> Option<(u16, u16)>
```

## Rect

```rust
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

Rect::new(x, y, width, height)
Rect::from_size(width, height)

rect.left()   -> u16
rect.right()  -> u16
rect.top()    -> u16
rect.bottom() -> u16
rect.area()   -> u32
rect.is_empty() -> bool
rect.contains(x, y) -> bool
rect.shrink(top, right, bottom, left) -> Rect
```

## Buffer

```rust
Buffer::new(width, height)

buf.width()  -> u16
buf.height() -> u16
buf.get(x, y) -> Option<&Cell>
buf.get_mut(x, y) -> Option<&mut Cell>
buf.set(x, y, cell)
buf.clear()
buf.diff(&other) -> impl Iterator<Item = (u16, u16, &Cell)>
```

## Cell

```rust
pub struct Cell {
    pub char: char,
    pub fg: Rgb,
    pub bg: Rgb,
    pub style: TextStyle,
    pub wide_continuation: bool,
}

Cell::new(char)
cell.with_fg(rgb)
cell.with_bg(rgb)
cell.with_style(text_style)
```

## Transitions

```rust
Transitions::new()

// Individual properties
.left(duration, easing)
.top(duration, easing)
.right(duration, easing)
.bottom(duration, easing)
.width(duration, easing)
.height(duration, easing)
.background(duration, easing)
.foreground(duration, easing)

// Groups
.position(duration, easing)
.size(duration, easing)
.colors(duration, easing)
.all(duration, easing)

.has_any() -> bool
```

## Easing

```rust
pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

easing.apply(progress) -> f32
```
