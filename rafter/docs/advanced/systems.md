# Systems

Systems provide global keybinds and overlays that work across all apps.

## System Keybinds

Systems define keybinds with higher priority than apps.

### Defining a System

```rust
#[system]
struct GlobalNav;

#[system_impl]
impl GlobalNav {
    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "ctrl+q" => quit,
            "ctrl+tab" => next_app,
            "ctrl+shift+tab" => prev_app,
        }
    }

    #[handler]
    async fn quit(cx: &AppContext) {
        cx.exit();
    }

    #[handler]
    async fn next_app(cx: &AppContext) {
        let instances = cx.instances();
        if instances.len() <= 1 {
            return;
        }

        let current = instances.iter().position(|i| i.is_focused);
        if let Some(idx) = current {
            let next = (idx + 1) % instances.len();
            cx.focus_instance(instances[next].id);
        }
    }

    #[handler]
    async fn prev_app(cx: &AppContext) {
        let instances = cx.instances();
        if instances.len() <= 1 {
            return;
        }

        let current = instances.iter().position(|i| i.is_focused);
        if let Some(idx) = current {
            let prev = if idx == 0 { instances.len() - 1 } else { idx - 1 };
            cx.focus_instance(instances[prev].id);
        }
    }
}
```

### System vs App Keybinds

| Priority | Source | Example |
|----------|--------|---------|
| 1 (highest) | System keybinds | Ctrl+Q to quit |
| 2 | Modal keybinds | Escape to close |
| 3 | App keybinds | Custom app shortcuts |
| 4 (lowest) | Widget keybinds | Arrow keys in list |

System keybinds always take precedence over app keybinds.

## System Overlays

Overlays render on top of all apps (like a status bar or command palette).

### Defining an Overlay

```rust
#[system_overlay]
struct StatusBar {
    message: String,
}

#[system_overlay_impl]
impl StatusBar {
    fn position(&self) -> SystemOverlayPosition {
        SystemOverlayPosition::Bottom
    }

    fn height(&self) -> u16 {
        1
    }

    fn page(&self) -> Node {
        let msg = self.message.get();
        let instances = cx.instances().len();

        page! {
            row (bg: surface, width: fill) {
                text (fg: muted) { format!("[{} apps]", instances) }
                column (flex: 1) {}
                text { msg }
            }
        }
    }
}
```

### Overlay Positions

| Position | Description |
|----------|-------------|
| `SystemOverlayPosition::Top` | Above all content |
| `SystemOverlayPosition::Bottom` | Below all content |

### Overlay with Keybinds

Overlays can have their own keybinds:

```rust
#[system_overlay]
struct CommandPalette {
    visible: bool,
    input: Input,
}

#[system_overlay_impl]
impl CommandPalette {
    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "ctrl+p" => toggle,
            "escape" => close,
        }
    }

    #[handler]
    async fn toggle(&self, _cx: &AppContext) {
        self.visible.update(|v| *v = !*v);
    }

    #[handler]
    async fn close(&self, _cx: &AppContext) {
        self.visible.set(false);
    }

    fn position(&self) -> SystemOverlayPosition {
        SystemOverlayPosition::Top
    }

    fn height(&self) -> u16 {
        if self.visible.get() { 3 } else { 0 }
    }

    fn page(&self) -> Node {
        if !self.visible.get() {
            return Node::Empty;
        }

        page! {
            column (bg: surface, border: rounded, padding: 1) {
                text (bold) { "Command Palette" }
                input(bind: self.input, on_submit: run_command)
            }
        }
    }
}
```

## Registering Systems

Systems are automatically registered via the `inventory` crate. The macros handle registration.

## Use Cases

### Global Quit

```rust
#[system]
struct QuitHandler;

#[system_impl]
impl QuitHandler {
    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "ctrl+q" => quit,
        }
    }

    #[handler]
    async fn quit(cx: &AppContext) {
        cx.exit();
    }
}
```

### App Switcher

```rust
#[system]
struct AppSwitcher;

#[system_impl]
impl AppSwitcher {
    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "alt+1" => app_1,
            "alt+2" => app_2,
            "alt+3" => app_3,
        }
    }

    #[handler]
    async fn app_1(cx: &AppContext) {
        focus_by_index(cx, 0);
    }

    #[handler]
    async fn app_2(cx: &AppContext) {
        focus_by_index(cx, 1);
    }

    #[handler]
    async fn app_3(cx: &AppContext) {
        focus_by_index(cx, 2);
    }
}

fn focus_by_index(cx: &AppContext, index: usize) {
    let instances = cx.instances();
    if let Some(info) = instances.get(index) {
        cx.focus_instance(info.id);
    }
}
```

### Status Line Overlay

```rust
#[system_overlay]
struct StatusLine;

#[system_overlay_impl]
impl StatusLine {
    fn position(&self) -> SystemOverlayPosition {
        SystemOverlayPosition::Bottom
    }

    fn height(&self) -> u16 {
        1
    }

    fn page(&self) -> Node {
        page! {
            row (bg: surface, width: fill, gap: 2) {
                text (fg: primary, bold) { "Rafter" }
                text (fg: muted) { "|" }
                text (fg: muted) { "Ctrl+Q quit" }
                text (fg: muted) { "Ctrl+Tab switch" }
            }
        }
    }
}
```

## Interaction with Apps

Systems can interact with apps via the standard mechanisms:

```rust
#[handler]
async fn toggle_sidebar(cx: &AppContext) {
    // Publish event to all apps
    cx.publish(ToggleSidebar);
}

#[handler]
async fn check_status(cx: &AppContext) {
    // Request from specific app type
    if let Ok(status) = cx.request::<MainApp, GetStatus>(GetStatus).await {
        // Use status
    }
}
```
