//! Indexer dashboard modal.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, List, ListItem, ListState, NumberInput, NumberInputState, SelectionMode, Text};
use tuidom::{Color, Element, Style};

use super::{
    EnvSyncStatus, IndexerSettingsChanged, IndexerStatusChanged, IndexerStatusResponse,
    PauseIndexerEvent, ResumeIndexerEvent, SyncProgress, SyncSettings, TriggerSyncEvent,
    UpdateIndexerSettingsEvent,
};
use crate::systems::taskbar::StatusIndicator;

/// Page enum for the indexer dashboard tabs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Status,
    Settings,
}

/// Environment list item for display.
#[derive(Clone, Debug)]
struct EnvListItem {
    env_id: i64,
    env_name: String,
    status: StatusIndicator,
    last_sync: Option<String>,
    error: Option<String>,
    progress: Option<SyncProgress>,
}

impl ListItem for EnvListItem {
    type Key = i64;

    fn key(&self) -> i64 {
        self.env_id
    }

    fn render(&self) -> Element {
        let (indicator, color) = status_indicator_display(&self.status);

        let mut row = Element::row().gap(1);

        // Status indicator
        row = row.child(Element::text(indicator).style(Style::new().foreground(Color::var(color))));

        // Environment name
        row = row.child(Element::text(&self.env_name));

        // Progress or last sync
        if let Some(ref progress) = self.progress {
            // Show detailed progress: "Entities: 123/500 | OptionSets: ⏳"
            let entities_text = format!("{}/{}", progress.entities_done, progress.entities_total);
            let optionsets_text = if progress.optionsets_done {
                "✓"
            } else if progress.optionsets_pending {
                "⏳"
            } else {
                "-"
            };
            let progress_text = format!(" [E:{} O:{}]", entities_text, optionsets_text);
            row = row.child(
                Element::text(progress_text).style(Style::new().foreground(Color::var("muted"))),
            );
        } else if let Some(ref last) = self.last_sync {
            row = row.child(
                Element::text(format!(" ({})", last))
                    .style(Style::new().foreground(Color::var("muted"))),
            );
        }

        // Error indicator
        if self.error.is_some() {
            row = row.child(
                Element::text(" !")
                    .style(Style::new().foreground(Color::var("error")).bold()),
            );
        }

        row
    }
}

/// Convert EnvSyncStatus to EnvListItem.
fn to_list_item(status: &EnvSyncStatus) -> EnvListItem {
    let last_sync = status.last_sync.map(|dt| {
        let now = chrono::Utc::now();
        let duration = now.signed_duration_since(dt);

        if duration.num_seconds() < 60 {
            "just now".to_string()
        } else if duration.num_minutes() < 60 {
            format!("{}m ago", duration.num_minutes())
        } else if duration.num_hours() < 24 {
            format!("{}h ago", duration.num_hours())
        } else {
            format!("{}d ago", duration.num_days())
        }
    });

    EnvListItem {
        env_id: status.env_id,
        env_name: status.env_name.clone(),
        status: status.status.clone(),
        last_sync,
        error: status.error.clone(),
        progress: status.progress.clone(),
    }
}

/// Get display info for a status indicator.
fn status_indicator_display(status: &StatusIndicator) -> (&'static str, &'static str) {
    match status {
        StatusIndicator::Idle => ("●", "muted"),
        StatusIndicator::Running => ("●", "success"),
        StatusIndicator::Paused => ("●", "warning"),
        StatusIndicator::Done => ("●", "success"),
        StatusIndicator::Available => ("●", "primary"),
        StatusIndicator::PartialError => ("●", "warning"),
        StatusIndicator::Error => ("●", "error"),
    }
}

/// Indexer dashboard modal.
#[modal(size = Lg, pages)]
pub struct IndexerDashboardModal {
    // Status state
    is_paused: bool,
    overall_status: StatusIndicator,
    env_list: ListState<EnvListItem>,

    // Settings state
    check_interval: NumberInputState,
    refresh_threshold: NumberInputState,
    settings_dirty: bool,
}

