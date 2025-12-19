# Styling

Rafter uses a flexbox-inspired layout system with inline styling.

## Basic Styling

Styles are optional and use parentheses:

```rust
view! {
    // No style
    column {
        text { "Hello" }
    }
    
    // With style
    column (padding: 1, gap: 1) {
        text (bold, color: primary) { "Styled" }
    }
}
```

## Layout

### Flex Direction

Layout elements (`column`, `row`) set the flex direction:

```rust
column { }  // flex-direction: column
row { }     // flex-direction: row
```

### Justify Content (Main Axis)

```rust
column (justify: start) { }         // Default
column (justify: center) { }
column (justify: end) { }
column (justify: space_between) { }
column (justify: space_around) { }
```

### Align Items (Cross Axis)

```rust
row (align: start) { }
row (align: center) { }
row (align: end) { }
row (align: stretch) { }  // Default
```

### Flex Grow/Shrink

```rust
row {
    text { "Fixed" }
    text (flex: 1) { "Takes remaining space" }
    text { "Fixed" }
}

row {
    text (flex: 1) { "Equal" }
    text (flex: 1) { "Equal" }
    text (flex: 2) { "Double" }
}
```

## Sizing

```rust
// Fixed size (cells)
column (width: 50, height: 10) { }

// Percentage of parent
column (width: 50%, height: 100%) { }

// Min/max constraints
column (min_width: 20, max_width: 80) { }
column (min_height: 5, max_height: 20) { }
```

## Spacing

### Padding (Inner)

```rust
column (padding: 1) { }                   // All sides
column (padding_h: 2, padding_v: 1) { }   // Horizontal, vertical
column (padding_left: 1, padding_right: 2) { }
column (padding_top: 1, padding_bottom: 2) { }
```

### Margin (Outer)

```rust
column (margin: 1) { }
column (margin_h: 2, margin_v: 1) { }
```

### Gap (Between Children)

```rust
column (gap: 1) { }   // Space between each child
row (gap: 2) { }
```

## Borders

```rust
// Border styles
column (border: single) { }
column (border: double) { }
column (border: rounded) { }
column (border: thick) { }

// Border color
column (border: single, border_color: primary) { }
```

## Colors

### Theme Colors

Use theme-defined color names (compile-time checked):

```rust
text (color: primary) { }
text (color: secondary) { }
text (color: text_muted) { }
text (bg: surface) { }
column (border_color: error) { }
```

### Literal Colors

```rust
// OKLCH (recommended for perceptual uniformity)
text (color: oklch(0.7, 0.15, 200)) { }

// HSL
text (color: hsl(200, 50%, 70%)) { }

// Hex
text (color: "#3498db") { }
```

## Text Styling

```rust
text (bold) { }
text (italic) { }
text (underline) { }
text (dim) { }
text (bold, italic, underline) { }
```

## Conditional Styling

```rust
row (bg: if selected { surface } else { background }) {
    text (color: if active { primary } else { text_muted }) {
        record.name
    }
}
```

## Theming

### Defining a Theme

```rust
#[theme]
struct MyTheme {
    primary: Color,
    secondary: Color,
    background: Color,
    surface: Color,
    text: Color,
    text_muted: Color,
    error: Color,
    success: Color,
}

impl MyTheme {
    fn dark() -> Self {
        Self {
            primary: Color::oklch(0.6, 0.15, 250),
            secondary: Color::oklch(0.7, 0.1, 200),
            background: Color::oklch(0.15, 0.02, 250),
            surface: Color::oklch(0.2, 0.02, 250),
            text: Color::oklch(0.9, 0.02, 250),
            text_muted: Color::oklch(0.6, 0.02, 250),
            error: Color::oklch(0.6, 0.2, 25),
            success: Color::oklch(0.6, 0.15, 145),
        }
    }
    
    fn light() -> Self {
        Self {
            primary: Color::oklch(0.5, 0.15, 250),
            background: Color::oklch(0.98, 0.01, 250),
            // ...
        }
    }
}
```

### Theme Macro Features

The `#[theme]` macro generates:

- **Serialization**: `to_value()`, `from_value()`
- **Introspection**: `fields()`, `get()`, `set()`

```rust
// Introspection for dynamic UIs
for field in theme.fields() {
    println!("{}: {:?}", field.name, field.kind);
}

// Runtime modification
theme.set("primary", Color::oklch(0.5, 0.2, 300));

// Serialize to whatever format you want
let value = theme.to_value();
let toml = your_serializer(value);
```

### Grouped Themes

```rust
#[theme]
struct MyTheme {
    #[group]
    colors: ThemeColors,
    #[group]
    spacing: ThemeSpacing,
}

#[theme_group]
struct ThemeColors {
    primary: Color,
    secondary: Color,
}

#[theme_group]
struct ThemeSpacing {
    padding: u16,
    gap: u16,
}
```

### Runtime Theme Switching

```rust
#[handler]
fn toggle_theme(&mut self, cx: AppContext) {
    if cx.theme().is_dark() {
        cx.set_theme(MyTheme::light());
    } else {
        cx.set_theme(MyTheme::dark());
    }
}
```

### Compile-Time Validation

Color names are checked at compile time:

```rust
text (color: primary) { }    // OK
text (color: primry) { }     // Compile error: unknown theme color 'primry'
text (color: tertiary) { }   // Compile error: unknown theme color 'tertiary'
```

Literal colors bypass the check:

```rust
text (color: oklch(0.5, 0.1, 200)) { }  // Always valid
```
