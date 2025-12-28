# Color Reference

## Constructors

| Constructor | Description |
|-------------|-------------|
| `Color::oklch(l, c, h)` | OKLCH color (l: 0-1, c: 0-0.4, h: 0-360) |
| `Color::oklcha(l, c, h, a)` | OKLCH with alpha (a: 0-1) |
| `Color::rgb(r, g, b)` | RGB color (0-255 each) |
| `Color::var(name)` | Named color variable |

## Operations

All operations return a new `Color`:

| Operation | Description |
|-----------|-------------|
| `.lighten(amount)` | Increase lightness (0-1) |
| `.darken(amount)` | Decrease lightness (0-1) |
| `.saturate(amount)` | Increase chroma (0-1) |
| `.desaturate(amount)` | Decrease chroma (0-1) |
| `.hue_shift(degrees)` | Rotate hue (0-360) |
| `.alpha(value)` | Set transparency (0-1) |
| `.mix(other, amount)` | Blend with another color |

## Chaining

Operations can be chained:

```rust
Color::oklch(0.5, 0.15, 250.0)
    .lighten(0.1)
    .saturate(0.05)
    .alpha(0.9)
```

## Conversion

| Method | Description |
|--------|-------------|
| `.to_rgb()` | Convert to `Rgb` struct |
| `.to_dsl()` | Convert to DSL string |

## Rgb Struct

```rust
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

Rgb::new(255, 128, 0)
```

## OKLCH Hue Values

| Hue | Color |
|-----|-------|
| 25 | Red |
| 60 | Orange |
| 90 | Yellow |
| 140 | Green |
| 190 | Cyan |
| 250 | Blue |
| 300 | Purple |
| 330 | Pink |
