# Layout System

tuidom uses a flexbox-inspired layout system. Elements are sized and positioned based on their constraints and the available space from their parent container.

## Core Concepts

### The Box Model

Every element has:
- **Content area**: Where children or text render
- **Padding**: Inner spacing between content and border
- **Border**: Visual border (if styled)
- **Margin**: Outer spacing around the element

```
┌─ margin ──────────────────────────────┐
│ ┌─ border ──────────────────────────┐ │
│ │ ┌─ padding ─────────────────────┐ │ │
│ │ │                               │ │ │
│ │ │         content area          │ │ │
│ │ │                               │ │ │
│ │ └───────────────────────────────┘ │ │
│ └───────────────────────────────────┘ │
└───────────────────────────────────────┘
```

### Flex Layout

Container elements (`col`, `row`, `box_`) lay out their children using flexbox rules:

1. **Main axis**: Direction of child placement (row = horizontal, column = vertical)
2. **Cross axis**: Perpendicular to main axis
3. **Sizing**: Children sized based on `Size` constraints
4. **Distribution**: Extra space distributed by `Justify` and `Align`

## Quick Reference

| Property | Description | Values |
|----------|-------------|--------|
| `width`/`height` | Size constraints | Fixed, Fill, Flex, Auto, Percent |
| `direction` | Main axis | Row, Column |
| `gap` | Space between children | u16 (cells) |
| `justify` | Main axis alignment | Start, Center, End, SpaceBetween, SpaceAround |
| `align` | Cross axis alignment | Start, Center, End, Stretch |
| `wrap` | Multi-line layout | NoWrap, Wrap |
| `position` | Positioning mode | Static, Relative, Absolute |
| `overflow` | Content overflow | Visible, Hidden, Scroll, Auto |

## Layout Examples

### Vertical Stack

```rust
Element::col()
    .gap(1)
    .child(Element::text("First"))
    .child(Element::text("Second"))
    .child(Element::text("Third"))
```

### Horizontal Row

```rust
Element::row()
    .gap(2)
    .child(Element::text("Left"))
    .child(Element::text("Right"))
```

### Split Layout

```rust
Element::row()
    .width(Size::Fill)
    .height(Size::Fill)
    .child(sidebar().width(Size::Fixed(30)))
    .child(main_content().width(Size::Fill))
```

### Centered Content

```rust
Element::col()
    .width(Size::Fill)
    .height(Size::Fill)
    .justify(Justify::Center)
    .align(Align::Center)
    .child(Element::text("Centered!"))
```

## Next Steps

- [Size Types](./layout/size.md) - Understanding Fixed, Fill, Flex, Auto, Percent
- [Flex Layout](./layout/flex.md) - Direction, justify, align, gap, wrap
- [Positioning](./layout/positioning.md) - Static, Relative, Absolute, z_index
- [Edges](./layout/edges.md) - Padding and margin
