# Positioning

The `position` property controls how an element is placed relative to its parent and siblings.

## Position Modes

### `Position::Static` (default)

Normal flow layout. The element participates in flex layout with its siblings.

```rust
Element::box_()
    .position(Position::Static)  // Default
```

The `top`, `left`, `right`, `bottom` properties have no effect on static elements.

### `Position::Relative`

Positioned relative to its normal position. The element still participates in flow layout, but is visually offset.

```rust
Element::text("Shifted")
    .position(Position::Relative)
    .left(2)   // Shift 2 cells right from normal position
    .top(-1)   // Shift 1 row up from normal position
```

The offset doesn't affect sibling layout—siblings behave as if the element were in its original position.

### `Position::Absolute`

Removed from normal flow and positioned relative to the parent's content area.

```rust
Element::box_()
    .position(Position::Absolute)
    .left(10)
    .top(5)
    .width(Size::Fixed(20))
    .height(Size::Fixed(10))
```

Absolute elements:
- Don't affect sibling layout
- Are positioned relative to the parent's top-left corner
- Can overlap other elements

## Position Offsets

### `left` and `top`

Position from the parent's top-left corner:

```rust
Element::box_()
    .position(Position::Absolute)
    .left(10)  // 10 cells from left edge
    .top(5)    // 5 rows from top edge
```

### `right` and `bottom`

Position from the parent's bottom-right corner:

```rust
Element::box_()
    .position(Position::Absolute)
    .right(0)   // Align to right edge
    .bottom(0)  // Align to bottom edge
```

### Combining Offsets

Use opposing offsets for stretching:

```rust
Element::box_()
    .position(Position::Absolute)
    .left(2)
    .right(2)   // Stretch to fill width minus 4 cells
    .top(1)
    .bottom(1)  // Stretch to fill height minus 2 rows
```

## Z-Index

The `z_index` property controls stacking order for overlapping elements:

```rust
// Background layer
Element::box_()
    .position(Position::Absolute)
    .z_index(0)

// Foreground layer
Element::box_()
    .position(Position::Absolute)
    .z_index(1)  // Renders on top
```

Rules:
- Higher z_index renders on top
- Elements with equal z_index render in document order (later = on top)
- Default z_index is 0
- Negative values are allowed

## Common Patterns

### Overlay

```rust
Element::box_()
    .width(Size::Fill)
    .height(Size::Fill)
    .child(main_content())
    .child(
        Element::box_()
            .position(Position::Absolute)
            .left(0)
            .top(0)
            .right(0)
            .bottom(0)
            .z_index(10)
            .style(Style::new().background(Color::oklch(0.0, 0.0, 0.0).alpha(0.5)))
            .child(modal_content())
    )
```

### Corner Badge

```rust
Element::box_()
    .width(Size::Fixed(40))
    .height(Size::Fixed(10))
    .child(card_content())
    .child(
        Element::text("NEW")
            .position(Position::Absolute)
            .right(1)
            .top(0)
            .style(Style::new().background(Color::oklch(0.5, 0.2, 25.0)))
    )
```

### Floating Action Button

```rust
Element::box_()
    .width(Size::Fill)
    .height(Size::Fill)
    .child(main_content())
    .child(
        Element::text("[+]")
            .position(Position::Absolute)
            .right(2)
            .bottom(2)
            .focusable(true)
            .clickable(true)
    )
```

### Tooltip

```rust
fn tooltip_wrapper(content: Element, tooltip: &str, show: bool) -> Element {
    let mut wrapper = Element::box_()
        .child(content);

    if show {
        wrapper = wrapper.child(
            Element::text(tooltip)
                .position(Position::Absolute)
                .left(0)
                .top(-1)  // Above the element
                .z_index(100)
                .style(Style::new().background(Color::oklch(0.3, 0.0, 0.0)))
        );
    }

    wrapper
}
```

## Layout Considerations

- Absolute elements don't contribute to parent sizing with `Size::Auto`
- Absolute elements respect parent's padding
- Use `min_width`/`min_height` on parents if they only contain absolute children
