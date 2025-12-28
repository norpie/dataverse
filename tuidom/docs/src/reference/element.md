# Element API

Complete reference for the `Element` struct.

## Constructors

| Constructor | Description |
|-------------|-------------|
| `Element::box_()` | Generic container |
| `Element::text(content)` | Text element |
| `Element::col()` | Vertical flex container |
| `Element::row()` | Horizontal flex container |
| `Element::custom(content)` | Custom content element |

## Builder Methods

### Identity

| Method | Type | Description |
|--------|------|-------------|
| `.id(id)` | `impl Into<String>` | Element identifier |

### Layout - Box Model

| Method | Type | Default | Description |
|--------|------|---------|-------------|
| `.width(size)` | `Size` | `Auto` | Width constraint |
| `.height(size)` | `Size` | `Auto` | Height constraint |
| `.min_width(v)` | `u16` | None | Minimum width |
| `.max_width(v)` | `u16` | None | Maximum width |
| `.min_height(v)` | `u16` | None | Minimum height |
| `.max_height(v)` | `u16` | None | Maximum height |
| `.padding(edges)` | `Edges` | `0,0,0,0` | Inner spacing |
| `.margin(edges)` | `Edges` | `0,0,0,0` | Outer spacing |

### Positioning

| Method | Type | Default | Description |
|--------|------|---------|-------------|
| `.position(pos)` | `Position` | `Static` | Positioning mode |
| `.top(v)` | `i16` | None | Top offset |
| `.left(v)` | `i16` | None | Left offset |
| `.right(v)` | `i16` | None | Right offset |
| `.bottom(v)` | `i16` | None | Bottom offset |
| `.z_index(v)` | `i16` | `0` | Stacking order |

### Flex Container

| Method | Type | Default | Description |
|--------|------|---------|-------------|
| `.direction(dir)` | `Direction` | `Column` | Main axis |
| `.gap(v)` | `u16` | `0` | Space between children |
| `.justify(j)` | `Justify` | `Start` | Main axis alignment |
| `.align(a)` | `Align` | `Start` | Cross axis alignment |
| `.wrap(w)` | `Wrap` | `NoWrap` | Multi-line wrapping |

### Flex Item

| Method | Type | Default | Description |
|--------|------|---------|-------------|
| `.flex_grow(v)` | `u16` | `0` | Growth factor |
| `.flex_shrink(v)` | `u16` | `1` | Shrink factor |
| `.align_self(a)` | `Align` | None | Override parent align |

### Overflow

| Method | Type | Default | Description |
|--------|------|---------|-------------|
| `.overflow(o)` | `Overflow` | `Visible` | Overflow handling |
| `.scroll_offset(x, y)` | `u16, u16` | `0, 0` | Scroll position |

### Visual

| Method | Type | Default | Description |
|--------|------|---------|-------------|
| `.style(s)` | `Style` | Default | Visual style |
| `.transitions(t)` | `Transitions` | None | Animation config |

### Text

| Method | Type | Default | Description |
|--------|------|---------|-------------|
| `.text_wrap(w)` | `TextWrap` | `NoWrap` | Text wrapping |
| `.text_align(a)` | `TextAlign` | `Left` | Text alignment |

### Interaction

| Method | Type | Default | Description |
|--------|------|---------|-------------|
| `.focusable(b)` | `bool` | `false` | Can receive focus |
| `.clickable(b)` | `bool` | `false` | Responds to clicks |
| `.draggable(b)` | `bool` | `false` | Can be dragged |

### Children

| Method | Type | Description |
|--------|------|-------------|
| `.child(element)` | `Element` | Add single child |
| `.children(iter)` | `impl IntoIterator<Item = Element>` | Add multiple children |

## Fields

Direct field access (all `pub`):

```rust
pub struct Element {
    pub id: String,
    pub content: Content,
    pub width: Size,
    pub height: Size,
    pub min_width: Option<u16>,
    pub max_width: Option<u16>,
    pub min_height: Option<u16>,
    pub max_height: Option<u16>,
    pub padding: Edges,
    pub margin: Edges,
    pub position: Position,
    pub top: Option<i16>,
    pub left: Option<i16>,
    pub right: Option<i16>,
    pub bottom: Option<i16>,
    pub z_index: i16,
    pub direction: Direction,
    pub gap: u16,
    pub justify: Justify,
    pub align: Align,
    pub wrap: Wrap,
    pub flex_grow: u16,
    pub flex_shrink: u16,
    pub align_self: Option<Align>,
    pub overflow: Overflow,
    pub scroll_offset: (u16, u16),
    pub style: Style,
    pub transitions: Transitions,
    pub text_wrap: TextWrap,
    pub text_align: TextAlign,
    pub focusable: bool,
    pub clickable: bool,
    pub draggable: bool,
}
```
