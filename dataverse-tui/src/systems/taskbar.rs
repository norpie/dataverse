//! Taskbar system - right-side overlay for instance management.

use std::collections::HashMap;
use std::sync::Arc;

use rafter::prelude::*;
use rafter::widgets::{Button, Text};
use rafter::{InstanceClosed, InstanceId, InstanceInfo, InstanceSpawned, Overlay};
use tuidom::{Color, Edges, Overflow, Position, Size, Style};

use crate::widgets::ScrollingText;

const COLLAPSED_WIDTH: u16 = 1;
const EXPANDED_WIDTH: u16 = 42;
const GROUP_OVERLAY_WIDTH: u16 = 30;

/// Status indicator for various subsystems.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum StatusIndicator {
    #[default]
    Idle,
    Running,
    Paused,
    Done,
    Available,
}

/// Queue status information.
#[derive(Clone, Debug)]
pub struct QueueStatus {
    pub is_running: bool,
    pub counts: crate::apps::queue::repository::StatusCounts,
}

impl Default for QueueStatus {
    fn default() -> Self {
        Self {
            is_running: false,
            counts: Default::default(),
        }
    }
}

/// Client connection information.
#[derive(Clone, Debug, Default)]
pub struct ClientStatus {
    pub host: String,
    pub auth_method: String,
    pub status: StatusIndicator,
}

/// Indexer status information.
#[derive(Clone, Debug, Default)]
pub struct IndexerStatus {
    pub status: StatusIndicator,
}

#[system]
pub struct Taskbar {
    collapsed: bool,
    instances: Vec<InstanceInfo>,
    /// Currently expanded group (app name), if any.
    expanded_group: Option<String>,
    /// Queue status.
    queue: QueueStatus,
    /// Client connection status.
    client: ClientStatus,
    /// Indexer status.
    indexer: IndexerStatus,
}

#[system_impl]
impl Taskbar {
    fn overlay(&self) -> Option<Overlay> {
        let collapsed = self.collapsed.get();
        let width = if collapsed {
            COLLAPSED_WIDTH
        } else {
            EXPANDED_WIDTH
        };

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
        bind("alt+.", cycle_next_instance);
        bind("alt+,", cycle_prev_instance);
    }

    #[handler]
    async fn toggle_collapsed(&self) {
        self.collapsed.update(|c| *c = !*c);
    }

    #[handler]
    async fn cycle_next_instance(&self, gx: &GlobalContext) {
        let instances = gx.instances();
        if instances.len() <= 1 {
            return;
        }
        let focused_id = gx.focused_instance_id();
        let current_idx = focused_id
            .and_then(|id| instances.iter().position(|i| i.id == id))
            .unwrap_or(0);
        let next_idx = (current_idx + 1) % instances.len();
        gx.focus_instance(instances[next_idx].id);
    }

    #[handler]
    async fn cycle_prev_instance(&self, gx: &GlobalContext) {
        let instances = gx.instances();
        if instances.len() <= 1 {
            return;
        }
        let focused_id = gx.focused_instance_id();
        let current_idx = focused_id
            .and_then(|id| instances.iter().position(|i| i.id == id))
            .unwrap_or(0);
        let prev_idx = if current_idx == 0 {
            instances.len() - 1
        } else {
            current_idx - 1
        };
        gx.focus_instance(instances[prev_idx].id);
    }

    fn render_status_indicator(&self, status: &StatusIndicator) -> (Element, &'static str) {
        match status {
            StatusIndicator::Idle => (
                Element::text("●").style(Style::new().foreground(Color::var("text.muted"))),
                "Idle",
            ),
            StatusIndicator::Running => (
                Element::text("●").style(Style::new().foreground(Color::var("success"))),
                "Running",
            ),
            StatusIndicator::Paused => (
                Element::text("●").style(Style::new().foreground(Color::var("warning"))),
                "Paused",
            ),
            StatusIndicator::Done => (
                Element::text("●").style(Style::new().foreground(Color::var("primary"))),
                "Done",
            ),
            StatusIndicator::Available => (
                Element::text("●").style(Style::new().foreground(Color::var("success"))),
                "Available",
            ),
        }
    }

    #[on_start]
    async fn on_start(&self) {
        // Queue status will be set when QueueReady event is received
        // (Queue app starts after systems)
        
        self.client.set(ClientStatus {
            host: "localhost:8080".to_string(),
            auth_method: "API Key".to_string(),
            status: StatusIndicator::Available,
        });
        self.indexer.set(IndexerStatus {
            status: StatusIndicator::Idle,
        });
    }

