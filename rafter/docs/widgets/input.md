# Input

A text input field for capturing user text.

## State Field

```rust
#[app]
struct MyApp {
    name: Input,
}
```

## Basic Usage

```rust
fn page(&self) -> Node {
    page! {
        input(bind: self.name)
    }
}
```

## Initialization

```rust
async fn on_start(&self, _cx: &AppContext) {
    self.name.set_placeholder("Enter your name");
    self.name.set_value("Default value");
}
```

## Attributes

| Attribute | Type | Description |
|-----------|------|-------------|
| `bind` | Input | The Input field to bind |
| `on_change` | handler | Called on every keystroke |
| `on_submit` | handler | Called when Enter is pressed |

## Reading Values

```rust
#[handler]
async fn on_submit(&self, cx: &AppContext) {
    let value = self.name.value();
    cx.toast(format!("Hello, {}!", value));
}
```

## Methods

| Method | Description |
|--------|-------------|
| `value()` | Get current text |
| `set_value(s)` | Set text content |
| `set_placeholder(s)` | Set placeholder text |
| `clear()` | Clear the input |
| `set_error(msg)` | Display validation error |
| `clear_error()` | Clear validation error |

## Event Handlers

### on_change

Called on every keystroke:

```rust
page! {
    input(bind: self.search, on_change: on_search_change)
}

#[handler]
async fn on_search_change(&self, _cx: &AppContext) {
    let query = self.search.value();
    self.filter_results(&query);
}
```

### on_submit

Called when Enter is pressed:

```rust
page! {
    input(bind: self.message, on_submit: send_message)
}

#[handler]
async fn send_message(&self, cx: &AppContext) {
    let msg = self.message.value();
    self.messages.update(|m| m.push(msg.clone()));
    self.message.clear();
}
```

## Validation

Input integrates with the validation system:

```rust
let result = Validator::new()
    .field(&self.email, "email")
        .required("Email is required")
        .email("Invalid email format")
    .validate();

// Errors are automatically displayed on the widget
```

## Keyboard Navigation

- Tab: Move to next focusable widget
- Shift+Tab: Move to previous widget
- Standard text editing keys work (arrows, backspace, delete, etc.)

## Complete Example

```rust
#[app]
struct InputDemo {
    username: Input,
    email: Input,
}

#[app_impl]
impl InputDemo {
    async fn on_start(&self, _cx: &AppContext) {
        self.username.set_placeholder("Username");
        self.email.set_placeholder("Email address");
    }

    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "ctrl+s" => submit,
            "q" => quit,
        }
    }

    #[handler]
    async fn submit(&self, cx: &AppContext) {
        let result = Validator::new()
            .field(&self.username, "username")
                .required("Username is required")
                .min_length(3, "At least 3 characters")
            .field(&self.email, "email")
                .required("Email is required")
                .email("Invalid email")
            .validate();

        if result.is_valid() {
            cx.toast(Toast::success("Form submitted!"));
        } else {
            result.focus_first(cx);
        }
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        cx.exit();
    }

    fn page(&self) -> Node {
        page! {
            column (padding: 2, gap: 1) {
                text (bold) { "Input Demo" }
                row (gap: 2) {
                    text { "Username:" }
                    input(bind: self.username)
                }
                row (gap: 2) {
                    text { "Email:   " }
                    input(bind: self.email)
                }
                text (fg: muted) { "Ctrl+S to submit, q to quit" }
            }
        }
    }
}
```
