# Hit Testing

Hit testing determines which element is under a given screen position.

## Functions

### `hit_test(layout, root, x, y) -> Option<String>`

Find the deepest **clickable** element at position (x, y):

```rust
use tuidom::hit_test;

if let Some(id) = hit_test(&layout, &root, mouse_x, mouse_y) {
    println!("Clicked on: {}", id);
}
```

Returns `None` if no clickable element contains the point.

### `hit_test_any(layout, root, x, y) -> Option<String>`

Find the deepest element (clickable or not) at position (x, y):

```rust
use tuidom::hit_test_any;

if let Some(id) = hit_test_any(&layout, &root, mouse_x, mouse_y) {
    println!("Element under mouse: {}", id);
}
```

Always returns an element if the point is within the root.

### `hit_test_focusable(layout, root, x, y) -> Option<String>`

Find the deepest **focusable** element at position (x, y):

```rust
use tuidom::hit_test_focusable;

if let Some(id) = hit_test_focusable(&layout, &root, mouse_x, mouse_y) {
    focus.focus(&id);
}
```

Used internally for focus-follows-mouse behavior.

## How It Works

1. Start at the root element
2. Check if point (x, y) is within the element's bounds
3. Check children in reverse order (later = rendered on top)
4. Return the deepest matching element

The reverse order ensures that elements rendered on top (higher z-index or later in tree) take priority.

## Requirements

- **Layout required**: Hit testing uses computed layout positions from `LayoutResult`
- **ID required**: Elements must have IDs to be returned from hit testing
- **Clickable/Focusable**: `hit_test` only returns clickable elements; `hit_test_focusable` only returns focusable elements

## Usage Examples

### Custom Click Handling

```rust
Event::Click { x, y, button, .. } => {
    // Use hit_test_any for elements that aren't marked clickable
    if let Some(id) = hit_test_any(&layout, &root, x, y) {
        match id.as_str() {
            "special-area" => handle_special_click(),
            _ => {}
        }
    }
}
```

### Tooltip Positioning

```rust
let hovered = hit_test_any(&layout, &root, mouse_x, mouse_y);
if hovered.as_deref() == Some("info-icon") {
    show_tooltip = true;
}
```

### Context Menu

```rust
Event::Click { x, y, button: MouseButton::Right, .. } => {
    if let Some(id) = hit_test(&layout, &root, x, y) {
        context_menu_target = Some(id);
        context_menu_pos = (x, y);
    }
}
```

## Automatic Usage

Hit testing is used automatically by:

- `FocusState::process_events` - For click event targeting
- `FocusState::process_events` - For focus-follows-mouse
- `ScrollState::process_events` - For scroll event targeting

You typically only need to call hit test functions directly for custom behaviors.

## Z-Index Considerations

Hit testing respects visual stacking order:

1. Elements later in the child list are checked first
2. Higher z-index elements are checked before lower ones
3. The first match (deepest, topmost) is returned

```rust
Element::box_()
    .child(
        Element::box_()
            .id("back")
            .z_index(0)
            .clickable(true)
    )
    .child(
        Element::box_()
            .id("front")
            .z_index(1)
            .clickable(true)
    )

// Click on overlapping area → "front" is returned
```

## Performance

Hit testing traverses the element tree, so performance is O(n) where n is the number of elements. For typical UIs, this is negligible.