    #[event_handler]
    async fn on_queue_ready(&self, event: crate::apps::queue::api::QueueReady) {
        self.queue.set(QueueStatus {
            is_running: event.is_running,
            counts: event.counts,
        });
    }

    #[event_handler]
    async fn on_queue_status_changed(&self, event: crate::apps::queue::api::QueueStatusChanged) {
        self.queue.set(QueueStatus {
            is_running: event.is_running,
            counts: event.counts,
        });
    }

    #[event_handler]
    async fn on_instance_spawned(&self, event: InstanceSpawned, gx: &GlobalContext) {
        log::info!(
            "[Taskbar] Received InstanceSpawned event for {} (id={:?})",
            event.name,
            event.id
        );
        let instances = gx.instances();
        log::info!(
            "[Taskbar] After spawn, instances: {:?}",
            instances.iter().map(|i| i.name).collect::<Vec<_>>()
        );
        self.instances.set(instances);
    }

    #[event_handler]
    async fn on_instance_closed(&self, event: InstanceClosed, gx: &GlobalContext) {
        log::info!(
            "[Taskbar] Received InstanceClosed event for {} (id={:?})",
            event.name,
            event.id
        );
        let instances = gx.instances();
        log::info!(
            "[Taskbar] After close, instances: {:?}",
            instances.iter().map(|i| i.name).collect::<Vec<_>>()
        );
        self.instances.set(instances);
    }

    #[handler]
    async fn focus_instance(&self, id: InstanceId, gx: &GlobalContext) {
        self.expanded_group.set(None);
        gx.focus_instance(id);
    }

    #[handler]
    async fn open_group(&self, name: String) {
        self.expanded_group.set(Some(name));
    }

    #[handler]
    async fn close_group(&self) {
        self.expanded_group.set(None);
    }

    #[handler]
    async fn focus_queue(&self, gx: &GlobalContext) {
        let _ = crate::apps::Queue::get_or_spawn_and_focus(gx);
    }

    fn render_collapsed(&self) -> Element {
        use rafter::page;
        page! {
            button (id: "toggle", width: fill, height: fill, ghost) style (bg: surface)
                on_activate: toggle_collapsed()
            {
                column (height: fill, justify: center) {
                    text (content: "◀")
                }
            }
        }
    }

