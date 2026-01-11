//! Taskbar system - right-side overlay for instance management.

use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::{InstanceClosed, InstanceInfo, InstanceSpawned, Overlay};

const COLLAPSED_WIDTH: u16 = 3;
const EXPANDED_WIDTH: u16 = 32;

#[system]
pub struct Taskbar {
    collapsed: bool,
    instances: Vec<InstanceInfo>,
}

#[system_impl]
impl Taskbar {
    fn overlay(&self) -> Option<Overlay> {
        let collapsed = self.collapsed.get();
        let width = if collapsed { COLLAPSED_WIDTH } else { EXPANDED_WIDTH };

        let content = if collapsed {
            self.render_collapsed()
        } else {
            self.render_expanded()
        };

        Some(Overlay::right(width, content))
    }

    #[keybinds]
    fn keys() {
        bind("alt+t", toggle_collapsed);
    }

    #[handler]
    async fn toggle_collapsed(&self) {
        self.collapsed.update(|c| *c = !*c);
    }

    #[event_handler]
    async fn on_instance_spawned(&self, _event: InstanceSpawned, gx: &GlobalContext) {
        self.instances.set(gx.instances());
    }

    #[event_handler]
    async fn on_instance_closed(&self, _event: InstanceClosed, gx: &GlobalContext) {
        self.instances.set(gx.instances());
    }

    fn render_collapsed(&self) -> Element {
        use rafter::page;
        page! {
            column (width: fill, height: fill) style (bg: surface) {
                column (height: fill) {}
                button (label: "◀", id: "toggle", ghost)
                    on_activate: toggle_collapsed()
                column (height: fill) {}
            }
        }
    }

    fn render_expanded(&self) -> Element {
        use rafter::page;
        page! {
            column (width: fill, height: fill) style (bg: surface) {
                column (height: fill) {}
                button (label: "▶", id: "toggle", ghost)
                    on_activate: toggle_collapsed()
                column (height: fill) {}
            }
        }
    }
}
