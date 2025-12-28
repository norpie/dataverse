# Focus Management

`FocusState` tracks keyboard focus and handles Tab navigation.

## Creating FocusState

```rust
let mut focus = FocusState::new();
```

## Basic Usage

```rust
loop {
    // Pass focused element to UI
    let ui = build_ui(focus.focused());
    term.render(&ui)?;

    // Process events with focus handling
    let raw = term.poll(None)?;
    let events = focus.process_events(&raw, &ui, term.layout());

    // Handle events
    for event in events {
        // ...
    }
}
```

## Methods

### `focused() -> Option<&str>`

Get the currently focused element ID:

```rust
if let Some(id) = focus.focused() {
    println!("Focused: {}", id);
}
```

### `focus(id: &str) -> bool`

Programmatically focus an element. Returns `true` if focus changed:

```rust
focus.focus("my-input");
```

### `blur() -> bool`

Clear focus. Returns `true` if something was focused:

```rust
focus.blur();
```

### `focus_next(root: &Element) -> Option<String>`

Focus the next focusable element:

```rust
if let Some(new_id) = focus.focus_next(&root) {
    // Focus moved to new_id
}
```

### `focus_prev(root: &Element) -> Option<String>`

Focus the previous focusable element:

```rust
if let Some(new_id) = focus.focus_prev(&root) {
    // Focus moved to new_id
}
```

### `process_events(...) -> Vec<Event>`

Convert raw crossterm events to high-level events:

```rust
let events = focus.process_events(&raw_events, &root, &layout);
```

This handles:
- Tab/Shift+Tab navigation
- Focus follows mouse (hover over focusable elements)
- Targeting key events to focused element
- Click/scroll event targeting via hit testing

## Making Elements Focusable

Mark elements as focusable with `.focusable(true)`:

```rust
Element::text("Button")
    .id("my-button")      // ID required for focus tracking
    .focusable(true)
```

## Focus Order

Focus order follows document order—the order elements appear in the tree:

```rust
Element::col()
    .child(Element::text("First").id("1").focusable(true))   // Tab: 1st
    .child(Element::text("Second").id("2").focusable(true))  // Tab: 2nd
    .child(Element::text("Third").id("3").focusable(true))   // Tab: 3rd
```

Tab cycles through focusable elements in this order. Shift+Tab goes in reverse.

## Focus Events

Focus changes emit `Focus` and `Blur` events:

```rust
Event::Focus { target } => {
    // Element gained focus
}
Event::Blur { target } => {
    // Element lost focus
}
```

## Visual Feedback

Update element appearance based on focus state:

```rust
fn build_ui(focused: Option<&str>) -> Element {
    Element::col()
        .child(text_input("name", "Name:", focused))
        .child(text_input("email", "Email:", focused))
}

fn text_input(id: &str, label: &str, focused: Option<&str>) -> Element {
    let is_focused = focused == Some(id);

    let bg = if is_focused {
        Color::oklch(0.35, 0.08, 250.0)
    } else {
        Color::oklch(0.25, 0.02, 250.0)
    };

    let border = if is_focused {
        Border::Double
    } else {
        Border::Single
    };

    Element::row()
        .child(Element::text(label))
        .child(
            Element::text("...")
                .id(id)
                .focusable(true)
                .style(Style::new()
                    .background(bg)
                    .border(border))
        )
}
```

## Focus Follows Mouse

By default, hovering over a focusable element focuses it. This is handled in `process_events`.

The sequence is:
1. Mouse moves over focusable element
2. Previous element (if any) receives `Blur` event
3. New element receives `Focus` event
4. `focus.focused()` returns the new element's ID

## Utility Functions

### `collect_focusable(element: &Element) -> Vec<String>`

Get all focusable element IDs in tree order:

```rust
use tuidom::collect_focusable;

let focusable_ids = collect_focusable(&root);
// ["input_1", "input_2", "submit_btn", ...]
```

## Common Patterns

### Form Navigation

```rust
fn form(focused: Option<&str>) -> Element {
    Element::col()
        .gap(1)
        .child(field("name", "Name", focused))
        .child(field("email", "Email", focused))
        .child(field("message", "Message", focused))
        .child(
            Element::row()
                .gap(2)
                .child(button("cancel", "Cancel", focused))
                .child(button("submit", "Submit", focused))
        )
}
```

### Initial Focus

Focus an element when the app starts:

```rust
let mut focus = FocusState::new();
focus.focus("first-input");
```

### Conditional Focus

Disable focus on certain conditions:

```rust
Element::text("Submit")
    .id("submit")
    .focusable(is_valid)  // Only focusable when form is valid
```
