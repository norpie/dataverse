use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Text;

use crate::widgets::Spinner;

#[app(name = "Welcome", singleton, on_blur = Close)]
pub struct Welcome {}

#[app_impl]
impl Welcome {
    fn title(&self) -> String {
        "Home".to_string()
    }

    fn element(&self) -> Element {
        page! {
            column (padding: 2, gap: 1) style (bg: background) {
                text (content: "Dataverse TUI") style (bold, fg: interact)
                text (content: "Press Ctrl+P to open launcher") style (fg: muted)
                text (content: "Press Ctrl+Q to quit") style (fg: muted)
                spinner (id: "main-spinner")
            }
        }
    }
}
