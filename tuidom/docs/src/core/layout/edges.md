# Edges (Padding & Margin)

The `Edges` struct is used for both `padding` and `margin` properties. It defines spacing on all four sides of an element.

## Creating Edges

### `Edges::new(top, right, bottom, left)`

Explicit values for each side (CSS order):

```rust
Element::box_()
    .padding(Edges::new(1, 2, 1, 2))  // top=1, right=2, bottom=1, left=2
```

### `Edges::all(value)`

Same value for all sides:

```rust
Element::box_()
    .padding(Edges::all(2))  // 2 cells on all sides
```

### `Edges::symmetric(vertical, horizontal)`

Different values for vertical and horizontal:

```rust
Element::box_()
    .padding(Edges::symmetric(1, 2))  // 1 top/bottom, 2 left/right
```

### `Edges::horizontal(value)` / `Edges::vertical(value)`

Only horizontal or vertical spacing:

```rust
Element::box_()
    .padding(Edges::horizontal(2))  // 2 left, 2 right, 0 top/bottom
    .margin(Edges::vertical(1))     // 1 top, 1 bottom, 0 left/right
```

### `Edges::top(value)` / `Edges::right(value)` / `Edges::bottom(value)` / `Edges::left(value)`

Single side only:

```rust
Element::box_()
    .padding(Edges::top(1))
    .margin(Edges::left(2))
```

## Padding vs Margin

### Padding

Space **inside** the element's border, between the border and content.

```rust
Element::box_()
    .padding(Edges::all(2))
    .style(Style::new().border(Border::Single))
    .child(Element::text("Content"))
```
```
┌────────────────┐
│                │
│    Content     │
│                │
└────────────────┘
```

Padding affects:
- Where children are positioned
- The content area available to children
- Background color fills the padding area

### Margin

Space **outside** the element's border, between this element and its siblings.

```rust
Element::col()
    .child(Element::text("A"))
    .child(Element::text("B").margin(Edges::vertical(1)))
    .child(Element::text("C"))
```
```
A

B

C
```

Margin affects:
- Spacing from sibling elements
- Spacing from parent padding

## Properties

Access individual values:

```rust
let edges = Edges::new(1, 2, 3, 4);
edges.top     // 1
edges.right   // 2
edges.bottom  // 3
edges.left    // 4
```

Calculate totals:

```rust
let edges = Edges::new(1, 2, 1, 2);
edges.horizontal_total()  // 4 (left + right)
edges.vertical_total()    // 2 (top + bottom)
```

## Common Patterns

### Card Layout

```rust
Element::box_()
    .padding(Edges::all(1))
    .style(Style::new().border(Border::Rounded))
    .child(Element::text("Card content"))
```

### Section Separator

```rust
Element::box_()
    .margin(Edges::vertical(1))
    .child(Element::text("─".repeat(40)))
```

### Indented List

```rust
fn list_item(text: &str, indent: u16) -> Element {
    Element::text(text)
        .padding(Edges::left(indent * 2))
}
```

### Button Padding

```rust
Element::text("  Submit  ")  // Inline padding via spaces
// or
Element::text("Submit")
    .padding(Edges::symmetric(0, 2))
```

## Default Values

`Edges::default()` returns all zeros:

```rust
Edges {
    top: 0,
    right: 0,
    bottom: 0,
    left: 0,
}
```

## Margin Collapse

Unlike CSS, tuidom does **not** collapse margins. Adjacent margins are additive:

```rust
Element::col()
    .child(Element::text("A").margin(Edges::bottom(2)))
    .child(Element::text("B").margin(Edges::top(2)))
// Total space between A and B is 4 rows
```
