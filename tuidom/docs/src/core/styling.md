# Styling

The `Style` struct controls the visual appearance of elements: colors, borders, and text formatting.

## The Style Builder

Create and configure styles with the fluent builder pattern:

```rust
Element::text("Styled text")
    .style(Style::new()
        .background(Color::oklch(0.3, 0.1, 250.0))
        .foreground(Color::oklch(0.9, 0.02, 0.0))
        .border(Border::Rounded)
        .bold())
```

## Style Properties

| Method | Description |
|--------|-------------|
| `.background(color)` | Background color |
| `.foreground(color)` | Text/foreground color |
| `.border(border)` | Border style |
| `.text_style(style)` | Full text style struct |
| `.bold()` | Enable bold text |
| `.italic()` | Enable italic text |
| `.underline()` | Enable underline |
| `.dim()` | Enable dim/faint text |

## Quick Examples

### Header Style

```rust
Style::new()
    .background(Color::oklch(0.3, 0.1, 250.0))
    .bold()
```

### Error Style

```rust
Style::new()
    .foreground(Color::oklch(0.6, 0.2, 25.0))
    .bold()
```

### Card Style

```rust
Style::new()
    .background(Color::oklch(0.2, 0.02, 250.0))
    .border(Border::Rounded)
```

### Subtle Text

```rust
Style::new()
    .foreground(Color::oklch(0.5, 0.01, 0.0))
    .dim()
```

## Style Inheritance

Styles don't inherit from parents by default. Each element must specify its own style:

```rust
Element::col()
    .style(Style::new().foreground(Color::oklch(0.9, 0.0, 0.0)))  // White text
    .child(Element::text("Inherits nothing"))  // Uses default colors
```

To share styles, create style constants or helper functions:

```rust
fn text_style() -> Style {
    Style::new().foreground(Color::oklch(0.9, 0.02, 0.0))
}

Element::col()
    .child(Element::text("One").style(text_style()))
    .child(Element::text("Two").style(text_style()))
```

## Default Values

`Style::default()` has:
- `background`: None (transparent)
- `foreground`: None (terminal default)
- `border`: `Border::None`
- `text_style`: All flags false (normal text)

## Combining Styles

Styles are built incrementally. Start with a base and add properties:

```rust
let base = Style::new()
    .background(Color::oklch(0.2, 0.02, 250.0));

let highlighted = base.clone()
    .background(Color::oklch(0.3, 0.08, 250.0))
    .bold();
```

## Next Steps

- [Colors](./styling/colors.md) - OKLCH color space and color operations
- [Text Styles](./styling/text-styles.md) - Bold, italic, underline, dim
- [Borders](./styling/borders.md) - Border variants and usage
