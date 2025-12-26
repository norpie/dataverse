# Animations

Rafter supports smooth transitions for style changes.

## Basic Transitions

Add `transition` to animate style changes:

```rust
fn page(&self) -> Node {
    let bg = if self.active.get() {
        Color::oklch(0.5, 0.15, 150.0)  // Green
    } else {
        Color::oklch(0.3, 0.1, 25.0)    // Red
    };

    page! {
        // id is required for transition tracking
        column (id: "status", bg: {bg}, transition: 300) {
            text { "Status indicator" }
        }
    }
}
```

## Transition Attributes

| Attribute | Type | Description |
|-----------|------|-------------|
| `transition` | number | Duration in milliseconds |
| `easing` | function | Easing curve |

## Easing Functions

| Easing | Description |
|--------|-------------|
| `linear` | Constant speed |
| `ease_in` | Start slow, accelerate |
| `ease_out` | Start fast, decelerate |
| `ease_in_out` | Slow at both ends |

```rust
page! {
    column (id: "panel", bg: {color}, transition: 500, easing: ease_out) {
        text { "Smooth transition" }
    }
}
```

## What Can Be Animated

Currently supported:
- Background color (`bg`)

The transition smoothly interpolates between colors over the specified duration.

## ID Requirement

Transitions require an `id` to track the element across renders:

```rust
// Works - element has id
column (id: "my-panel", bg: {color}, transition: 300) { ... }

// Won't animate - no id to track
column (bg: {color}, transition: 300) { ... }
```

## Example: Toggle Animation

```rust
#[app]
struct AnimDemo {
    enabled: bool,
}

#[app_impl]
impl AnimDemo {
    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "space" => toggle,
            "q" => quit,
        }
    }

    #[handler]
    async fn toggle(&self, _cx: &AppContext) {
        self.enabled.update(|v| *v = !*v);
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        cx.exit();
    }

    fn page(&self) -> Node {
        let enabled = self.enabled.get();
        let bg = if enabled {
            Color::oklch(0.45, 0.12, 145.0)  // Greenish
        } else {
            Color::oklch(0.35, 0.08, 25.0)   // Reddish
        };
        let status = if enabled { "ON" } else { "OFF" };

        page! {
            column (id: "main", padding: 2, bg: {bg}, transition: 400, easing: ease_out) {
                text (bold) { "Animation Demo" }
                text { format!("Status: {}", status) }
                text (fg: muted) { "Press Space to toggle" }
            }
        }
    }
}
```

## Example: Progress Indicator

```rust
fn page(&self) -> Node {
    let progress = self.progress.get();  // 0.0 to 1.0

    // Interpolate color based on progress
    let hue = 25.0 + (progress * 125.0);  // Red to green
    let bg = Color::oklch(0.5, 0.15, hue);

    page! {
        column (padding: 1) {
            text (bold) { "Loading..." }
            row (id: "progress-bar", bg: {bg}, width: 30, transition: 100) {
                text { format!("{:.0}%", progress * 100.0) }
            }
        }
    }
}
```

## Animation Manager

For more complex animations, you can use the `AnimationManager` directly (advanced usage):

```rust
use rafter::prelude::{AnimatedProperty, Easing};

// In your app state
#[app]
struct ComplexAnim {
    #[state(skip)]
    opacity: AnimatedProperty<f32>,
}

#[app_impl]
impl ComplexAnim {
    async fn on_start(&self, _cx: &AppContext) {
        self.opacity.set_target(1.0, Duration::from_millis(500), Easing::EaseOut);
    }

    fn page(&self) -> Node {
        let opacity = self.opacity.current();
        // Use opacity value...
    }
}
```

## Performance Considerations

- Transitions are rendered at the terminal's refresh rate
- Use reasonable durations (100-500ms for most UI)
- Complex animations may affect performance on slower terminals
- The framework automatically batches updates

## Reduce Motion

Respect user preferences:

```rust
fn page(&self) -> Node {
    let transition = if self.reduce_motion.get() { 0 } else { 300 };

    page! {
        column (id: "panel", bg: {color}, transition: {transition}) {
            // content
        }
    }
}
```

## Limitations

Current limitations:
- Only background color is animatable
- No keyframe animations
- No transform animations (scale, rotate)
- Element must have an `id` for tracking

Future versions may expand animation capabilities.
