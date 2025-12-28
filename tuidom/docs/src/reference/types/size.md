# Size Reference

The `Size` enum controls element dimensions.

## Variants

```rust
pub enum Size {
    Fixed(u16),
    Fill,
    Flex(u16),
    Auto,
    Percent(f32),
}
```

### `Fixed(u16)`

Exact size in terminal cells.

```rust
.width(Size::Fixed(40))  // Exactly 40 cells wide
```

### `Fill`

Expand to fill all available space. When multiple siblings use `Fill`, space is divided equally.

```rust
.width(Size::Fill)
```

### `Flex(u16)`

Proportional distribution based on flex factor.

```rust
// Two children: 1:2 ratio
.child(a.width(Size::Flex(1)))  // Gets 1/3
.child(b.width(Size::Flex(2)))  // Gets 2/3
```

### `Auto`

Size determined by content. For text, this is the text width/height. For containers, the minimum size to fit children.

```rust
.width(Size::Auto)  // Size to fit content
```

### `Percent(f32)`

Percentage of parent's content area.

```rust
.width(Size::Percent(50.0))  // Half of parent
```

## Default

`Size::Auto` is the default for both width and height.

## With Constraints

Combine with min/max for responsive behavior:

```rust
Element::box_()
    .width(Size::Fill)
    .min_width(20)
    .max_width(100)
```
