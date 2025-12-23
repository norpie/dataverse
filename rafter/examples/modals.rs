//! Modal Sizes Example
//!
//! Demonstrates the modal size presets:
//! - `ModalSize::Sm` - 30% of screen
//! - `ModalSize::Md` - 50% of screen
//! - `ModalSize::Lg` - 80% of screen
//! - `ModalSize::Auto` - fits content (default)

use std::fs::File;

use log::LevelFilter;
use rafter::prelude::*;
use rafter::styling::color::Color;
use rafter::styling::theme::{DefaultTheme, Theme};
use simplelog::{Config, WriteLogger};

// ============================================================================
// Custom Theme
// ============================================================================

#[derive(Debug, Clone)]
struct ModalTheme {
    inner: DefaultTheme,
}

impl ModalTheme {
    fn new() -> Self {
        Self {
            inner: DefaultTheme {
                primary: Color::rgb(0, 200, 200),
                secondary: Color::rgb(100, 150, 255),
                background: Color::rgb(25, 25, 35),
                surface: Color::rgb(40, 40, 55),
                text: Color::rgb(230, 230, 240),
                text_muted: Color::rgb(140, 140, 160),
                error: Color::rgb(255, 100, 100),
                success: Color::rgb(100, 220, 100),
                warning: Color::rgb(255, 200, 50),
                info: Color::rgb(100, 180, 255),
                validation_error: Color::rgb(255, 100, 100),
                validation_error_border: Color::rgb(255, 100, 100),
            },
        }
    }
}

impl Theme for ModalTheme {
    fn resolve(&self, name: &str) -> Option<Color> {
        self.inner.resolve(name)
    }

    fn color_names(&self) -> Vec<&'static str> {
        self.inner.color_names()
    }

    fn clone_box(&self) -> Box<dyn Theme> {
        Box::new(self.clone())
    }
}

// ============================================================================
// Small Modal (30% of screen)
// ============================================================================

#[modal]
struct SmallModal;

#[modal_impl]
impl SmallModal {
    fn size(&self) -> ModalSize {
        ModalSize::Sm
    }

    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "escape" | "enter" => close,
        }
    }

    #[handler]
    async fn close(&self, mx: &ModalContext<()>) {
        mx.close(());
    }

    fn page(&self) -> Node {
        page! {
            column (width: fill, height: fill, padding: 2, gap: 1, bg: surface) {
                text (bold, fg: primary) { "Small Modal (30%)" }
                text (fg: muted) { "This modal takes up 30% of the screen." }
                text (fg: muted) { "Perfect for quick confirmations." }
                column (flex: 1) {} // Spacer
                button(label: "Close [Enter/Esc]", id: "close", on_click: close)
            }
        }
    }
}

// ============================================================================
// Medium Modal (50% of screen)
// ============================================================================

#[modal]
struct MediumModal;

#[modal_impl]
impl MediumModal {
    fn size(&self) -> ModalSize {
        ModalSize::Md
    }

    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "escape" | "enter" => close,
        }
    }

    #[handler]
    async fn close(&self, mx: &ModalContext<()>) {
        mx.close(());
    }

    fn page(&self) -> Node {
        page! {
            column (width: fill, height: fill, padding: 2, gap: 1, bg: surface) {
                text (bold, fg: primary) { "Medium Modal (50%)" }
                text (fg: muted) { "This modal takes up 50% of the screen." }
                text (fg: muted) { "Good for forms and detailed information." }
                text { "" }
                text { "Lorem ipsum dolor sit amet, consectetur adipiscing elit." }
                text { "Sed do eiusmod tempor incididunt ut labore et dolore." }
                column (flex: 1) {} // Spacer
                button(label: "Close [Enter/Esc]", id: "close", on_click: close)
            }
        }
    }
}

// ============================================================================
// Large Modal (80% of screen)
// ============================================================================

#[modal]
struct LargeModal;

