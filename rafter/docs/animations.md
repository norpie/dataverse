# Animations

Rafter supports animations while remaining efficient on terminal rendering.

## Render Strategy

Rafter uses an intelligent render loop:

- **Idle**: No renders when nothing changes (efficient)
- **User input**: Re-render on keyboard/mouse events
- **State change**: Re-render when state mutates
- **Animation active**: Runs at configured FPS (30-60fps)
- **Animation complete**: Returns to idle

This means animations are smooth when running, but CPU usage drops to near-zero when idle.

## Declarative Animations

### Animate In/Out

```rust
view! {
    toast (
        animate_in: slide(from: right, duration: 150ms),
        animate_out: slide(to: right, duration: 150ms),
    ) {
        text { "Record saved" }
    }
}
```

### Built-in Animation Types

```rust
// Slide
slide(from: left | right | top | bottom, duration: ms)
slide(to: left | right | top | bottom, duration: ms)

// Fade
fade_in(duration: ms)
fade_out(duration: ms)

// Expand/collapse
expand(from: 0, to: 100%, duration: ms)
collapse(from: 100%, to: 0, duration: ms)
```

## State-Driven Transitions

Smooth transitions when state changes:

```rust
view! {
    row (
        bg: if selected { primary } else { surface },
        transition: bg(duration: 100ms),
    ) {
        text { record.name }
    }
}
```

Multiple transitions:

```rust
view! {
    column (
        width: if expanded { 100% } else { 50% },
        opacity: if visible { 1.0 } else { 0.0 },
        transition: width(duration: 200ms), opacity(duration: 150ms),
    ) {
        // content
    }
}
```

## Animated Components

### Spinner

```rust
view! {
    spinner { }  // Default spinner
    
    spinner (style: dots) { }
    spinner (style: line) { }
    spinner (style: pulse) { }
}
```

### Progress Bar

```rust
view! {
    progress_bar (
        value: self.progress,
        max: 100,
        animate: true,  // Smooth value transitions
    ) { }
}
```

### Animated Element

For custom animations:

```rust
view! {
    animated (
        kind: spin,
        duration: 500ms,
    ) {
        text { "⠋" }
    }
    
    animated (
        kind: pulse,
        duration: 1000ms,
    ) {
        text (color: error) { "!" }
    }
}
```

## Reduce Motion

Users can disable or reduce animations for accessibility.

### User Configuration

```toml
# User config
[accessibility]
reduce_motion = true
```

### Animation Behavior Settings

```rust
view! {
    toast (
        animate_in: slide(from: right),
        reduce_motion: skip,  // No animation if reduce_motion enabled
    ) { }
    
    spinner (
        reduce_motion: slower,  // Still animate, but slower
    ) { }
    
    animated (
        kind: pulse,
        reduce_motion: static,  // Show static indicator instead
    ) {
        text { "!" }
    }
}
```

### Reduce Motion Options

| Option | Behavior |
|--------|----------|
| `skip` | No animation, instant state change |
| `slower` | Animate but at reduced speed |
| `static` | Show static version (for spinners) |

### Checking Reduce Motion

```rust
fn view(&self, cx: ViewContext) -> Node {
    let animate = !cx.reduce_motion();
    
    view! {
        column (
            transition: if animate { Some(bg(duration: 100ms)) } else { None },
        ) {
            // content
        }
    }
}
```

## Animation Triggers

Animations can be triggered by:

### State Changes

```rust
// Background color animates when `selected` changes
row (
    bg: if selected { primary } else { surface },
    transition: bg(duration: 100ms),
) { }
```

### Element Appearance

```rust
// Animates when element enters the view
toast (animate_in: slide(from: right)) { }
```

### Element Removal

```rust
// Animates when element leaves the view
toast (animate_out: fade_out(duration: 150ms)) { }
```

### Explicit Triggers

```rust
#[handler]
fn shake_input(&mut self, cx: AppContext) {
    cx.animate("email_input", shake(duration: 300ms));
}

view! {
    input (id: "email_input") { }
}
```

## Performance Considerations

- Animations only run the render loop when active
- Multiple simultaneous animations share the same render loop
- Terminal rendering is optimized (only changed cells redrawn)
- Complex animations may need lower FPS on slower terminals

### FPS Configuration

```rust
rafter::Runtime::new()
    .animation_fps(30)  // Default: 30
```
