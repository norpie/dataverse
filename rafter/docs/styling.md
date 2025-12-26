# Styling

This guide covers theming and visual styling in rafter.

## Themes

Themes define the color palette for your application.

### Default Theme

Rafter includes a `DefaultTheme` with these colors:

| Name | Description |
|------|-------------|
| `primary` | Main accent color |
| `secondary` | Secondary accent |
| `background` | Page background |
| `surface` | Elevated surfaces (cards, modals) |
| `text` | Primary text color |
| `muted` | Subdued text (hints, labels) |
| `error` | Error states |
| `success` | Success states |
| `warning` | Warning states |
| `info` | Informational states |

### Using Theme Colors

Reference theme colors by name in `page!`:

```rust
page! {
    column (bg: background) {
        text (fg: primary, bold) { "Title" }
        text (fg: muted) { "Subtitle" }
        text (fg: error) { "Error message" }
    }
}
```

### Custom Themes

Create a custom theme by implementing the `Theme` trait:

```rust
use rafter::styling::color::Color;
use rafter::styling::theme::{DefaultTheme, Theme};

#[derive(Debug, Clone)]
struct MyTheme {
    inner: DefaultTheme,
}

impl MyTheme {
    fn new() -> Self {
        Self {
            inner: DefaultTheme {
                primary: Color::rgb(0, 200, 200),
                secondary: Color::rgb(100, 150, 255),
                background: Color::rgb(25, 25, 35),
                surface: Color::rgb(40, 40, 55),
                text: Color::rgb(230, 230, 240),
                text_muted: Color::rgb(140, 140, 160),
                error: Color::rgb(255, 100, 100),
                success: Color::rgb(100, 220, 100),
                warning: Color::rgb(255, 200, 50),
                info: Color::rgb(100, 180, 255),
                validation_error: Color::rgb(255, 100, 100),
                validation_error_border: Color::rgb(255, 100, 100),
            },
        }
    }
}

impl Theme for MyTheme {
    fn resolve(&self, name: &str) -> Option<Color> {
        self.inner.resolve(name)
    }

    fn color_names(&self) -> Vec<&'static str> {
        self.inner.color_names()
    }

    fn clone_box(&self) -> Box<dyn Theme> {
        Box::new(self.clone())
    }
}
```

### Applying a Theme

Pass the theme to the runtime:

```rust
#[tokio::main]
async fn main() {
    rafter::Runtime::new()
        .theme(MyTheme::new())
        .initial::<MyApp>()
        .run()
        .await
        .unwrap();
}
```

### Changing Theme at Runtime

```rust
#[handler]
async fn switch_to_dark(&self, cx: &AppContext) {
    cx.set_theme(DarkTheme::new());
}
```

## Colors

### RGB Colors

```rust
Color::rgb(255, 100, 100)  // Red, Green, Blue (0-255)
```

### OKLCH Colors

Perceptually uniform color space:

```rust
Color::oklch(0.7, 0.15, 150.0)  // Lightness, Chroma, Hue
```

### Named Colors

Reference theme colors by string name.

## Text Styling

Style text elements in `page!`:

```rust
page! {
    // Font weight
    text (bold) { "Bold text" }

    // Font style
    text (italic) { "Italic text" }

    // Decoration
    text (underline) { "Underlined" }

    // Colors
    text (fg: primary) { "Primary color" }
    text (bg: surface) { "With background" }

    // Combined
    text (bold, fg: error, underline) { "Error!" }
}
```

## Container Styling

Style containers with backgrounds and borders:

```rust
page! {
    // Background color
    column (bg: surface) { ... }

    // Border styles
    column (border: single) { ... }
    column (border: double) { ... }
    column (border: rounded) { ... }
    column (border: thick) { ... }

    // Combined
    column (bg: surface, border: rounded, padding: 1) {
        text { "Card content" }
    }
}
```

## Dynamic Styling

Use expressions for dynamic styles:

```rust
fn page(&self) -> Node {
    let is_error = self.has_error.get();
    let status_color = if is_error { "error" } else { "success" };

    page! {
        text (fg: {status_color}) { "Status" }
    }
}
```

## Transitions

Animate style changes with transitions:

```rust
fn page(&self) -> Node {
    let value = self.value.get();
    let bg = if value > 0 {
        Color::oklch(0.4, 0.1, 150.0)  // Greenish
    } else {
        Color::oklch(0.3, 0.1, 25.0)   // Reddish
    };

    page! {
        // id is required for transition tracking
        column (id: "status", bg: {bg}, transition: 300, easing: ease_out) {
            text { format!("Value: {}", value) }
        }
    }
}
```

### Transition Attributes

| Attribute | Description |
|-----------|-------------|
| `transition` | Duration in milliseconds |
| `easing` | Easing function |

### Easing Functions

- `linear` - Constant speed
- `ease_in` - Start slow, end fast
- `ease_out` - Start fast, end slow
- `ease_in_out` - Slow at both ends

## Focus Styling

Widgets automatically show focus indicators. The default theme highlights focused elements with the primary color.

## Widget-Specific Styling

Some widgets accept styling attributes:

```rust
page! {
    // Button styling is handled by the widget
    button(id: "primary", label: "Submit", on_click: submit)

    // List with custom appearance
    list(bind: self.items, height: fill)
}
```

## Next Steps

- [Modals](modals.md) - Styled dialog overlays
- [Widgets](widgets/overview.md) - Widget styling options
