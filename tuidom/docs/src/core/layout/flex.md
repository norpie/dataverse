# Flex Layout

tuidom uses a flexbox-style layout system for arranging children within containers.

## Direction

The `direction` property controls the main axis:

```rust
Element::box_()
    .direction(Direction::Row)    // Children laid out left-to-right
    .direction(Direction::Column) // Children laid out top-to-bottom
```

Shortcuts:
- `Element::row()` = `Element::box_().direction(Direction::Row)`
- `Element::col()` = `Element::box_().direction(Direction::Column)`

## Gap

The `gap` property adds spacing between children:

```rust
Element::col()
    .gap(1)  // 1 row between each child
    .child(Element::text("A"))
    .child(Element::text("B"))
    .child(Element::text("C"))
```

Result:
```
A

B

C
```

Gap does not add space before the first child or after the last child.

## Justify (Main Axis Alignment)

`justify` controls how children are distributed along the main axis.

### `Justify::Start` (default)

Pack children at the start:

```rust
Element::row().justify(Justify::Start)
```
```
[A][B][C]              |
```

### `Justify::Center`

Center children:

```rust
Element::row().justify(Justify::Center)
```
```
|      [A][B][C]       |
```

### `Justify::End`

Pack children at the end:

```rust
Element::row().justify(Justify::End)
```
```
|              [A][B][C]
```

### `Justify::SpaceBetween`

Distribute children with equal space between:

```rust
Element::row().justify(Justify::SpaceBetween)
```
```
[A]       [B]       [C]
```

### `Justify::SpaceAround`

Distribute with equal space around each child:

```rust
Element::row().justify(Justify::SpaceAround)
```
```
   [A]     [B]     [C]
```

## Align (Cross Axis Alignment)

`align` controls how children are positioned along the cross axis.

### `Align::Start` (default)

Align to the start of the cross axis:

```rust
Element::row()
    .height(Size::Fixed(5))
    .align(Align::Start)
```
```
┌───────────────────┐
│[A][B][C]          │
│                   │
│                   │
│                   │
└───────────────────┘
```

### `Align::Center`

Center on the cross axis:

```rust
Element::row()
    .height(Size::Fixed(5))
    .align(Align::Center)
```
```
┌───────────────────┐
│                   │
│[A][B][C]          │
│                   │
└───────────────────┘
```

### `Align::End`

Align to the end of the cross axis:

```rust
Element::row()
    .height(Size::Fixed(5))
    .align(Align::End)
```
```
┌───────────────────┐
│                   │
│                   │
│                   │
│[A][B][C]          │
└───────────────────┘
```

### `Align::Stretch`

Stretch children to fill the cross axis:

```rust
Element::row()
    .height(Size::Fixed(5))
    .align(Align::Stretch)
    .child(Element::text("A"))
```
```
┌───────────────────┐
│AAAAAAAAAAAAAAAAAAA│
│AAAAAAAAAAAAAAAAAAA│
│AAAAAAAAAAAAAAAAAAA│
│AAAAAAAAAAAAAAAAAAA│
│AAAAAAAAAAAAAAAAAAA│
└───────────────────┘
```

## Wrap

By default, children stay on a single line. Use `wrap` to allow wrapping:

```rust
Element::row()
    .width(Size::Fixed(20))
    .wrap(Wrap::Wrap)
    .child(Element::text("One").width(Size::Fixed(8)))
    .child(Element::text("Two").width(Size::Fixed(8)))
    .child(Element::text("Three").width(Size::Fixed(8)))
```
```
[One    ][Two    ]
[Three  ]
```

### `Wrap::NoWrap` (default)

Children overflow if they don't fit.

### `Wrap::Wrap`

Children wrap to the next line.

## Flex Grow and Shrink

### `flex_grow`

Controls how extra space is distributed:

```rust
Element::row()
    .child(Element::text("A").flex_grow(1))  // Grows to fill space
    .child(Element::text("B").flex_grow(0))  // Fixed size
```

### `flex_shrink`

Controls how children shrink when space is limited:

```rust
Element::row()
    .child(Element::text("Important").flex_shrink(0))  // Never shrinks
    .child(Element::text("Optional").flex_shrink(1))   // Shrinks first
```

Default `flex_shrink` is 1.

## Align Self

Override the parent's `align` for a specific child:

```rust
Element::col()
    .align(Align::Start)
    .child(Element::text("Normal"))
    .child(Element::text("Centered").align_self(Align::Center))
    .child(Element::text("Normal"))
```

## Common Patterns

### Centered Container

```rust
Element::col()
    .width(Size::Fill)
    .height(Size::Fill)
    .justify(Justify::Center)
    .align(Align::Center)
    .child(content())
```

### Toolbar with Spacer

```rust
Element::row()
    .width(Size::Fill)
    .child(Element::text("Left"))
    .child(Element::box_().width(Size::Fill))  // Spacer
    .child(Element::text("Right"))
```

### Equal Width Columns

```rust
Element::row()
    .width(Size::Fill)
    .child(col1().width(Size::Fill))
    .child(col2().width(Size::Fill))
    .child(col3().width(Size::Fill))
```
