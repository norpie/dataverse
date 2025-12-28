# Differential Rendering

tuidom uses double-buffering and differential updates for efficient rendering.

## How It Works

### Double Buffering

Two buffers are maintained:
- **Current buffer**: What we're rendering this frame
- **Previous buffer**: What was rendered last frame

```
Frame N:
  [Current] ← Render new UI here
  [Previous] ← Last frame's content

After render:
  [Previous] ← Was current, now reference
  [Current] ← Fresh buffer for next frame
```

### Diff and Update

Only changed cells are written to the terminal:

```rust
for (x, y, cell) in current.diff(&previous) {
    // Move cursor to (x, y)
    // Set colors if changed
    // Write character
}
```

### Optimization: Skip Consecutive Updates

The terminal tracks the last written position. Consecutive cells skip cursor movement:

```
Position 1: write 'H'
Position 2: write 'e' (no cursor move needed)
Position 3: write 'l' (no cursor move needed)
...
Position 10: (gap) move cursor
Position 10: write 'X'
```

## Render Pipeline

1. **Resize check**: Recreate buffers if terminal size changed
2. **Animation update**: Capture snapshots, detect changes, update transitions
3. **Clear buffer**: Reset current buffer to defaults
4. **Layout**: Compute positions and sizes for all elements
5. **Render to buffer**: Draw elements to current buffer
6. **Diff**: Find cells that changed from previous frame
7. **Write**: Output changed cells to terminal
8. **Swap**: Previous ← Current for next frame

## Performance Characteristics

### Best Case: Static UI

When nothing changes:
- Diff finds zero differences
- No terminal output
- Only layout computation (fast)

### Typical Case: Partial Updates

When only some elements change:
- Diff finds small number of changes
- Minimal terminal output
- Very fast

### Worst Case: Full Redraw

When everything changes (resize, scroll, etc.):
- Diff finds many differences
- More terminal output
- Still efficient due to batching

## Why This Matters

### Terminal I/O is Slow

Writing to the terminal is expensive:
- Each character requires output
- Cursor movement has overhead
- Color changes require escape sequences

By only writing changes, we minimize I/O.

### Flicker-Free Updates

Full redraws cause visible flicker. Differential updates:
- Don't clear the screen
- Only change what needs to change
- Result in smooth visuals

### Animation Support

Smooth animations require:
- High frame rates (~60 FPS)
- Low per-frame overhead
- Efficient updates

Differential rendering enables smooth transitions.

## Debugging

To see what's being rendered, use logging:

```rust
let log_file = File::create("app.log")?;
WriteLogger::init(LevelFilter::Debug, Config::default(), log_file)?;
```

The render timing shows:
- `clear`: Time to reset buffer
- `layout`: Time to compute positions
- `render`: Time to draw to buffer
- `flush`: Time to output changes to terminal
