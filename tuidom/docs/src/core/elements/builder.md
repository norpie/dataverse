# Builder Pattern

Every `Element` method returns `Self`, enabling fluent method chaining. This page documents all available builder methods.

## Identity

### `.id(id)`

Sets a unique identifier for the element.

```rust
Element::text("Submit").id("submit-button")
```

IDs are used for:
- Event targeting (`target` field in events)
- Layout queries (`layout.get("id")`)
- Focus management
- Animation state tracking

## Layout Methods

### `.width(size)` / `.height(size)`

Set the element's size constraints.

```rust
Element::box_()
    .width(Size::Fixed(40))   // Exactly 40 cells
    .height(Size::Fill)       // Fill available height
```

See [Size Types](../layout/size.md) for all size variants.

### `.min_width(u16)` / `.max_width(u16)`

Set minimum and maximum width constraints.

```rust
Element::text("Flexible")
    .width(Size::Fill)
    .min_width(10)
    .max_width(50)
```

### `.min_height(u16)` / `.max_height(u16)`

Set minimum and maximum height constraints.

```rust
Element::col()
    .height(Size::Fill)
    .min_height(5)
    .max_height(20)
```

### `.padding(edges)`

Set inner spacing between the element's border and its content.

```rust
Element::box_()
    .padding(Edges::all(2))           // 2 cells on all sides
    .padding(Edges::symmetric(1, 2))  // 1 vertical, 2 horizontal
    .padding(Edges::new(1, 2, 1, 2))  // top, right, bottom, left
```

### `.margin(edges)`

Set outer spacing around the element.

```rust
Element::box_()
    .margin(Edges::horizontal(2))  // 2 cells left and right
```

## Positioning Methods

### `.position(pos)`

Set the positioning mode.

```rust
Element::box_()
    .position(Position::Absolute)
    .left(10)
    .top(5)
```

See [Positioning](../layout/positioning.md) for details.

### `.top(i16)` / `.left(i16)` / `.right(i16)` / `.bottom(i16)`

Set position offsets. Only meaningful for `Position::Relative` or `Position::Absolute`.

```rust
Element::box_()
    .position(Position::Absolute)
    .left(10)
    .top(5)
```

Negative values are allowed for relative positioning.

### `.z_index(i16)`

Set stacking order. Higher values render on top.

```rust
Element::box_().z_index(10)  // Renders above z_index(5)
```

Default is 0. Negative values are allowed.

## Flex Container Methods

### `.direction(dir)`

Set layout direction.

```rust
Element::box_()
    .direction(Direction::Row)    // Horizontal
    .direction(Direction::Column) // Vertical (default)
```

### `.gap(u16)`

Set spacing between children.

```rust
Element::col()
    .gap(1)
    .child(Element::text("A"))
    .child(Element::text("B"))
```

### `.justify(justify)`

Align children along the main axis.

```rust
Element::row()
    .justify(Justify::Start)        // Pack at start
    .justify(Justify::Center)       // Center
    .justify(Justify::End)          // Pack at end
    .justify(Justify::SpaceBetween) // Distribute evenly
    .justify(Justify::SpaceAround)  // Even space around each
```

### `.align(align)`

Align children along the cross axis.

```rust
Element::row()
    .height(Size::Fixed(5))
    .align(Align::Start)   // Top
    .align(Align::Center)  // Middle
    .align(Align::End)     // Bottom
    .align(Align::Stretch) // Fill height
```

### `.wrap(wrap)`

Control whether children wrap to multiple lines.

```rust
Element::row()
    .wrap(Wrap::NoWrap)  // Keep on single line (default)
    .wrap(Wrap::Wrap)    // Wrap to next line if needed
```

## Flex Item Methods

### `.flex_grow(u16)`

How much this element grows relative to siblings.

```rust
Element::row()
    .child(Element::text("A").width(Size::Flex(1)))  // 1/3 of space
    .child(Element::text("B").width(Size::Flex(2)))  // 2/3 of space
```

### `.flex_shrink(u16)`

How much this element shrinks when space is limited.

```rust
Element::box_().flex_shrink(0)  // Never shrink
```

Default is 1.

### `.align_self(align)`

Override parent's `align` for this specific child.

```rust
Element::col()
    .align(Align::Start)
    .child(Element::text("Normal"))
    .child(Element::text("Centered").align_self(Align::Center))
```

## Overflow Methods

### `.overflow(overflow)`

Control how content is handled when it exceeds the element's bounds.

```rust
Element::box_()
    .overflow(Overflow::Visible)  // Show overflow (default)
    .overflow(Overflow::Hidden)   // Clip overflow
    .overflow(Overflow::Scroll)   // Scrollable with scrollbar
    .overflow(Overflow::Auto)     // Scrollbar only when needed
```

### `.scroll_offset(x, y)`

Set the current scroll position.

```rust
Element::box_()
    .overflow(Overflow::Scroll)
    .scroll_offset(0, scroll_state.y)
```

## Visual Methods

### `.style(style)`

Set the element's visual appearance.

```rust
Element::text("Styled")
    .style(Style::new()
        .background(Color::oklch(0.3, 0.1, 250.0))
        .foreground(Color::oklch(0.9, 0.0, 0.0))
        .border(Border::Rounded)
        .bold())
```

### `.transitions(transitions)`

Configure animated transitions.

```rust
Element::box_()
    .style(Style::new().background(bg_color))
    .transitions(Transitions::new()
        .background(Duration::from_millis(300), Easing::EaseOut))
```

## Text Methods

### `.text_wrap(wrap)`

Control how text wraps.

```rust
Element::text("Long text...")
    .text_wrap(TextWrap::NoWrap)    // No wrapping
    .text_wrap(TextWrap::WordWrap)  // Wrap at word boundaries
    .text_wrap(TextWrap::CharWrap)  // Wrap at any character
    .text_wrap(TextWrap::Truncate)  // Cut off with ellipsis
```

### `.text_align(align)`

Set horizontal text alignment.

```rust
Element::text("Centered")
    .width(Size::Fill)
    .text_align(TextAlign::Left)   // Left-align (default)
    .text_align(TextAlign::Center) // Center
    .text_align(TextAlign::Right)  // Right-align
```

## Interaction Methods

### `.focusable(bool)`

Allow the element to receive keyboard focus.

```rust
Element::text("Button")
    .id("my-button")
    .focusable(true)
```

### `.clickable(bool)`

Allow the element to receive click events.

```rust
Element::text("Click me")
    .id("clickable")
    .clickable(true)
```

### `.draggable(bool)`

Mark the element as draggable (reserved for future use).

```rust
Element::box_().draggable(true)
```

## Child Methods

### `.child(element)`

Add a single child element.

```rust
Element::col()
    .child(Element::text("First"))
    .child(Element::text("Second"))
```

### `.children(iter)`

Add multiple children from an iterator.

```rust
let items = vec!["A", "B", "C"];
Element::col()
    .children(items.iter().map(|s| Element::text(*s)))
```
