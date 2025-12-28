# Property Transitions

The `Transitions` struct configures which properties animate and how.

## Creating Transitions

```rust
use std::time::Duration;
use tuidom::{Transitions, Easing};

let transitions = Transitions::new()
    .background(Duration::from_millis(300), Easing::EaseOut)
    .left(Duration::from_millis(400), Easing::EaseInOut);
```

## Individual Property Methods

### Position Properties

```rust
.left(duration, easing)    // Animate left offset
.top(duration, easing)     // Animate top offset
.right(duration, easing)   // Animate right offset
.bottom(duration, easing)  // Animate bottom offset
```

### Size Properties

```rust
.width(duration, easing)   // Animate width
.height(duration, easing)  // Animate height
```

### Color Properties

```rust
.background(duration, easing)  // Animate background color
.foreground(duration, easing)  // Animate text color
```

## Group Methods

### `.position(duration, easing)`

Set the same transition for all position offsets:

```rust
Transitions::new().position(Duration::from_millis(400), Easing::EaseOut)
// Equivalent to:
// .left(...).top(...).right(...).bottom(...)
```

### `.size(duration, easing)`

Set the same transition for width and height:

```rust
Transitions::new().size(Duration::from_millis(300), Easing::EaseInOut)
// Equivalent to:
// .width(...).height(...)
```

### `.colors(duration, easing)`

Set the same transition for background and foreground:

```rust
Transitions::new().colors(Duration::from_millis(200), Easing::EaseOut)
// Equivalent to:
// .background(...).foreground(...)
```

### `.all(duration, easing)`

Set the same transition for all properties:

```rust
Transitions::new().all(Duration::from_millis(300), Easing::EaseOut)
```

## Checking Configuration

### `.has_any() -> bool`

Check if any transition is configured:

```rust
if transitions.has_any() {
    // Element has animations
}
```

## TransitionConfig

Each property uses a `TransitionConfig`:

```rust
pub struct TransitionConfig {
    pub duration: Duration,
    pub easing: Easing,
}
```

Access directly:

```rust
if let Some(config) = transitions.background {
    println!("Background: {:?} with {:?}", config.duration, config.easing);
}
```

## Overriding Transitions

Later calls override earlier ones:

```rust
Transitions::new()
    .all(Duration::from_millis(300), Easing::Linear)
    .background(Duration::from_millis(500), Easing::EaseOut)  // Override
```

## Complete Example

```rust
fn animated_panel(expanded: bool, focused: bool) -> Element {
    let width = if expanded { Size::Fixed(60) } else { Size::Fixed(30) };
    let bg = if focused {
        Color::oklch(0.35, 0.1, 250.0)
    } else {
        Color::oklch(0.25, 0.02, 250.0)
    };

    Element::box_()
        .width(width)
        .height(Size::Fill)
        .style(Style::new().background(bg).border(Border::Rounded))
        .transitions(Transitions::new()
            .width(Duration::from_millis(400), Easing::EaseOut)
            .background(Duration::from_millis(200), Easing::EaseOut))
        .child(content())
}
```

## Default Values

`Transitions::default()` has all properties set to `None` (no transitions).