#[modal_impl]
impl LargeModal {
    fn size(&self) -> ModalSize {
        ModalSize::Lg
    }

    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "escape" | "enter" => close,
        }
    }

    #[handler]
    async fn close(&self, mx: &ModalContext<()>) {
        mx.close(());
    }

    fn page(&self) -> Node {
        page! {
            column (width: fill, height: fill, padding: 2, gap: 1, bg: surface) {
                text (bold, fg: primary) { "Large Modal (80%)" }
                text (fg: muted) { "This modal takes up 80% of the screen." }
                text (fg: muted) { "Ideal for complex workflows and full-page overlays." }
                text { "" }
                text { "Lorem ipsum dolor sit amet, consectetur adipiscing elit." }
                text { "Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua." }
                text { "Ut enim ad minim veniam, quis nostrud exercitation ullamco." }
                text { "Duis aute irure dolor in reprehenderit in voluptate velit." }
                text { "" }
                text (fg: info) { "This size is great for:" }
                text { "  - Multi-step wizards" }
                text { "  - Settings panels" }
                text { "  - File browsers" }
                text { "  - Rich text editors" }
                column (flex: 1) {} // Spacer
                button(label: "Close [Enter/Esc]", id: "close", on_click: close)
            }
        }
    }
}

// ============================================================================
// Auto Modal (fits content)
// ============================================================================

#[modal]
struct AutoModal;

#[modal_impl]
impl AutoModal {
    // size() defaults to ModalSize::Auto

    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "escape" | "enter" => close,
        }
    }

    #[handler]
    async fn close(&self, mx: &ModalContext<()>) {
        mx.close(());
    }

    fn page(&self) -> Node {
        page! {
            column (padding: 2, gap: 1, bg: surface) {
                text (bold, fg: primary) { "Auto Modal" }
                text (fg: muted) { "Sized to fit content." }
                text { "" }
                button(label: "Close", id: "close", on_click: close)
            }
        }
    }
}

// ============================================================================
// Main App
// ============================================================================

#[app]
struct ModalApp {
    #[state(skip)]
    _placeholder: (),
}

#[app_impl]
impl ModalApp {
    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "1" => show_small,
            "2" => show_medium,
            "3" => show_large,
            "4" => show_auto,
            "q" => quit,
        }
    }

    #[handler]
    async fn show_small(&self, cx: &AppContext) {
        cx.modal(SmallModal).await;
    }

    #[handler]
    async fn show_medium(&self, cx: &AppContext) {
        cx.modal(MediumModal).await;
    }

    #[handler]
    async fn show_large(&self, cx: &AppContext) {
        cx.modal(LargeModal).await;
    }

    #[handler]
    async fn show_auto(&self, cx: &AppContext) {
        cx.modal(AutoModal).await;
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        cx.exit();
    }

    fn page(&self) -> Node {
        page! {
            column (padding: 2, gap: 2, bg: background) {
                column (gap: 1) {
                    text (bold, fg: primary) { "Modal Sizes Demo" }
                    text (fg: muted) { "Showcasing modal size presets" }
                }

                column (gap: 1, border: rounded, padding: 1) {
                    text (bold) { "Available Sizes:" }
                    text { "" }
                    row (gap: 2) {
                        text (fg: secondary) { "[1]" }
                        text { "Small (30%)" }
                    }
                    row (gap: 2) {
                        text (fg: secondary) { "[2]" }
                        text { "Medium (50%)" }
                    }
                    row (gap: 2) {
                        text (fg: secondary) { "[3]" }
                        text { "Large (80%)" }
                    }
                    row (gap: 2) {
                        text (fg: secondary) { "[4]" }
                        text { "Auto (fit content)" }
                    }
                }

                row (gap: 2) {
                    button(label: "Small", id: "small", on_click: show_small)
                    button(label: "Medium", id: "medium", on_click: show_medium)
                    button(label: "Large", id: "large", on_click: show_large)
                    button(label: "Auto", id: "auto", on_click: show_auto)
                }

                text (fg: muted) { "Press 1-4 or click buttons to open modals. Press q to quit." }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Initialize file logging
    if let Ok(log_file) = File::create("modals.log") {
        let _ = WriteLogger::init(LevelFilter::Debug, Config::default(), log_file);
    }

    if let Err(e) = rafter::Runtime::new()
        .theme(ModalTheme::new())
        .start_with::<ModalApp>()
        .await
    {
        eprintln!("Error: {}", e);
    }
}
