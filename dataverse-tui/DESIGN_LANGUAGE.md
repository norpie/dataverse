# Design Language

This document defines the visual and interaction patterns for dataverse-tui.

## Colors

Use the default rafter theme. Key colors:
- `interact` - Titles, highlights
- `primary` - Primary text, keybind hints
- `muted` - Secondary text, descriptions
- `surface` - Modal backgrounds and elements which need to pop out from the background (tables, trees, lists, panels) [in modals use surface2 for tables/lists to create layering]
- `background` - App background

## Density

Compact - minimize whitespace while maintaining readability.

## Spacing

- **Padding**: `(1, 2)` - vertical 1, horizontal 2 (matches visual spacing due to character aspect ratio)
- **Gap**: `1` between header, content, and footer in containers. NOT between every element, we prioritize vertical compactness and info density.
- **Borders**: None

## Typography

- **Titles**: Bold, interact color
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

### Form Fields

Use the `label` parameter on `input`, `select`, and `autocomplete` widgets. The label renders in muted color directly above the field (no gap). Field-to-field spacing comes from the parent container's `gap`.

```rust
column (gap: 1) {
    input (state: self.name, id: "name", label: "Name", placeholder: "Enter name...")
    input (state: self.email, id: "email", label: "Email", placeholder: "user@example.com")
    select (state: self.country, id: "country", label: "Country", placeholder: "Select...")
}
```

Labels should always be above the field, never inline beside it. Inline labels waste horizontal space and make vertical scanning difficult.

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
- Title in bold interact color
- Auto size for simple dialogs, `Lg` for complex ones (e.g., launcher)
- Bottom button row should be pushed to the bottom using `justify: end` on the outer column or a spacer element

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

If loading, show a spinner.
If static empty state, show a horitontally and vertically centered message.

## Feedback

- **Toasts**: Informational messages, minor errors
- **Modals**: Critical errors, confirmations, important decisions

Toast duration: use defaults.

## Selection

Use default rafter widget styling for selection highlighting.

## Status Indicator

Colored dot (●) with label showing system state.

```rust
row (gap: 1) {
    text (content: "●") style (fg: success)
    text (content: "Running")
}
```

## Emphasis Point

Accent dot (●) with interact label, followed by secondary text. Keys in primary color.

```rust
row (gap: 1) {
    text (content: "●") style (fg: interact)
    text (content: "Tip") style (fg: interact)
    text (content: " Use ")
    text (content: "ctrl+q") style (fg: primary)
    text (content: " to quickly quit")
}
```

Example: "● Tip Use the ctrl+q shortcut to quickly quit the program"
