# Colors

tuidom uses the OKLCH color space by default, which provides perceptually uniform colors. RGB is also supported.

## Creating Colors

### OKLCH Colors

```rust
Color::oklch(lightness, chroma, hue)
```

- **Lightness** (0.0 - 1.0): 0 = black, 1 = white
- **Chroma** (0.0 - ~0.4): 0 = gray, higher = more saturated
- **Hue** (0 - 360): Color wheel position

```rust
Color::oklch(0.5, 0.15, 250.0)  // Medium blue
Color::oklch(0.3, 0.1, 140.0)   // Dark green
Color::oklch(0.7, 0.2, 25.0)    // Bright red-orange
```

### Common Hue Values

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

### OKLCH with Alpha

```rust
Color::oklcha(0.5, 0.15, 250.0, 0.5)  // 50% transparent blue
```

### RGB Colors

```rust
Color::rgb(255, 128, 0)  // Orange
Color::rgb(0, 0, 0)      // Black
Color::rgb(255, 255, 255) // White
```

### Color Variables

Reference named colors (for theming):

```rust
Color::var("primary")
Color::var("error")
```

## Color Operations

Chain operations to modify colors:

### `lighten(amount)` / `darken(amount)`

Adjust lightness. Amount is 0.0 to 1.0.

```rust
let base = Color::oklch(0.5, 0.15, 250.0);
let lighter = base.clone().lighten(0.2);  // L becomes ~0.7
let darker = base.clone().darken(0.2);    // L becomes ~0.3
```

### `saturate(amount)` / `desaturate(amount)`

Adjust chroma (saturation):

```rust
let vibrant = base.clone().saturate(0.1);    // More colorful
let muted = base.clone().desaturate(0.05);   // More gray
```

### `hue_shift(degrees)`

Rotate the hue:

```rust
let complementary = base.clone().hue_shift(180.0);  // Opposite color
let analogous = base.clone().hue_shift(30.0);       // Adjacent color
```

### `alpha(value)`

Set transparency (0.0 = transparent, 1.0 = opaque):

```rust
let semi_transparent = base.clone().alpha(0.5);
```

### `mix(other, amount)`

Blend with another color:

```rust
let blended = base.clone().mix(Color::oklch(0.9, 0.0, 0.0), 0.5);  // 50% mix with white
```

## Chaining Operations

Operations can be chained:

```rust
Color::oklch(0.5, 0.15, 250.0)
    .lighten(0.1)
    .saturate(0.05)
    .alpha(0.9)
```

## Color Palettes

### Creating a Color Scale

```rust
fn blue_scale(level: u8) -> Color {
    let l = 0.1 + (level as f32 * 0.08);  // 0.1 to 0.9
    Color::oklch(l, 0.12, 250.0)
}

// blue_scale(1) = dark blue
// blue_scale(5) = medium blue
// blue_scale(9) = light blue
```

### Semantic Colors

```rust
mod colors {
    use tuidom::Color;

    pub fn primary() -> Color {
        Color::oklch(0.5, 0.15, 250.0)
    }

    pub fn success() -> Color {
        Color::oklch(0.55, 0.15, 140.0)
    }

    pub fn warning() -> Color {
        Color::oklch(0.65, 0.18, 60.0)
    }

    pub fn error() -> Color {
        Color::oklch(0.55, 0.2, 25.0)
    }

    pub fn text() -> Color {
        Color::oklch(0.9, 0.02, 250.0)
    }

    pub fn text_dim() -> Color {
        Color::oklch(0.6, 0.01, 250.0)
    }

    pub fn surface() -> Color {
        Color::oklch(0.2, 0.02, 250.0)
    }
}
```

## Converting to RGB

Get the RGB representation:

```rust
let color = Color::oklch(0.5, 0.15, 250.0);
let rgb = color.to_rgb();  // Rgb { r, g, b }
```

## DSL Representation

Get a string representation (useful for debugging):

```rust
let color = Color::oklch(0.5, 0.15, 250.0).lighten(0.1);
let dsl = color.to_dsl();  // "oklch(0.5, 0.15, 250) | lighten(0.1)"
```

## Why OKLCH?

OKLCH has advantages over RGB:

1. **Perceptually uniform**: Equal steps in L/C/H look equally different to humans
2. **Predictable lightness**: L=0.5 always looks medium brightness
3. **Easy adjustments**: Lightening/darkening is intuitive
4. **Smooth transitions**: Animations between OKLCH colors look natural

Compare:
```rust
// RGB: hard to predict how this looks
Color::rgb(128, 64, 192)

// OKLCH: clearly a medium-dark, moderately saturated purple
Color::oklch(0.4, 0.15, 300.0)
```
