# Understanding the Render Loop

tuidom uses an immediate-mode rendering pattern where the UI is rebuilt from scratch every frame. This chapter explains how the render loop works and best practices for structuring your application.

## The Basic Loop

Every tuidom application follows this pattern:

```rust
fn main() -> std::io::Result<()> {
    let mut term = Terminal::new()?;
    let mut state = AppState::default();

    loop {
        // 1. Build UI from state
        let ui = build_ui(&state);

        // 2. Render
        term.render(&ui)?;

        // 3. Poll for events
        let events = term.poll(None)?;

        // 4. Update state
        for event in events {
            update_state(&mut state, event);
        }
    }
}
```

## Step 1: Build UI

The UI is a pure function of your application state:

```rust
struct AppState {
    count: i32,
    focus: FocusState,
}

fn build_ui(state: &AppState) -> Element {
    Element::col()
        .child(Element::text(format!("Count: {}", state.count)))
        .child(button("increment", "+1", state.focus.focused()))
        .child(button("decrement", "-1", state.focus.focused()))
}
```

This approach has several benefits:
- **Predictable**: Same state always produces same UI
- **Debuggable**: Easy to inspect state and understand the UI
- **No stale state**: No widget state to get out of sync

## Step 2: Render

`term.render(&ui)` does several things:

1. **Layout**: Computes positions and sizes for all elements
2. **Animation**: Updates transition state and interpolates values
3. **Buffer**: Renders elements to an internal buffer
4. **Diff**: Compares with previous frame
5. **Output**: Writes only changed cells to the terminal

```rust
let layout = term.render(&ui)?;

// Access computed layout info
if let Some(rect) = layout.get("my-element") {
    println!("Element at ({}, {}), size {}x{}",
        rect.x, rect.y, rect.width, rect.height);
}
```

### Layout Access

The `render` method returns a `LayoutResult` that lets you query element positions:

```rust
let layout = term.render(&ui)?;

// Get element rectangle
let rect = layout.get("my-element");

// For scrollable elements
let content = layout.content_size("scroll-container");
let viewport = layout.viewport_size("scroll-container");
```

## Step 3: Poll for Events

`term.poll(timeout)` waits for user input:

```rust
// Block until event
let events = term.poll(None)?;

// Wait up to 100ms
let events = term.poll(Some(Duration::from_millis(100)))?;

// Non-blocking check
let events = term.poll(Some(Duration::ZERO))?;
```

When animations are active, `poll` automatically uses short timeouts (~16ms) to maintain smooth animation, regardless of the timeout you specify.

### Raw vs Processed Events

`poll` returns raw crossterm events. For high-level events with focus and targeting, use `FocusState::process_events`:

```rust
let raw_events = term.poll(None)?;
let events = focus.process_events(&raw_events, &ui, term.layout());
```

## Step 4: Update State

Handle events and update your application state:

```rust
for event in events {
    match event {
        Event::Key { key: Key::Enter, target: Some(id), .. } => {
            match id.as_str() {
                "increment" => state.count += 1,
                "decrement" => state.count -= 1,
                _ => {}
            }
        }
        Event::Click { target: Some(id), .. } => {
            // Handle click on element with `id`
        }
        _ => {}
    }
}
```

## Animation-Aware Polling

When elements have active transitions, `poll` behaves differently:

```rust
// With active animations, this effectively becomes:
// poll(Some(Duration::from_millis(16)))
// to maintain ~60fps animation
let events = term.poll(None)?;

// Check if animations are running
if term.has_active_transitions() {
    // UI is animating
}
```

## Structuring Larger Applications

For larger apps, organize UI building into focused functions:

```rust
fn build_ui(state: &AppState) -> Element {
    Element::col()
        .child(header(state))
        .child(content(state))
        .child(footer(state))
}

fn header(state: &AppState) -> Element {
    Element::row()
        .child(Element::text(&state.title))
        .child(nav_menu(state.focus.focused()))
}

fn content(state: &AppState) -> Element {
    match state.current_view {
        View::Dashboard => dashboard(state),
        View::Settings => settings(state),
        View::About => about(state),
    }
}
```

## Performance Considerations

1. **Element reuse**: Elements are cheap to create. Don't worry about creating new elements each frame.

2. **Differential rendering**: Only changed cells are written to the terminal, so static UI sections are essentially free.

3. **Layout caching**: If the element tree structure and constraints haven't changed, layout computation is fast.

4. **Avoid blocking**: Don't do heavy computation in the render loop. Use background tasks or async for long operations.

## Next Steps

- [Elements](../core/elements.md) - Deep dive into element types and the builder API
- [Events](../interactivity/events.md) - Complete event reference
- [Animations](../advanced/animations.md) - Adding smooth transitions
