# Modals

Modals are overlay dialogs that capture input until closed. They return typed results to the caller.

## Defining a Modal

Use `#[modal]` and `#[modal_impl]`:

```rust
#[modal]
struct ConfirmModal {
    #[state(skip)]
    message: String,
}

#[modal_impl]
impl ConfirmModal {
    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "y" | "enter" => confirm,
            "n" | "escape" => cancel,
        }
    }

    #[handler]
    async fn confirm(&self, mx: &ModalContext<bool>) {
        mx.close(true);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<bool>) {
        mx.close(false);
    }

    fn page(&self) -> Node {
        let msg = self.message.clone();
        page! {
            column (padding: 2, gap: 1, bg: surface) {
                text (bold, fg: warning) { "Confirm" }
                text { msg }
                row (gap: 2) {
                    button(id: "no", label: "No [n]", on_click: cancel)
                    button(id: "yes", label: "Yes [y]", on_click: confirm)
                }
            }
        }
    }
}
```

## Opening a Modal

Use `cx.modal()` to open a modal and await its result:

```rust
#[handler]
async fn delete_item(&self, cx: &AppContext) {
    let confirmed = cx.modal(ConfirmModal {
        message: "Delete this item?".to_string(),
    }).await;

    if confirmed {
        self.perform_delete();
        cx.toast(Toast::success("Item deleted"));
    }
}
```

## ModalContext

Handlers receive `&ModalContext<R>` where `R` is the result type:

```rust
#[handler]
async fn confirm(&self, mx: &ModalContext<bool>) {
    mx.close(true);  // Close modal and return true
}
```

The result type is inferred from how you call `mx.close()`.

## Modal Sizes

Control modal size with the `size()` method:

```rust
#[modal_impl]
impl MyModal {
    fn size(&self) -> ModalSize {
        ModalSize::Md  // 50% of screen
    }
    // ...
}
```

### Size Options

| Size | Description |
|------|-------------|
| `ModalSize::Auto` | Fit content (default) |
| `ModalSize::Sm` | 30% of screen |
| `ModalSize::Md` | 50% of screen |
| `ModalSize::Lg` | 80% of screen |
| `ModalSize::Fixed { width, height }` | Fixed cell dimensions |
| `ModalSize::Proportional { width, height }` | Fraction of screen (0.0-1.0) |

### Example

```rust
#[modal_impl]
impl LargeModal {
    fn size(&self) -> ModalSize {
        ModalSize::Lg
    }

    fn page(&self) -> Node {
        page! {
            column (width: fill, height: fill, padding: 2, bg: surface) {
                text (bold) { "Large Modal" }
                column (flex: 1) {
                    // Content fills available space
                }
                button(id: "close", label: "Close", on_click: close)
            }
        }
    }
}
```

## Modal Position

Control modal position with the `position()` method:

```rust
#[modal_impl]
impl MyModal {
    fn position(&self) -> ModalPosition {
        ModalPosition::Centered  // Default
    }
}
```

### Position Options

| Position | Description |
|----------|-------------|
| `ModalPosition::Centered` | Center of screen (default) |
| `ModalPosition::At { x, y }` | Absolute position |

## Nested Modals

Modals can open other modals:

```rust
#[modal]
struct FirstModal;

#[modal_impl]
impl FirstModal {
    #[handler]
    async fn confirm(&self, cx: &AppContext, mx: &ModalContext<bool>) {
        // Open a nested modal
        let really_sure = cx.modal(SecondModal).await;

        if really_sure {
            mx.close(true);
        }
        // If not confirmed, stay on this modal
    }
}
```

## Modal State

Modals can have reactive state like apps:

```rust
#[modal]
struct InputModal {
    value: String,  // Becomes State<String>

    #[state(skip)]
    label: String,  // Static field
}

#[modal_impl]
impl InputModal {
    #[handler]
    async fn submit(&self, mx: &ModalContext<Option<String>>) {
        let value = self.value.get();
        if !value.is_empty() {
            mx.close(Some(value));
        }
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<String>>) {
        mx.close(None);
    }

    fn page(&self) -> Node {
        let label = self.label.clone();
        page! {
            column (padding: 2, gap: 1, bg: surface) {
                text (bold) { label }
                input(bind: self.value, on_submit: submit)
                row (gap: 2) {
                    button(id: "cancel", label: "Cancel", on_click: cancel)
                    button(id: "submit", label: "Submit", on_click: submit)
                }
            }
        }
    }
}
```

## Result Types

Modals can return any type:

```rust
// Boolean result
#[handler]
async fn confirm(&self, mx: &ModalContext<bool>) {
    mx.close(true);
}

// Option result
#[handler]
async fn submit(&self, mx: &ModalContext<Option<String>>) {
    mx.close(Some(self.value.get()));
}

// Enum result
enum DialogResult {
    Save,
    Discard,
    Cancel,
}

#[handler]
async fn save(&self, mx: &ModalContext<DialogResult>) {
    mx.close(DialogResult::Save);
}
```

## AppContext in Modals

Modal handlers can access `AppContext`:

```rust
#[handler]
async fn do_something(&self, cx: &AppContext, mx: &ModalContext<()>) {
    // Show toast
    cx.toast("Processing...");

    // Access global data
    let client = cx.data::<ApiClient>();

    // Open nested modal
    let confirmed = cx.modal(ConfirmModal::default()).await;

    mx.close(());
}
```

## Next Steps

- [Multi-App](multi-app.md) - Multiple app instances
- [Communication](communication.md) - Inter-app messaging
