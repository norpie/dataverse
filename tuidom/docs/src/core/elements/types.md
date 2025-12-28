# Element Types

tuidom provides four constructors for creating elements, each optimized for different use cases.

## Element::text()

Creates an element that displays text content.

```rust
Element::text("Hello, world!")
Element::text(format!("Count: {}", count))
```

Text elements:
- Default to `Size::Auto` (sized to fit content)
- Support text wrapping via `.text_wrap()`
- Support text alignment via `.text_align()`
- Can contain only text (no children)

```rust
Element::text("A long line of text that will wrap")
    .width(Size::Fixed(20))
    .text_wrap(TextWrap::WordWrap)
    .text_align(TextAlign::Center)
```

## Element::box_()

Creates a generic container element.

```rust
Element::box_()
    .width(Size::Fixed(30))
    .height(Size::Fixed(10))
    .child(Element::text("Inside the box"))
```

Box elements:
- Default to `Direction::Column`
- Default to `Size::Auto`
- Can contain children

Use `box_()` (with underscore) because `box` is a reserved keyword in Rust.

## Element::col()

Creates a vertical flex container (shorthand for `box_()` with `Direction::Column`).

```rust
Element::col()
    .gap(1)
    .child(Element::text("First"))
    .child(Element::text("Second"))
    .child(Element::text("Third"))
```

Children are laid out top-to-bottom:

```
┌────────────┐
│ First      │
│ Second     │
│ Third      │
└────────────┘
```

## Element::row()

Creates a horizontal flex container (shorthand for `box_()` with `Direction::Row`).

```rust
Element::row()
    .gap(2)
    .child(Element::text("Left"))
    .child(Element::text("Center"))
    .child(Element::text("Right"))
```

Children are laid out left-to-right:

```
┌─────────────────────────┐
│ Left   Center   Right   │
└─────────────────────────┘
```

## Element::custom()

Creates an element with custom rendering logic.

```rust
struct ProgressBar {
    progress: f32, // 0.0 to 1.0
}

impl CustomContent for ProgressBar {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let filled = (area.width as f32 * self.progress) as u16;
        for x in area.x..area.x + area.width {
            let char = if x < area.x + filled { '█' } else { '░' };
            if let Some(cell) = buf.get_mut(x, area.y) {
                cell.char = char;
            }
        }
    }

    fn intrinsic_size(&self) -> (u16, u16) {
        (20, 1) // Default size: 20 wide, 1 tall
    }
}

// Usage
Element::custom(ProgressBar { progress: 0.7 })
    .width(Size::Fixed(30))
```

See [Custom Content](./custom-content.md) for more details on implementing `CustomContent`.

## Content Enum

Internally, elements store their content as a `Content` enum:

```rust
pub enum Content {
    None,                        // Empty container
    Text(String),                // Text content
    Children(Vec<Element>),      // Child elements
    Custom(Box<dyn CustomContent>), // Custom rendering
}
```

When you call `.child()` on an element, it converts `Content::None` to `Content::Children` automatically.

## Choosing the Right Type

| Use Case | Type |
|----------|------|
| Display text | `Element::text()` |
| Vertical stack | `Element::col()` |
| Horizontal row | `Element::row()` |
| Custom direction/no children | `Element::box_()` |
| Special rendering | `Element::custom()` |
