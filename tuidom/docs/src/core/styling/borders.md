# Borders

Borders add visual boundaries around elements.

## Border Variants

### `Border::None` (default)

No border:

```rust
Element::box_()
    .style(Style::new().border(Border::None))
```

### `Border::Single`

Single-line box drawing characters:

```rust
Element::box_()
    .style(Style::new().border(Border::Single))
```
```
┌────────────┐
│  content   │
└────────────┘
```

### `Border::Double`

Double-line box drawing characters:

```rust
Element::box_()
    .style(Style::new().border(Border::Double))
```
```
╔════════════╗
║  content   ║
╚════════════╝
```

### `Border::Rounded`

Single-line with rounded corners:

```rust
Element::box_()
    .style(Style::new().border(Border::Rounded))
```
```
╭────────────╮
│  content   │
╰────────────╯
```

### `Border::Thick`

Thick/heavy box drawing characters:

```rust
Element::box_()
    .style(Style::new().border(Border::Thick))
```
```
┏━━━━━━━━━━━━┓
┃  content   ┃
┗━━━━━━━━━━━━┛
```

## Using Borders

Apply borders via the `Style` builder:

```rust
Element::box_()
    .style(Style::new()
        .border(Border::Rounded)
        .background(Color::oklch(0.2, 0.02, 250.0)))
    .padding(Edges::all(1))
    .child(Element::text("Card content"))
```

## Border and Layout

Borders consume space:
- Each border side takes 1 cell
- Total element width = content + padding + border (2 cells for left+right)

```rust
Element::box_()
    .width(Size::Fixed(20))  // Total width including border
    .style(Style::new().border(Border::Single))
    // Content area = 18 cells (20 - 2 for borders)
```

## Border Colors

Borders use the element's foreground color:

```rust
Element::box_()
    .style(Style::new()
        .border(Border::Single)
        .foreground(Color::oklch(0.6, 0.15, 250.0)))  // Blue border
```

## Common Patterns

### Card

```rust
fn card(content: Element) -> Element {
    Element::box_()
        .style(Style::new()
            .border(Border::Rounded)
            .background(Color::oklch(0.15, 0.01, 250.0)))
        .padding(Edges::all(1))
        .child(content)
}
```

### Panel with Header

```rust
fn panel(title: &str, content: Element) -> Element {
    Element::col()
        .style(Style::new().border(Border::Single))
        .child(
            Element::text(title)
                .width(Size::Fill)
                .style(Style::new()
                    .background(Color::oklch(0.3, 0.08, 250.0))
                    .bold())
        )
        .child(
            Element::box_()
                .padding(Edges::all(1))
                .child(content)
        )
}
```

### Focused vs Unfocused

```rust
fn focusable_box(is_focused: bool) -> Element {
    let border = if is_focused {
        Border::Double
    } else {
        Border::Single
    };

    let border_color = if is_focused {
        Color::oklch(0.6, 0.15, 140.0)  // Green
    } else {
        Color::oklch(0.4, 0.02, 0.0)    // Gray
    };

    Element::box_()
        .style(Style::new()
            .border(border)
            .foreground(border_color))
}
```

### Dialog Box

```rust
fn dialog(title: &str, content: Element) -> Element {
    Element::box_()
        .position(Position::Absolute)
        .left(10)
        .top(5)
        .width(Size::Fixed(40))
        .z_index(100)
        .style(Style::new()
            .border(Border::Double)
            .background(Color::oklch(0.2, 0.02, 250.0)))
        .padding(Edges::all(1))
        .child(
            Element::col()
                .child(Element::text(title).style(Style::new().bold()))
                .child(Element::text(""))
                .child(content)
        )
}
```

## Terminal Compatibility

All border styles use Unicode box drawing characters, which are widely supported in modern terminals. On terminals that don't support Unicode, characters may render as basic ASCII (e.g., `+`, `-`, `|`).
