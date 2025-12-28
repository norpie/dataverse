# Layout Enums Reference

## Direction

Main axis for flex layout.

```rust
pub enum Direction {
    Row,     // Horizontal (left to right)
    Column,  // Vertical (top to bottom) - default
}
```

## Position

Positioning mode for elements.

```rust
pub enum Position {
    Static,   // Normal flow - default
    Relative, // Offset from normal position
    Absolute, // Positioned relative to parent
}
```

## Justify

Main axis alignment for flex containers.

```rust
pub enum Justify {
    Start,        // Pack at start - default
    Center,       // Center
    End,          // Pack at end
    SpaceBetween, // Even space between
    SpaceAround,  // Even space around
}
```

## Align

Cross axis alignment for flex containers.

```rust
pub enum Align {
    Start,   // Align to start - default
    Center,  // Center
    End,     // Align to end
    Stretch, // Stretch to fill
}
```

## Wrap

Multi-line flex behavior.

```rust
pub enum Wrap {
    NoWrap, // Keep on single line - default
    Wrap,   // Wrap to multiple lines
}
```

## Defaults

| Type | Default |
|------|---------|
| `Direction` | `Column` |
| `Position` | `Static` |
| `Justify` | `Start` |
| `Align` | `Start` |
| `Wrap` | `NoWrap` |
