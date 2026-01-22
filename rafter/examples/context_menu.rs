//! Context menu example.
//!
//! Demonstrates how to create and show context menus with options, separators, and submenus.
//! Right-click anywhere in the app to show a context menu.

use rafter::EventData;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Text};

#[app]
struct ContextMenuDemo {
    counter: i32,
    last_action: String,
}

#[app_impl]
impl ContextMenuDemo {
    async fn on_start(&self) {
        self.counter.set(0);
        self.last_action
            .set("Press 'Show Menu' button to show context menu".to_string());
    }

    #[keybinds]
    fn keys() {
        bind("q", quit);
        bind("m", show_menu);
    }

    #[handler]
    async fn quit(&self, gx: &GlobalContext) {
        gx.shutdown();
    }

    #[context_menu]
    fn main_menu(&self, x: u16, y: u16) {
        context_menu! {
            option("Increment Counter", increment());
            option("Decrement Counter", decrement());
            option("Reset Counter", reset());

            separator();

            submenu("Export") {
                option("Export as JSON", export_json());
                option("Export as CSV", export_csv());
                option("Export as XML", export_xml());
            };

            separator();

            submenu("Theme") {
                option("Light Mode", set_theme_light());
                option("Dark Mode", set_theme_dark());
            };

            separator();

            option("Show Toast", show_toast(x, y));
            option("Close App", close_app());
        }
    }

    #[handler]
    async fn increment(&self) {
        let new_val = self.counter.get() + 1;
        self.counter.set(new_val);
        self.last_action.set("Incremented counter".to_string());
    }

    #[handler]
    async fn decrement(&self) {
        let new_val = self.counter.get() - 1;
        self.counter.set(new_val);
        self.last_action.set("Decremented counter".to_string());
    }

    #[handler]
    async fn reset(&self) {
        self.counter.set(0);
        self.last_action.set("Reset counter to 0".to_string());
    }

    #[handler]
    async fn export_json(&self, gx: &GlobalContext) {
        gx.toast(Toast::success("Exported as JSON"));
        self.last_action.set("Exported as JSON".to_string());
    }

    #[handler]
    async fn export_csv(&self, gx: &GlobalContext) {
        gx.toast(Toast::success("Exported as CSV"));
        self.last_action.set("Exported as CSV".to_string());
    }

    #[handler]
    async fn export_xml(&self, gx: &GlobalContext) {
        gx.toast(Toast::success("Exported as XML"));
        self.last_action.set("Exported as XML".to_string());
    }

    #[handler]
    async fn set_theme_light(&self, gx: &GlobalContext) {
        gx.toast(Toast::info("Light mode (not implemented in this example)"));
        self.last_action.set("Switched to light mode".to_string());
    }

    #[handler]
    async fn set_theme_dark(&self, gx: &GlobalContext) {
        gx.toast(Toast::info("Dark mode is the default"));
        self.last_action.set("Switched to dark mode".to_string());
    }

    #[handler]
    async fn show_toast(&self, x: u16, y: u16, gx: &GlobalContext) {
        gx.toast(Toast::info(format!("Clicked at ({}, {})", x, y)));
        self.last_action
            .set(format!("Showed toast for position ({}, {})", x, y));
    }

    #[handler]
    async fn close_app(&self, cx: &AppContext) {
        self.last_action.set("Closing app".to_string());
        cx.close();
    }

    #[handler]
    async fn show_menu(&self, cx: &AppContext, gx: &GlobalContext, event: &EventData) {
        // If activated via keyboard (Enter on button) or click, position menu below the element.
        // Otherwise (keybind), use focused element rect or mouse position as fallback.
        let (x, y) = if let Some(rect) = event.element_rect() {
            // Position menu at bottom-left of the activated element
            (rect.x, rect.y + rect.height)
        } else if let Some(rect) = gx.focused_element_rect() {
            // Fallback: use focused element rect (for keybind activation)
            (rect.x, rect.y + rect.height)
        } else {
            // Final fallback: use mouse position
            gx.mouse_position()
        };

        let menu = self.main_menu(x, y);
        cx.context_menu(menu, x, y);
    }

    fn element(&self) -> Element {
        let counter_value = self.counter.get();
        let counter_text = counter_value.to_string();
        let last_action = self.last_action.get();

        page! {
            column (width: fill, height: fill, padding: 2, gap: 2) {
                text (content: "Context Menu Example") style (bold, fg: interact)

                column (width: fill, height: fill, justify: center, align: center, gap: 2) {
                    text (content: "Press [m] to show context menu or click the button below") style (fg: muted)

                    button (label: "Show Menu [m]", id: "show-menu") on_activate: show_menu()

                    column (gap: 1, align: center) {
                        row (gap: 1) {
                            text (content: "Counter:") style (fg: secondary)
                            text (content: counter_text) style (bold, fg: interact)
                        }
                        text (content: last_action) style (fg: primary)
                    }

                    column (gap: 1, align: center) style (fg: muted) {
                        text (content: "")
                        text (content: "Menu Features:")
                        text (content: "• Basic options (Increment, Decrement, Reset)")
                        text (content: "• Separators")
                        text (content: "• Submenus (Export, Theme)")
                        text (content: "• Handler arguments (position)")
                        text (content: "• Toast notifications")
                        text (content: "")
                        text (content: "Try: Tab to button, Enter to show menu (appears below button)")
                        text (content: "     Then use arrow keys to navigate, Enter to select")
                        text (content: "")
                        text (content: "m show menu  q quit")
                    }
                }
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use simplelog::{Config, LevelFilter, WriteLogger};
    use std::fs::File;

    WriteLogger::init(
        LevelFilter::Debug,
        Config::default(),
        File::create("/tmp/context_menu.log")?,
    )?;

    let app = ContextMenuDemo::default();
    let runtime = Runtime::new()?.run(app);

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(runtime)?;

    Ok(())
}