    fn render_expanded(&self) -> Element {
        use rafter::page;

        let instances = self.instances.get();
        let expanded_group = self.expanded_group.get();
        let focused_style = Style::new().background(Color::var("list.item_focused"));

        // Filter out Queue instance (has permanent nav in status section)
        let filtered_instances: Vec<&InstanceInfo> = instances
            .iter()
            .filter(|info| info.name != "Queue")
            .collect();

        // Group instances by app name
        let mut groups: HashMap<String, Vec<&InstanceInfo>> = HashMap::new();
        for info in &filtered_instances {
            groups.entry(info.name.to_string()).or_default().push(info);
        }

        // Sort groups by name for consistent ordering
        let mut group_names: Vec<_> = groups.keys().cloned().collect();
        group_names.sort();

        // Build list items
        let mut list_items: Vec<Element> = Vec::new();

        for group_name in group_names {
            let group_instances = &groups[&group_name];

            if group_instances.len() == 1 {
                // Single instance: render directly
                let info = group_instances[0];
                let label = format!("{} - {}", info.name, info.title);
                let btn_id = format!("instance-{}", info.id);
                let text_id = format!("instance-text-{}", info.id);
                let text_elem = ScrollingText::new()
                    .text(label)
                    .width(EXPANDED_WIDTH - 4)
                    .id(text_id)
                    .build(&Default::default(), &Default::default());

                let instance_id = info.id;
                let btn_style = focused_style.clone();

                let item = page! {
                    button (id: btn_id, width: fill, style_focused: btn_style)
                        on_activate: focus_instance(instance_id)
                    {
                        { text_elem }
                    }
                };
                list_items.push(item);
            } else {
                // Multiple instances: render group with overlay
                let group_id = format!("group-{}", group_name);
                let group_label = format!("◀ {}", group_name);

                // Check if this group is expanded
                let is_expanded = expanded_group.as_ref() == Some(&group_name);

                if is_expanded {
                    // Build overlay with instance list
                    let overlay =
                        self.render_group_overlay(&group_name, group_instances, &focused_style);

                    // Group root button wrapped with overlay as sibling
                    let group_name_clone = group_name.clone();
                    let group_name_blur = group_name.clone();
                    let btn_style = focused_style.clone();
                    let box_id = format!("{}-container", group_id);

                    // Button uses group_id so handlers match
                    let btn = page! {
                        button (id: group_id, width: fill, label: group_label, style_focused: btn_style)
                            on_focus: open_group(group_name_clone)
                            on_blur: handle_group_blur(group_name_blur)
                    };

                    let item = Element::box_()
                        .id(&box_id)
                        .width(Size::Fill)
                        .height(Size::Fixed(1))
                        .child(btn)
                        .child(overlay);

                    list_items.push(item);
                } else {
                    // Group root button only
                    let group_name_clone = group_name.clone();
                    let btn_style = focused_style.clone();
                    let item = page! {
                        button (id: group_id, width: fill, label: group_label, style_focused: btn_style)
                            on_focus: open_group(group_name_clone)
                    };
                    list_items.push(item);
                }
            }
        }

        let list_padding = Edges::new(1, 2, 1, 1);

        // Build the instances section with the list items
        let mut instances_section = Element::col()
            .width(Size::Fill)
            .child(
                Element::text("Instances")
                    .style(Style::new().bold().foreground(Color::var("interact"))),
            )
            .child(Element::col().height(Size::Fixed(1))); // Spacer after title

        for item in list_items {
            instances_section = instances_section.child(item);
        }

        // Build the status section
        let status_title =
            Element::text("Status").style(Style::new().bold().foreground(Color::var("interact")));

        // Queue subsection
        let queue = self.queue.get();
        let counts = &queue.counts;
        
        // Determine status indicator and text based on queue state
        let (queue_indicator, queue_status_text) = if !queue.is_running {
            (
                Element::text("●").style(Style::new().foreground(Color::var("warning"))),
                "Paused"
            )
        } else if counts.failed > 0 || counts.partially_failed > 0 {
            (
                Element::text("●").style(Style::new().foreground(Color::var("error"))),
                "Errors"
            )
        } else if counts.running > 0 {
            (
                Element::text("●").style(Style::new().foreground(Color::var("success"))),
                "Running"
            )
        } else if counts.ready > 0 || counts.blocked > 0 {
            (
                Element::text("●").style(Style::new().foreground(Color::var("warning"))),
                "Pending"
            )
        } else if counts.done > 0 {
            (
                Element::text("●").style(Style::new().foreground(Color::var("primary"))),
                "Done"
            )
        } else {
            (
                Element::text("●").style(Style::new().foreground(Color::var("text.muted"))),
                "Idle"
            )
        };
        
        // Calculate progress percentage
        let total = counts.total();
        let completed = counts.done + counts.failed + counts.partially_failed;
        let progress_pct = if total > 0 {
            format!("{:.1}%", (completed as f64 / total as f64) * 100.0)
        } else {
            "N/A%".to_string()
        };
        
        // Build condensed display with colored numbers: ready/running/done/failed (progress%)
        let numbers_row = Element::row()
            .child(Element::text(&counts.ready.to_string())
                .style(Style::new().foreground(Color::var("primary"))))
            .child(Element::text("/")
                .style(Style::new().foreground(Color::var("text.muted"))))
            .child(Element::text(&counts.running.to_string())
                .style(Style::new().foreground(Color::var("warning"))))
            .child(Element::text("/")
                .style(Style::new().foreground(Color::var("text.muted"))))
            .child(Element::text(&counts.done.to_string())
                .style(Style::new().foreground(Color::var("success"))))
            .child(Element::text("/")
                .style(Style::new().foreground(Color::var("text.muted"))))
            .child(Element::text(&(counts.failed + counts.partially_failed).to_string())
                .style(Style::new().foreground(Color::var("error"))))
            .child(Element::text(&format!(" ({})", progress_pct))
                .style(Style::new().foreground(Color::var("text.muted"))));

        let queue_content = Element::col()
            .width(Size::Fill)
            .gap(0)
            .child(
                Element::row()
                    .width(Size::Fill)
                    .justify(tuidom::Justify::SpaceBetween)
                    .child(Element::text("Queue"))
                    .child(
                        Element::row()
                            .gap(1)
                            .child(queue_indicator)
                            .child(Element::text(queue_status_text)),
                    )
            )
            .child(
                Element::row()
                    .width(Size::Fill)
                    .justify(tuidom::Justify::End)
                    .child(numbers_row)
            );
        
        let queue_button = page! {
            button (id: "queue-status", width: fill, ghost)
                on_activate: focus_queue()
            {
                { queue_content }
            }
        };

        // Client subsection
        let client = self.client.get();
        let (client_indicator, client_status_text) = self.render_status_indicator(&client.status);
        let client_row1 = Element::row()
            .width(Size::Fill)
            .justify(tuidom::Justify::SpaceBetween)
            .child(Element::text("Client"))
            .child(
                Element::row()
                    .gap(1)
                    .child(client_indicator)
                    .child(Element::text(client_status_text)),
            );
        let client_row2 = Element::row()
            .width(Size::Fill)
            .justify(tuidom::Justify::End)
            .child(
                Element::text(&client.host)
                    .style(Style::new().foreground(Color::var("text.muted"))),
            );
        let client_row3 = Element::row()
            .width(Size::Fill)
            .justify(tuidom::Justify::End)
            .child(
                Element::text(&client.auth_method)
                    .style(Style::new().foreground(Color::var("text.muted"))),
            );

        // Indexer subsection
        let indexer = self.indexer.get();
        let (indexer_indicator, indexer_status_text) =
            self.render_status_indicator(&indexer.status);
        let indexer_row = Element::row()
            .width(Size::Fill)
            .justify(tuidom::Justify::SpaceBetween)
            .child(Element::text("Indexer"))
            .child(
                Element::row()
                    .gap(1)
                    .child(indexer_indicator)
                    .child(Element::text(indexer_status_text)),
            );

        let status_section = Element::col()
            .width(Size::Fill)
            .gap(0)
            .child(status_title)
            .child(Element::col().height(Size::Fixed(1))) // Spacer after title
            .child(queue_button)
            .child(client_row1)
            .child(client_row2)
            .child(client_row3)
            .child(indexer_row);

        let content_col = Element::col()
            .width(Size::Fill)
            .height(Size::Fill)
            .padding(list_padding)
            .child(instances_section)
            .child(Element::col().height(Size::Fill))
            .child(status_section);

        page! {
            row (width: fill, height: fill) style (bg: surface) {
                button (id: "toggle", width: 1, height: fill, ghost)
                    on_activate: toggle_collapsed()
                {
                    column (height: fill, justify: center) {
                        text (content: "▶")
                    }
                }
                { content_col }
            }
        }
    }

