# Animations & Transitions

tuidom supports smooth animated transitions between property values.

## Configuring Transitions

Add transitions to elements using the `Transitions` builder:

```rust
use std::time::Duration;
use tuidom::{Element, Easing, Transitions, Style, Color};

Element::box_()
    .style(Style::new().background(bg_color))
    .transitions(Transitions::new()
        .background(Duration::from_millis(300), Easing::EaseOut))
```

When the background color changes between frames, it smoothly animates over 300ms.

## Supported Properties

Transitions can be applied to:

| Property | Method | Description |
|----------|--------|-------------|
| `left` | `.left(duration, easing)` | Left position offset |
| `top` | `.top(duration, easing)` | Top position offset |
| `right` | `.right(duration, easing)` | Right position offset |
| `bottom` | `.bottom(duration, easing)` | Bottom position offset |
| `width` | `.width(duration, easing)` | Element width |
| `height` | `.height(duration, easing)` | Element height |
| `background` | `.background(duration, easing)` | Background color |
| `foreground` | `.foreground(duration, easing)` | Text/foreground color |

## Group Setters

Convenience methods for common combinations:

```rust
// Position properties (left, top, right, bottom)
Transitions::new().position(Duration::from_millis(400), Easing::EaseOut)

// Size properties (width, height)
Transitions::new().size(Duration::from_millis(300), Easing::EaseInOut)

// Color properties (background, foreground)
Transitions::new().colors(Duration::from_millis(200), Easing::Linear)

// All properties
Transitions::new().all(Duration::from_millis(300), Easing::EaseOut)
```

## How It Works

1. **Snapshot**: When an element first appears, tuidom captures its current property values
2. **Detection**: Each frame, tuidom compares new values to the snapshot
3. **Transition**: When values change, an active transition is created
4. **Interpolation**: During rendering, values are interpolated based on elapsed time
5. **Completion**: When the transition finishes, the snapshot is updated

The `Terminal` handles this automatically via its internal `AnimationState`.

## Example: Animated Button

```rust
fn button(id: &str, label: &str, focused: bool) -> Element {
    let bg = if focused {
        Color::oklch(0.5, 0.15, 140.0)  // Bright green
    } else {
        Color::oklch(0.3, 0.05, 250.0)  // Dim blue
    };

    Element::text(label)
        .id(id)
        .focusable(true)
        .style(Style::new().background(bg).bold())
        .transitions(Transitions::new()
            .background(Duration::from_millis(200), Easing::EaseOut))
}
```

When focus changes, the background smoothly transitions.

## Example: Moving Element

```rust
fn sliding_box(is_open: bool) -> Element {
    let left = if is_open { 0 } else { -30 };

    Element::box_()
        .position(Position::Relative)
        .left(left)
        .width(Size::Fixed(30))
        .transitions(Transitions::new()
            .left(Duration::from_millis(400), Easing::EaseOut))
        .child(content())
}
```

## Animation-Aware Polling

When transitions are active, `Terminal::poll` uses short timeouts (~16ms) to maintain smooth animation:

```rust
// Normally blocks indefinitely
let events = term.poll(None)?;

// With active transitions, effectively becomes:
// term.poll(Some(Duration::from_millis(16)))?
```

Check if animations are running:

```rust
if term.has_active_transitions() {
    // UI is animating
}
```

## Accessibility

Enable reduced motion for users who prefer it:

```rust
term.set_reduced_motion(true);
```

When enabled, transitions complete instantly (no animation).

## Performance

- Transitions only affect changed properties
- Non-animated elements have zero overhead
- Color interpolation uses OKLCH for perceptually smooth results
- Only dirty cells are redrawn

## Next Steps

- [Easing Functions](./animations/easing.md) - Available easing curves
- [Property Transitions](./animations/transitions.md) - Transitions builder reference
