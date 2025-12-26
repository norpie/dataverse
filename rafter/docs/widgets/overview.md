# Widget Overview

Widgets are self-managed UI components that handle their own state and events.

## Widget Philosophy

- **Self-managed**: Widgets maintain internal state (focus, selection, scroll position)
- **Bound to fields**: Widgets are declared as app fields and bound in the page
- **Event-driven**: Widgets emit events that trigger your handlers

## Basic Pattern

```rust
#[app]
struct MyApp {
    name: Input,        // Widget as field
    items: List<Item>,  // Generic widget
}

#[app_impl]
impl MyApp {
    async fn on_start(&self, _cx: &AppContext) {
        // Initialize widget
        self.name.set_placeholder("Enter name");
    }

    fn page(&self) -> Node {
        page! {
            column {
                // Bind widget in page
                input(bind: self.name, on_submit: on_submit)
            }
        }
    }
}
```

## Available Widgets

### Input Widgets

| Widget | Description |
|--------|-------------|
| [Button](button.md) | Clickable button |
| [Input](input.md) | Text input field |
| [Checkbox](checkbox.md) | Toggle checkbox |
| [RadioGroup](radio-group.md) | Mutually exclusive options |
| [Collapsible](collapsible.md) | Expandable section |

### Selection Widgets

| Widget | Description |
|--------|-------------|
| [List](list.md) | Selectable list of items |
| [Tree](tree.md) | Hierarchical tree view |
| [Table](table.md) | Sortable data table |
| [Select](select.md) | Dropdown selection |
| [Autocomplete](autocomplete.md) | Text input with suggestions |

### Layout Widgets

| Widget | Description |
|--------|-------------|
| [ScrollArea](scroll-area.md) | Scrollable container |

## Common Attributes

Most widgets accept these attributes in `page!`:

| Attribute | Description |
|-----------|-------------|
| `bind` | The widget field to bind |
| `id` | Widget identifier (for focus, events) |
| `on_click` | Click handler |
| `on_change` | Value change handler |
| `on_submit` | Submit handler (Enter key) |
| `width` | Widget width |
| `height` | Widget height |
| `flex` | Flex grow factor |

## Focus and Navigation

### Tab Navigation

Widgets are automatically focusable. Use Tab/Shift+Tab to cycle focus.

### Programmatic Focus

```rust
cx.focus("my-widget-id");
```

### Focus State

Widgets visually indicate focus state with highlighting.

## Widget Events

Widgets emit events that trigger handlers:

```rust
page! {
    input(
        bind: self.name,
        on_change: on_name_change,  // Every keystroke
        on_submit: on_name_submit   // Enter key
    )

    list(
        bind: self.items,
        on_activate: on_item_open,      // Enter key
        on_selection_change: on_select, // Selection changed
        on_cursor_move: on_cursor       // Cursor moved
    )
}
```

## Widget State

Widgets store state like selection, scroll position, and cursor:

```rust
// Query widget state
let selected = self.items.selected_ids();
let cursor = self.items.cursor_id();
let is_checked = self.checkbox.is_checked();
let value = self.input.value();

// Modify widget state
self.items.set_cursor(0);
self.items.select("item-1");
self.checkbox.set_checked(true);
self.input.set_value("Hello");
```

## Selection Modes

List, Tree, and Table support different selection modes:

```rust
// In on_start
self.items.set_selection_mode(SelectionMode::Single);   // One item
self.items.set_selection_mode(SelectionMode::Multiple); // Many items
self.items.set_selection_mode(SelectionMode::None);     // No selection
```

## Rendering Custom Items

List, Tree, and Table use traits for custom rendering:

```rust
impl ListItem for MyItem {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn render(&self, focused: bool, selected: bool) -> Node {
        page! {
            row (gap: 2) {
                text { if selected { "[x]" } else { "[ ]" } }
                text (bold: focused) { self.name.clone() }
            }
        }
    }
}
```

## Next Steps

Explore individual widget documentation:

- [Button](button.md) - Start here for simple interactivity
- [Input](input.md) - Text input and forms
- [List](list.md) - Item selection
