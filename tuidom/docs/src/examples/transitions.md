# Animated UI

The transitions example showcases smooth property animations.

## Running

```bash
cargo run --example transitions
```

## Features Demonstrated

- Color transitions on focus/click
- Position transitions (sliding elements)
- Different easing functions
- Reduced motion accessibility toggle

## Key Patterns

### Animated Button

```rust
fn button(id: &str, label: &str, easing: Easing, focused: bool, active: bool) -> Element {
    let bg = if active {
        Color::oklch(0.6, 0.15, 140.0)  // Bright when active
    } else if focused {
        Color::oklch(0.45, 0.12, 250.0) // Medium when focused
    } else {
        Color::oklch(0.3, 0.05, 250.0)  // Dim otherwise
    };

    Element::text(label)
        .id(id)
        .focusable(true)
        .clickable(true)
        .style(Style::new().background(bg).bold())
        .transitions(Transitions::new()
            .background(Duration::from_millis(300), easing))
}
```

### Sliding Element

```rust
fn sliding_box(is_open: bool) -> Element {
    let left = if is_open { 15 } else { 2 };
    let bg = if is_open {
        Color::oklch(0.55, 0.15, 140.0)
    } else {
        Color::oklch(0.4, 0.1, 200.0)
    };

    Element::box_()
        .position(Position::Absolute)
        .left(left)
        .top(2)
        .width(Size::Fixed(12))
        .height(Size::Fixed(3))
        .style(Style::new().background(bg).border(Border::Rounded))
        .transitions(Transitions::new()
            .left(Duration::from_millis(400), Easing::EaseOut)
            .background(Duration::from_millis(300), Easing::EaseOut))
}
```

### Color Cycling

```rust
fn color_box(id: &str, base_hue: f32, focused: bool) -> Element {
    let (l, c, h) = if focused {
        (0.7, 0.18, (base_hue + 30.0) % 360.0)  // Shift hue when focused
    } else {
        (0.4, 0.1, base_hue)
    };

    Element::box_()
        .id(id)
        .focusable(true)
        .style(Style::new().background(Color::oklch(l, c, h)))
        .transitions(Transitions::new()
            .background(Duration::from_millis(500), Easing::EaseInOut))
}
```

### Reduced Motion Toggle

```rust
Event::Key { key: Key::Char('r'), .. } => {
    let current = term.has_active_transitions();
    term.set_reduced_motion(!current);
}
```

## Easing Comparison

The example includes buttons for each easing type:
- **Linear**: Constant rate
- **Ease In**: Starts slow, accelerates
- **Ease Out**: Starts fast, decelerates
- **Ease In-Out**: Slow-fast-slow

Focus each button to see the difference in animation feel.

## Controls

- **Tab**: Navigate to see color transitions
- **Click**: Trigger button active state
- **r**: Toggle reduced motion
- **q/Escape**: Quit
