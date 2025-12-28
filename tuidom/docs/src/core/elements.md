# Elements

Elements are the fundamental building blocks of tuidom UIs. Every visual component is an `Element` with properties that control its layout, appearance, and behavior.

## Creating Elements

tuidom provides several constructors for common element types:

```rust
// Text content
Element::text("Hello, world!")

// Containers
Element::box_()       // Generic container
Element::col()        // Vertical flex container (direction: Column)
Element::row()        // Horizontal flex container (direction: Row)

// Custom rendering
Element::custom(my_custom_content)
```

## The Builder Pattern

Elements use a fluent builder API. Chain methods to configure properties:

```rust
Element::text("Submit")
    .id("submit-btn")
    .width(Size::Fixed(20))
    .height(Size::Fixed(3))
    .focusable(true)
    .clickable(true)
    .style(Style::new()
        .background(Color::oklch(0.4, 0.12, 140.0))
        .bold())
```

Methods return `Self`, allowing unlimited chaining. Order doesn't matter—properties are applied to the final element.

## Element Properties

Elements have properties in several categories:

### Identity

| Method | Description |
|--------|-------------|
| `.id(id)` | Unique identifier for targeting events and layout queries |

### Content

| Method | Description |
|--------|-------------|
| `.child(element)` | Add a single child element |
| `.children(iter)` | Add multiple children from an iterator |

### Layout (Box Model)

| Method | Description |
|--------|-------------|
| `.width(size)` | Width constraint |
| `.height(size)` | Height constraint |
| `.min_width(u16)` | Minimum width in cells |
| `.max_width(u16)` | Maximum width in cells |
| `.min_height(u16)` | Minimum height in rows |
| `.max_height(u16)` | Maximum height in rows |
| `.padding(edges)` | Inner spacing |
| `.margin(edges)` | Outer spacing |

### Positioning

| Method | Description |
|--------|-------------|
| `.position(pos)` | Positioning mode: Static, Relative, Absolute |
| `.top(i16)` | Top offset (for positioned elements) |
| `.left(i16)` | Left offset |
| `.right(i16)` | Right offset |
| `.bottom(i16)` | Bottom offset |
| `.z_index(i16)` | Stacking order (higher = on top) |

### Flex Container

| Method | Description |
|--------|-------------|
| `.direction(dir)` | Row or Column layout |
| `.gap(u16)` | Space between children |
| `.justify(justify)` | Main axis alignment |
| `.align(align)` | Cross axis alignment |
| `.wrap(wrap)` | Whether items wrap to multiple lines |

### Flex Item

| Method | Description |
|--------|-------------|
| `.flex_grow(u16)` | How much to grow relative to siblings |
| `.flex_shrink(u16)` | How much to shrink when constrained |
| `.align_self(align)` | Override parent's align for this item |

### Overflow

| Method | Description |
|--------|-------------|
| `.overflow(overflow)` | Visible, Hidden, Scroll, or Auto |
| `.scroll_offset(x, y)` | Current scroll position |

### Visual

| Method | Description |
|--------|-------------|
| `.style(style)` | Background, foreground, border, text style |
| `.transitions(trans)` | Animation configuration |

### Text

| Method | Description |
|--------|-------------|
| `.text_wrap(wrap)` | NoWrap, WordWrap, CharWrap, or Truncate |
| `.text_align(align)` | Left, Center, or Right |

### Interaction

| Method | Description |
|--------|-------------|
| `.focusable(bool)` | Can receive keyboard focus |
| `.clickable(bool)` | Responds to mouse clicks |
| `.draggable(bool)` | Can be dragged (reserved for future use) |

## Adding Children

Containers hold child elements via `.child()` or `.children()`:

```rust
Element::col()
    .child(Element::text("First"))
    .child(Element::text("Second"))
    .child(Element::text("Third"))

// Or from an iterator
let items = vec!["A", "B", "C"];
Element::col()
    .children(items.iter().map(|s| Element::text(*s)))
```

## Element IDs

IDs are used for:
- Event targeting (`Event::Click { target: Some("my-id"), .. }`)
- Layout queries (`layout.get("my-id")`)
- Focus management
- Animation state tracking

```rust
Element::text("Clickable")
    .id("my-button")
    .clickable(true)
```

If no ID is set, elements get auto-generated IDs like `"el-42"` or `"text-17"`.

## Next Steps

- [Element Types](./elements/types.md) - Details on box_, text, col, row, custom
- [Builder Pattern](./elements/builder.md) - Complete builder method reference
- [Custom Content](./elements/custom-content.md) - Implementing the CustomContent trait
