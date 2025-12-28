# Overflow Reference

The `Overflow` enum controls content clipping and scrolling.

```rust
pub enum Overflow {
    Visible, // Content extends beyond bounds - default
    Hidden,  // Content is clipped
    Scroll,  // Always show scrollbar
    Auto,    // Scrollbar only when content overflows
}
```

## Visible

Content renders beyond element bounds. Parent overflow settings still apply.

```rust
.overflow(Overflow::Visible)
```

## Hidden

Content is clipped at element boundaries. No scrolling.

```rust
.overflow(Overflow::Hidden)
```

## Scroll

Content is clipped. Scrollbar is always visible. Use `.scroll_offset(x, y)` to control position.

```rust
.overflow(Overflow::Scroll)
.scroll_offset(scroll_state.x, scroll_state.y)
```

## Auto

Content is clipped. Scrollbar appears only when content exceeds viewport.

```rust
.overflow(Overflow::Auto)
```

## With ScrollState

For interactive scrolling:

```rust
let offset = scroll.get("my-list");
Element::col()
    .id("my-list")
    .overflow(Overflow::Scroll)
    .scroll_offset(offset.x, offset.y)
    .children(...)
```

## Default

`Overflow::Visible` is the default.