    fn render_group_overlay(
        &self,
        group_name: &str,
        instances: &[&InstanceInfo],
        focused_style: &Style,
    ) -> Element {
        let group_id = format!("group-{}", group_name);

        // Build overlay items
        let mut overlay_items: Vec<Element> = Vec::new();
        for info in instances {
            let label = format!("{} - {}", info.name, info.title);
            let item_id = format!("{}-item-{}", group_id, info.id);
            let text_id = format!("{}-text-{}", group_id, info.id);

            let text_elem = ScrollingText::new()
                .text(label)
                .width(GROUP_OVERLAY_WIDTH - 2)
                .id(text_id)
                .build(&Default::default(), &Default::default());

            let instance_id = info.id;
            let btn_style = focused_style.clone();

            // Build button element manually
            let btn = Element::row()
                .id(&item_id)
                .width(Size::Fill)
                .focusable(true)
                .clickable(true)
                .style(Style::new().background(Color::var("button.normal")))
                .style_focused(btn_style)
                .child(text_elem);

            // Register handlers
            let self_clone = self.clone();
            self.__handler_registry.register(
                &item_id,
                "on_activate",
                Arc::new(move |hx| {
                    let self_inner = self_clone.clone();
                    let gx = hx.gx().clone();
                    tokio::spawn(async move {
                        self_inner.focus_instance(instance_id, &gx).await;
                    });
                }),
            );

            let self_clone = self.clone();
            let group_id_clone = group_id.clone();
            self.__handler_registry.register(
                &item_id,
                "on_blur",
                Arc::new(move |hx| {
                    let should_close = match hx.event().blur_target() {
                        Some(new_target) => !new_target.starts_with(&group_id_clone),
                        None => true,
                    };
                    if should_close {
                        self_clone.expanded_group.set(None);
                    }
                }),
            );

            overlay_items.push(btn);
        }

        let mut overlay_col = Element::col()
            .width(Size::Fixed(GROUP_OVERLAY_WIDTH))
            .style(Style::new().background(Color::var("surface")));

        for item in overlay_items {
            overlay_col = overlay_col.child(item);
        }

        // Position overlay to the left of the group root
        overlay_col
            .position(Position::Absolute)
            .left(-(GROUP_OVERLAY_WIDTH as i16 + 1))
            .top(0)
            .overflow_y(Overflow::Auto)
    }

    #[handler]
    async fn handle_group_blur(&self, group_name: String, event: &rafter::EventData) {
        let group_id = format!("group-{}", group_name);
        let should_close = match event.blur_target() {
            Some(new_target) => !new_target.starts_with(&group_id),
            None => true,
        };
        if should_close {
            self.expanded_group.set(None);
        }
    }
}