impl IndexerDashboardModal {
    pub fn new(status: IndexerStatusResponse, settings: SyncSettings) -> Self {
        let items: Vec<EnvListItem> = status.environments.iter().map(to_list_item).collect();

        Self {
            is_paused: State::new(status.is_paused),
            overall_status: State::new(status.overall_status),
            env_list: State::new(
                ListState::new(items).with_selection(SelectionMode::Single),
            ),
            check_interval: State::new(
                NumberInputState::new(settings.check_interval_secs as f64)
                    .with_min(1.0)
                    .with_max(3600.0)
                    .integer(),
            ),
            refresh_threshold: State::new(
                NumberInputState::new(settings.refresh_threshold_pct as f64)
                    .with_min(1.0)
                    .with_max(100.0)
                    .integer(),
            ),
            settings_dirty: State::new(false),
            __page: State::new(Page::default()),
            __handler_registry: rafter::HandlerRegistry::default(),
            __derived_cache: Default::default(),
        }
    }
}

#[modal_impl(kind = System, layout = layout)]
impl IndexerDashboardModal {
    fn default_result(&self) {}

    // =========================================================================
    // Keybinds
    // =========================================================================

    #[keybinds]
    fn keys() {
        bind("escape", close);
        bind("1", tab_status);
        bind("2", tab_settings);
        bind("p", toggle_pause);
        bind("s", sync_all);
    }

    #[handler]
    async fn close(&self, mx: &ModalContext<()>) {
        mx.close(());
    }

    #[handler]
    async fn tab_status(&self) {
        self.navigate(Page::Status);
    }

    #[handler]
    async fn tab_settings(&self) {
        self.navigate(Page::Settings);
    }

    // =========================================================================
    // Status handlers
    // =========================================================================

    #[handler]
    async fn toggle_pause(&self, gx: &GlobalContext) {
        if self.is_paused.get() {
            gx.publish(ResumeIndexerEvent);
        } else {
            gx.publish(PauseIndexerEvent);
        }
    }

    #[handler]
    async fn sync_all(&self, gx: &GlobalContext) {
        gx.publish(TriggerSyncEvent { env_id: None });
        gx.toast(Toast::info("Sync triggered for all environments"));
    }

    #[handler]
    async fn sync_selected(&self, gx: &GlobalContext) {
        let list_state = self.env_list.get();
        let Some(env_id) = list_state.last_activated else {
            gx.toast(Toast::warning("Select an environment first"));
            return;
        };

        gx.publish(TriggerSyncEvent {
            env_id: Some(env_id),
        });
        gx.toast(Toast::info("Sync triggered"));
    }

    // =========================================================================
    // Settings handlers
    // =========================================================================

    #[handler]
    async fn on_interval_change(&self) {
        self.settings_dirty.set(true);
    }

    #[handler]
    async fn on_threshold_change(&self) {
        self.settings_dirty.set(true);
    }

    #[handler]
    async fn save_settings(&self, gx: &GlobalContext) {
        let interval = self.check_interval.with_ref(|s| s.value().max(1.0) as u64);
        let threshold = self
            .refresh_threshold
            .with_ref(|s| s.value().clamp(1.0, 100.0) as u64);

        gx.publish(UpdateIndexerSettingsEvent {
            check_interval_secs: interval,
            refresh_threshold_pct: threshold,
        });

        self.settings_dirty.set(false);
        gx.toast(Toast::success("Settings saved"));
    }

    // =========================================================================
    // Event handlers
    // =========================================================================

    #[event_handler]
    async fn on_status_changed(&self, event: IndexerStatusChanged, _gx: &GlobalContext) {
        log::debug!("[IndexerModal] Received IndexerStatusChanged event");
        self.is_paused.set(event.is_paused);
        self.overall_status.set(event.overall_status);

        let items: Vec<EnvListItem> = event.environments.iter().map(to_list_item).collect();
        self.env_list
            .set(ListState::new(items).with_selection(SelectionMode::Single));
    }

