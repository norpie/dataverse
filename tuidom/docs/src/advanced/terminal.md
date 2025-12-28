# Terminal & Rendering

The `Terminal` struct manages the terminal and coordinates rendering.

## Terminal

### Creating a Terminal

```rust
let mut term = Terminal::new()?;
```

This:
- Enters raw mode
- Switches to alternate screen (preserves shell history)
- Hides the cursor
- Enables mouse capture

When dropped, the terminal is automatically restored.

### Methods

#### `size() -> (u16, u16)`

Get current terminal dimensions:

```rust
let (width, height) = term.size();
```

#### `poll(timeout) -> Vec<CrosstermEvent>`

Wait for input events:

```rust
// Block indefinitely
let events = term.poll(None)?;

// Wait up to 100ms
let events = term.poll(Some(Duration::from_millis(100)))?;

// Non-blocking check
let events = term.poll(Some(Duration::ZERO))?;
```

Returns raw crossterm events. Use `FocusState::process_events` to convert to high-level events.

#### `render(&mut self, root: &Element) -> &LayoutResult`

Render an element tree:

```rust
let layout = term.render(&ui)?;
```

This:
1. Computes layout for all elements
2. Updates animation state
3. Renders to an internal buffer
4. Diffs against the previous buffer
5. Writes only changed cells to the terminal

#### `layout() -> &LayoutResult`

Get the layout from the last render:

```rust
let rect = term.layout().get("my-element");
```

#### `set_reduced_motion(enabled: bool)`

Enable accessibility mode that skips animations:

```rust
term.set_reduced_motion(true);
```

#### `has_active_transitions() -> bool`

Check if any transitions are currently animating:

```rust
if term.has_active_transitions() {
    // UI is animating
}
```

## Next Steps

- [Buffer System](./terminal/buffer.md) - Buffer and Cell structures
- [Differential Rendering](./terminal/diff-rendering.md) - How rendering is optimized
