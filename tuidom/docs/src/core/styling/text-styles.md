# Text Styles

Text styling controls formatting like bold, italic, and underline.

## Using Text Styles

### Via Style Builder

The most common way to apply text styles:

```rust
Element::text("Bold text")
    .style(Style::new().bold())

Element::text("Bold and italic")
    .style(Style::new().bold().italic())
```

### Via TextStyle Struct

For more control, use `TextStyle` directly:

```rust
let my_style = TextStyle::new()
    .bold()
    .underline();

Element::text("Styled")
    .style(Style::new().text_style(my_style))
```

## Available Styles

### `.bold()`

Makes text bold/bright:

```rust
Style::new().bold()
```

Use for headers, emphasis, and important labels.

### `.italic()`

Makes text italic (terminal support varies):

```rust
Style::new().italic()
```

Use for emphasis, quotes, or variable names.

### `.underline()`

Underlines text:

```rust
Style::new().underline()
```

Use for links or special emphasis.

### `.dim()`

Makes text dimmer/fainter:

```rust
Style::new().dim()
```

Use for secondary information, hints, or disabled states.

### `.strikethrough()`

Strikes through text (via `TextStyle`):

```rust
TextStyle::new().strikethrough()
```

Use for completed tasks or deprecated items.

## Combining Styles

Multiple text styles can be combined:

```rust
Element::text("Important!")
    .style(Style::new()
        .bold()
        .underline()
        .foreground(Color::oklch(0.6, 0.2, 25.0)))
```

## TextStyle Struct

The `TextStyle` struct holds all text formatting flags:

```rust
pub struct TextStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub dim: bool,
    pub strikethrough: bool,
}
```

Create with the builder pattern:

```rust
let style = TextStyle::new()
    .bold()
    .italic();
```

Or set fields directly:

```rust
let mut style = TextStyle::new();
style.bold = true;
style.underline = true;
```

## Terminal Compatibility

Not all terminals support all text styles:

| Style | Support |
|-------|---------|
| Bold | Universal |
| Dim | Common |
| Italic | Varies |
| Underline | Universal |
| Strikethrough | Limited |

When unsupported, the text renders normally.

## Common Patterns

### Header Text

```rust
fn header(text: &str) -> Element {
    Element::text(text)
        .style(Style::new()
            .foreground(Color::oklch(0.9, 0.05, 250.0))
            .bold())
}
```

### Hint Text

```rust
fn hint(text: &str) -> Element {
    Element::text(text)
        .style(Style::new()
            .foreground(Color::oklch(0.5, 0.01, 0.0))
            .dim())
}
```

### Link Text

```rust
fn link(text: &str) -> Element {
    Element::text(text)
        .style(Style::new()
            .foreground(Color::oklch(0.6, 0.15, 250.0))
            .underline())
}
```

### Error Text

```rust
fn error(text: &str) -> Element {
    Element::text(text)
        .style(Style::new()
            .foreground(Color::oklch(0.6, 0.2, 25.0))
            .bold())
}
```

### Disabled Text

```rust
fn disabled(text: &str) -> Element {
    Element::text(text)
        .style(Style::new()
            .foreground(Color::oklch(0.4, 0.01, 0.0))
            .dim())
}
```