    #[event_handler]
    async fn on_settings_changed(&self, event: IndexerSettingsChanged, _gx: &GlobalContext) {
        log::debug!("[IndexerModal] Received IndexerSettingsChanged event");
        // Only update if not dirty (user hasn't made changes)
        if !self.settings_dirty.get() {
            self.check_interval.set(
                NumberInputState::new(event.settings.check_interval_secs as f64)
                    .with_min(1.0)
                    .with_max(3600.0)
                    .integer(),
            );
            self.refresh_threshold.set(
                NumberInputState::new(event.settings.refresh_threshold_pct as f64)
                    .with_min(1.0)
                    .with_max(100.0)
                    .integer(),
            );
        }
    }

    // =========================================================================
    // Layout and Pages
    // =========================================================================

    fn layout(&self, content: Element) -> Element {
        let current = self.page();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill)
                style (bg: surface)
            {
                text (content: "Indexer Dashboard") style (bold, fg: interact)

                row (gap: 2) {
                    button (label: "Status", hint: "1", id: "tab-status")
                        style (fg: if current == Page::Status { interact } else { muted })
                        on_activate: tab_status()
                    button (label: "Settings", hint: "2", id: "tab-settings")
                        style (fg: if current == Page::Settings { interact } else { muted })
                        on_activate: tab_settings()
                }

                { content }
            }
        }
    }

    #[page(Status)]
    fn status_page(&self) -> Element {
        let is_paused = self.is_paused.get();
        let overall = self.overall_status.get();

        let (indicator, color) = status_indicator_display(&overall);
        let status_text = if is_paused {
            "Paused"
        } else {
            match overall {
                StatusIndicator::Idle => "Idle",
                StatusIndicator::Running => "Syncing",
                StatusIndicator::Error => "Error",
                StatusIndicator::PartialError => "Partial Error",
                _ => "Unknown",
            }
        };

        let pause_label = if is_paused { "Resume" } else { "Pause" };

        let status_indicator =
            Element::text(indicator).style(Style::new().foreground(Color::var(color)));

        page! {
            column (gap: 1, width: fill, height: fill, justify: between) {
                column (gap: 1, width: fill) {
                    // Overall status
                    row (gap: 1) {
                        { status_indicator }
                        text (content: status_text)
                    }

                    // Environment list
                    text (content: "Environments") style (fg: muted)
                    list (state: self.env_list, id: "env-list", height: fill)
                        style (bg: background)
                }

                // Bottom buttons: Cancel (left) / Actions (right)
                row (width: fill, justify: between) {
                    button (label: "Close", hint: "esc", id: "close")
                        on_activate: close()
                    row (gap: 1) {
                        button (label: pause_label, hint: "p", id: "toggle-pause")
                            on_activate: toggle_pause()
                        button (label: "Sync Selected", id: "sync-selected")
                            on_activate: sync_selected()
                        button (label: "Sync All", hint: "s", id: "sync-all")
                            on_activate: sync_all()
                    }
                }
            }
        }
    }

    #[page(Settings)]
    fn settings_page(&self) -> Element {
        let is_dirty = self.settings_dirty.get();

        page! {
            column (gap: 1, width: fill, height: fill, justify: between) {
                column (gap: 1, width: fill) {
                    number_input (
                        state: self.check_interval,
                        id: "check-interval",
                        label: "Check Interval (seconds)"
                    ) on_change: on_interval_change()

                    number_input (
                        state: self.refresh_threshold,
                        id: "refresh-threshold",
                        label: "Refresh Threshold (%)"
                    ) on_change: on_threshold_change()

                    // Help text
                    column (gap: 0) style (fg: muted) {
                        text (content: "Check interval: how often to check for stale cache")
                        text (content: "Refresh threshold: % of TTL elapsed before refresh")
                    }
                }

                // Bottom buttons: Cancel (left) / Save (right)
                row (width: fill, justify: between) {
                    button (label: "Close", hint: "esc", id: "close")
                        on_activate: close()
                    if is_dirty {
                        button (label: "Save", id: "save-settings")
                            on_activate: save_settings()
                    }
                }
            }
        }
    }
}
