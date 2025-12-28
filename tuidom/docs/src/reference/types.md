# Types Reference

Overview of tuidom's core types.

## Layout Types

### [Size](./types/size.md)

Element sizing constraints.

| Variant | Description |
|---------|-------------|
| `Fixed(u16)` | Exact size in cells |
| `Fill` | Fill available space |
| `Flex(u16)` | Proportional distribution |
| `Auto` | Size to content |
| `Percent(f32)` | Percentage of parent |

### [Layout Enums](./types/layout-enums.md)

| Type | Variants |
|------|----------|
| `Direction` | `Row`, `Column` |
| `Position` | `Static`, `Relative`, `Absolute` |
| `Justify` | `Start`, `Center`, `End`, `SpaceBetween`, `SpaceAround` |
| `Align` | `Start`, `Center`, `End`, `Stretch` |
| `Wrap` | `NoWrap`, `Wrap` |

### [Overflow](./types/overflow.md)

| Variant | Description |
|---------|-------------|
| `Visible` | Content extends beyond bounds |
| `Hidden` | Content is clipped |
| `Scroll` | Always show scrollbar |
| `Auto` | Scrollbar when needed |

### Edges

Padding and margin values.

```rust
Edges::new(top, right, bottom, left)
Edges::all(value)
Edges::symmetric(vertical, horizontal)
Edges::horizontal(value)
Edges::vertical(value)
Edges::top(value)
Edges::right(value)
Edges::bottom(value)
Edges::left(value)
```

### Rect

Rectangle for layout results.

```rust
Rect::new(x, y, width, height)
Rect::from_size(width, height)
rect.left(), rect.right(), rect.top(), rect.bottom()
rect.area()
rect.is_empty()
rect.contains(x, y)
rect.shrink(top, right, bottom, left)
```

## Styling Types

### Style

Visual appearance builder.

```rust
Style::new()
    .background(color)
    .foreground(color)
    .border(border)
    .text_style(style)
    .bold()
    .italic()
    .underline()
    .dim()
```

### Border

| Variant | Description |
|---------|-------------|
| `None` | No border |
| `Single` | Single line: `┌─┐` |
| `Double` | Double line: `╔═╗` |
| `Rounded` | Rounded corners: `╭─╮` |
| `Thick` | Thick line: `┏━┓` |

### TextStyle

Text formatting flags.

```rust
TextStyle::new()
    .bold()
    .italic()
    .underline()
    .dim()
    .strikethrough()
```

## Text Types

### TextWrap

| Variant | Description |
|---------|-------------|
| `NoWrap` | No wrapping |
| `WordWrap` | Wrap at words |
| `CharWrap` | Wrap at characters |
| `Truncate` | Cut with ellipsis |

### TextAlign

| Variant | Description |
|---------|-------------|
| `Left` | Left-align |
| `Center` | Center |
| `Right` | Right-align |
