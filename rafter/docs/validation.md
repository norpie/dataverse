# Validation

Rafter provides a fluent validation API for form fields.

## Basic Usage

```rust
#[handler]
async fn submit(&self, cx: &AppContext) {
    let result = Validator::new()
        .field(&self.name, "name")
            .required("Name is required")
        .field(&self.email, "email")
            .required("Email is required")
            .email("Please enter a valid email")
        .validate();

    if result.is_valid() {
        cx.toast(Toast::success("Form submitted!"));
    } else {
        result.focus_first(cx);
        if let Some(err) = result.first_error() {
            cx.toast(Toast::error(&err.message));
        }
    }
}
```

## Validation Result

```rust
let result = Validator::new()
    .field(&self.name, "name")
        .required("Required")
    .validate();

// Check validity
if result.is_valid() {
    // All fields passed
}

// Get all errors
for error in result.errors() {
    println!("{}: {}", error.field_name, error.message);
}

// Get first error
if let Some(error) = result.first_error() {
    cx.toast(Toast::error(&error.message));
}

// Focus the first invalid field
result.focus_first(cx);
```

## Built-in Rules

### String Fields (Input)

```rust
Validator::new()
    .field(&self.username, "username")
        .required("Username is required")
        .min_length(3, "At least 3 characters")
        .max_length(20, "At most 20 characters")
        .pattern(r"^[a-z0-9_]+$", "Only lowercase letters, numbers, underscores")
    .field(&self.email, "email")
        .required("Email is required")
        .email("Invalid email format")
    .field(&self.password, "password")
        .required("Password is required")
        .min_length(8, "At least 8 characters")
    .field(&self.confirm, "confirm")
        .equals(self.password.value(), "Passwords must match")
    .validate()
```

| Rule | Description |
|------|-------------|
| `required(msg)` | Non-empty after trimming |
| `min_length(n, msg)` | Minimum character count |
| `max_length(n, msg)` | Maximum character count |
| `pattern(regex, msg)` | Match regex pattern |
| `email(msg)` | Valid email format |
| `equals(value, msg)` | Match another value |
| `contains(substr, msg)` | Contains substring |

### Boolean Fields (Checkbox)

```rust
Validator::new()
    .field(&self.accept_terms, "terms")
        .checked("You must accept the terms")
    .validate()
```

| Rule | Description |
|------|-------------|
| `checked(msg)` | Must be checked |
| `unchecked(msg)` | Must be unchecked |

### Selection Fields (RadioGroup)

```rust
Validator::new()
    .field(&self.priority, "priority")
        .selected("Please select a priority")
        .selected_index(0, "Must be first option")
    .validate()
```

| Rule | Description |
|------|-------------|
| `selected(msg)` | An option must be selected |
| `selected_index(i, msg)` | Specific option must be selected |

## Custom Rules

### Synchronous Rules

```rust
Validator::new()
    .field(&self.age, "age")
        .rule(|v| {
            v.parse::<u32>().map_or(false, |n| n >= 18)
        }, "Must be 18 or older")
    .validate()
```

### Async Rules

```rust
Validator::new()
    .field(&self.username, "username")
        .required("Username is required")
        .rule_async(|v| async move {
            // Check if username is available
            !api::username_exists(&v).await
        }, "Username already taken")
    .validate_async()
    .await
```

## Chaining Rules

Rules are evaluated in order. The first failing rule stops evaluation for that field:

```rust
.field(&self.email, "email")
    .required("Email is required")  // Checked first
    .email("Invalid format")         // Only if not empty
    .rule_async(|v| async move {
        !api::email_registered(&v).await
    }, "Email already registered")   // Only if format is valid
```

## Widget Error Display

Validation automatically sets error state on widgets. The Input widget displays errors:

```rust
// In on_start
self.email.set_placeholder("Enter email");

// After validation fails
// The input will display the error message
```

Clear errors by revalidating or manually:

```rust
// Errors auto-clear on next validate() call
// Or clear manually:
self.email.clear_error();
```

## Complete Form Example

```rust
#[app]
struct RegistrationForm {
    username: Input,
    email: Input,
    password: Input,
    confirm_password: Input,
    accept_terms: Checkbox,
}

#[app_impl]
impl RegistrationForm {
    async fn on_start(&self, _cx: &AppContext) {
        self.username.set_placeholder("Username");
        self.email.set_placeholder("Email");
        self.password.set_placeholder("Password");
        self.confirm_password.set_placeholder("Confirm password");
        self.accept_terms.set_label("I accept the terms and conditions");
    }

    #[handler]
    async fn submit(&self, cx: &AppContext) {
        let result = Validator::new()
            .field(&self.username, "username")
                .required("Username is required")
                .min_length(3, "At least 3 characters")
                .max_length(20, "At most 20 characters")
            .field(&self.email, "email")
                .required("Email is required")
                .email("Invalid email format")
            .field(&self.password, "password")
                .required("Password is required")
                .min_length(8, "At least 8 characters")
            .field(&self.confirm_password, "confirm")
                .required("Please confirm password")
                .equals(self.password.value(), "Passwords don't match")
            .field(&self.accept_terms, "terms")
                .checked("You must accept the terms")
            .validate();

        if result.is_valid() {
            cx.toast(Toast::success("Registration successful!"));
        } else {
            result.focus_first(cx);
        }
    }

    fn page(&self) -> Node {
        page! {
            column (padding: 2, gap: 1) {
                text (bold, fg: primary) { "Registration" }
                input(bind: self.username)
                input(bind: self.email)
                input(bind: self.password)
                input(bind: self.confirm_password)
                checkbox(bind: self.accept_terms)
                button(id: "submit", label: "Register", on_click: submit)
            }
        }
    }
}
```

## Next Steps

- [Widgets](widgets/overview.md) - Input and form widgets
- [Styling](styling.md) - Error styling
