# Design Language

This document defines the visual and interaction patterns for dataverse-tui.

## Colors

Use the default rafter theme. Key colors:
- `accent` - Titles, highlights
- `primary` - Primary text, keybind hints
- `muted` - Secondary text, descriptions
- `surface` - Modal backgrounds
- `background` - App background

## Density

Compact - minimize whitespace while maintaining readability.

## Spacing

- **Padding**: `(1, 2)` - vertical 1, horizontal 2 (matches visual spacing due to character aspect ratio)
- **Gap**: `1` between elements
- **Borders**: None

## Typography

- **Titles**: Bold, accent color
- **Body text**: Default color
- **Secondary text**: Muted color

## Icons

Use UTF-8 characters where icons are needed:
- `■` `□` `▣` - Squares
- `▲` `▼` `▶` `◀` - Triangles
- `●` `○` - Circles

For tree/list navigation, use default rafter widget styling.

## Components

### Buttons

Label in primary color, optional keybind hint in muted color.

```rust
button (label: "Ok", hint: "y", id: "ok")
// Renders as: "Ok y" (Ok in primary, y in muted)
```

### Keybind Hints

Key in primary color, description in muted color. Multiple hints on one line use `justify: between`.

```rust
row (width: fill, justify: between) {
    row (gap: 1) {
        text (content: "esc") style (fg: primary)
        text (content: "close") style (fg: muted)
    }
    row (gap: 1) {
        text (content: "enter") style (fg: primary)
        text (content: "select") style (fg: muted)
    }
}
```

Key format: lowercase, actual key name (e.g., `esc` not `Escape`, `ctrl+p` not `C-p`).

### Modals

- Use `padding: (1, 2)` on content column
- Title in bold accent color
- Auto size for simple dialogs, `Lg` for complex ones (e.g., launcher)

### Confirmation Dialogs

Use the standardized `ConfirmModal`:

```rust
use crate::modals::ConfirmModal;

let confirmed = gx.modal(ConfirmModal::new("Delete this item?")).await;

// With custom title:
let confirmed = gx.modal(
    ConfirmModal::new("Are you sure?").title("Warning")
).await;
```

Buttons: Cancel (left) / Ok (right), with `n` and `y` hints.

## Loading States

Use the `Spinner` widget:

```rust
use crate::widgets::Spinner;

Spinner::new().build()

// Customized:
Spinner::new()
    .track_width(8)
    .snake_len(6)
    .hue(320.0)
    .frame_ms(60)
    .build()
```

## Empty States

Show nothing - no "Empty" placeholders. If loading, show a spinner.

## Feedback

- **Toasts**: Informational messages, minor errors
- **Modals**: Critical errors, confirmations, important decisions

Toast duration: use defaults.

## Selection

Use default rafter widget styling for selection highlighting.
