# Checkbox

A toggle checkbox with optional label.

## State Field

```rust
#[app]
struct MyApp {
    accept_terms: Checkbox,
}
```

## Basic Usage

```rust
fn page(&self) -> Node {
    page! {
        checkbox(bind: self.accept_terms)
    }
}
```

## Initialization

```rust
async fn on_start(&self, _cx: &AppContext) {
    self.accept_terms.set_label("I accept the terms and conditions");
    self.accept_terms.set_checked(false);
}
```

## Attributes

| Attribute | Type | Description |
|-----------|------|-------------|
| `bind` | Checkbox | The Checkbox field to bind |
| `on_change` | handler | Called when toggled |

## Methods

| Method | Description |
|--------|-------------|
| `is_checked()` | Get current state |
| `set_checked(bool)` | Set checked state |
| `toggle()` | Toggle the state |
| `set_label(s)` | Set the label text |
| `set_indicators(on, off)` | Custom check/uncheck characters |

## Custom Indicators

```rust
async fn on_start(&self, _cx: &AppContext) {
    self.option.set_label("Enable feature");
    self.option.set_indicators('X', 'O');  // Custom characters
}
```

## Event Handler

```rust
page! {
    checkbox(bind: self.dark_mode, on_change: on_theme_toggle)
}

#[handler]
async fn on_theme_toggle(&self, cx: &AppContext) {
    if self.dark_mode.is_checked() {
        cx.set_theme(DarkTheme::new());
    } else {
        cx.set_theme(LightTheme::new());
    }
}
```

## Validation

Checkbox integrates with validation:

```rust
let result = Validator::new()
    .field(&self.accept_terms, "terms")
        .checked("You must accept the terms")
    .validate();
```

## Keyboard Navigation

- Tab: Move focus
- Space/Enter: Toggle checkbox

## Complete Example

```rust
#[app]
struct CheckboxDemo {
    option_a: Checkbox,
    option_b: Checkbox,
    option_c: Checkbox,
}

#[app_impl]
impl CheckboxDemo {
    async fn on_start(&self, _cx: &AppContext) {
        self.option_a.set_label("Enable notifications");
        self.option_b.set_label("Dark mode");
        self.option_c.set_label("Remember me");
        self.option_c.set_indicators('Y', 'N');
    }

    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "q" => quit,
        }
    }

    #[handler]
    async fn on_change(&self, cx: &AppContext) {
        let a = self.option_a.is_checked();
        let b = self.option_b.is_checked();
        let c = self.option_c.is_checked();
        cx.toast(format!("Options: A={}, B={}, C={}", a, b, c));
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        cx.exit();
    }

    fn page(&self) -> Node {
        page! {
            column (padding: 2, gap: 1) {
                text (bold) { "Checkbox Demo" }
                checkbox(bind: self.option_a, on_change: on_change)
                checkbox(bind: self.option_b, on_change: on_change)
                checkbox(bind: self.option_c, on_change: on_change)
                text (fg: muted) { "Space to toggle, q to quit" }
            }
        }
    }
}
```
