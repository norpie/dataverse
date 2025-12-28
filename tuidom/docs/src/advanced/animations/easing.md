# Easing Functions

Easing functions control the rate of change during a transition.

## Available Easings

### `Easing::Linear`

Constant rate of change. Progress is proportional to time.

```rust
Transitions::new()
    .background(Duration::from_millis(300), Easing::Linear)
```

Best for: Simple animations where consistent speed is desired.

### `Easing::EaseIn`

Starts slow, accelerates toward the end.

```rust
Transitions::new()
    .left(Duration::from_millis(400), Easing::EaseIn)
```

Formula: `t²` (quadratic)

Best for: Elements entering the screen, building momentum.

### `Easing::EaseOut`

Starts fast, decelerates toward the end.

```rust
Transitions::new()
    .background(Duration::from_millis(200), Easing::EaseOut)
```

Formula: `1 - (1-t)²` (inverse quadratic)

Best for: Hover effects, focus transitions, settling into place.

### `Easing::EaseInOut`

Starts slow, speeds up in the middle, slows at the end.

```rust
Transitions::new()
    .position(Duration::from_millis(500), Easing::EaseInOut)
```

Formula: Combines EaseIn for first half, EaseOut for second half.

Best for: Complete motions, elements moving from one place to another.

## Choosing an Easing

| Effect | Recommended Easing |
|--------|-------------------|
| Color change (hover, focus) | `EaseOut` |
| Element appearing | `EaseOut` |
| Element disappearing | `EaseIn` |
| Element moving | `EaseInOut` or `EaseOut` |
| Size change | `EaseOut` |
| Continuous animation | `Linear` |

## Mathematical Formulas

```
Linear:    f(t) = t
EaseIn:    f(t) = t²
EaseOut:   f(t) = 1 - (1-t)²
EaseInOut: f(t) = t < 0.5 ? 2t² : 1 - (-2t + 2)² / 2
```

Where `t` is progress from 0.0 to 1.0.

## Using Easing

```rust
use tuidom::Easing;

// Apply to a transition
Transitions::new()
    .background(Duration::from_millis(300), Easing::EaseOut)

// Apply easing directly (for custom animations)
let progress = elapsed.as_secs_f32() / duration.as_secs_f32();
let eased = Easing::EaseOut.apply(progress.clamp(0.0, 1.0));
```

## Easing Comparison

At t=0.5 (halfway through animation):

| Easing | Output at t=0.5 |
|--------|-----------------|
| Linear | 0.50 |
| EaseIn | 0.25 (25% complete) |
| EaseOut | 0.75 (75% complete) |
| EaseInOut | 0.50 |

EaseIn feels sluggish at first, EaseOut feels responsive.
