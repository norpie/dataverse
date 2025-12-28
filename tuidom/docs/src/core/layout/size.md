# Size Types

The `Size` enum controls how elements are sized along their width and height axes.

## Variants

### `Size::Fixed(u16)`

An exact size in terminal cells.

```rust
Element::box_()
    .width(Size::Fixed(40))   // Exactly 40 cells wide
    .height(Size::Fixed(10))  // Exactly 10 rows tall
```

Use `Fixed` when you need precise control over dimensions.

### `Size::Fill`

Expands to fill all available space.

```rust
Element::box_()
    .width(Size::Fill)   // Take all horizontal space
    .height(Size::Fill)  // Take all vertical space
```

When multiple siblings use `Fill`, space is divided equally:

```rust
Element::row()
    .child(Element::text("A").width(Size::Fill))  // 50%
    .child(Element::text("B").width(Size::Fill))  // 50%
```

### `Size::Flex(u16)`

Distributes space proportionally based on flex factors.

```rust
Element::row()
    .child(Element::text("A").width(Size::Flex(1)))  // 1/3 of space
    .child(Element::text("B").width(Size::Flex(2)))  // 2/3 of space
```

The flex factor is relative to siblings:
- `Flex(1)` + `Flex(1)` = 50% each
- `Flex(1)` + `Flex(2)` = 33% + 67%
- `Flex(1)` + `Flex(3)` = 25% + 75%

### `Size::Auto`

Size determined by content.

```rust
Element::text("Hello")
    .width(Size::Auto)   // Width = text length
    .height(Size::Auto)  // Height = 1 (single line)
```

For containers, `Auto` means the minimum size needed to fit children:

```rust
Element::col()
    .width(Size::Auto)  // Width = widest child
    .child(Element::text("Short"))
    .child(Element::text("Longer text"))
```

### `Size::Percent(f32)`

A percentage of the available space.

```rust
Element::box_()
    .width(Size::Percent(50.0))   // 50% of parent width
    .height(Size::Percent(25.0))  // 25% of parent height
```

Percentages are calculated based on the parent's content area (after padding).

## Default Values

- `Element::text()`: `Size::Auto` for both axes
- `Element::box_()`, `col()`, `row()`: `Size::Auto` for both axes

## Combining with Constraints

Use `min_*` and `max_*` to constrain sizes:

```rust
Element::text("Flexible text")
    .width(Size::Fill)
    .min_width(20)   // At least 20 cells
    .max_width(100)  // At most 100 cells
```

This is useful for responsive layouts:

```rust
Element::box_()
    .width(Size::Fill)
    .max_width(80)  // Cap width for readability
```

## Examples

### Header-Content-Footer Layout

```rust
Element::col()
    .height(Size::Fill)
    .child(header().height(Size::Fixed(3)))     // Fixed header
    .child(content().height(Size::Fill))        // Flexible content
    .child(footer().height(Size::Fixed(1)))     // Fixed footer
```

### Sidebar Layout

```rust
Element::row()
    .width(Size::Fill)
    .child(sidebar().width(Size::Fixed(30)))    // Fixed sidebar
    .child(main().width(Size::Fill))            // Flexible main area
```

### Proportional Columns

```rust
Element::row()
    .width(Size::Fill)
    .child(col1().width(Size::Flex(1)))  // 25%
    .child(col2().width(Size::Flex(2)))  // 50%
    .child(col3().width(Size::Flex(1)))  // 25%
```

### Responsive Content

```rust
Element::box_()
    .width(Size::Percent(80.0))
    .max_width(120)
    .min_width(40)
```
