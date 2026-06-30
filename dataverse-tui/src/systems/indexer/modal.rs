//! Indexer dashboard modal.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{
    Button, List, ListItem, ListState, NumberInput, NumberInputState, SelectionMode, Text,
};
use tuidom::{Color, Element, Style};

use super::{
    CacheCategory, ClearCacheCategoryEvent, EnvSyncStatus, IndexerSettingsChanged,
    IndexerStatusChanged, IndexerStatusResponse, PauseIndexerEvent, ResumeIndexerEvent,
    SyncProgress, SyncSettings, TriggerSyncEvent, UpdateIndexerSettingsEvent,
};
use crate::modals::ConfirmModal;
use crate::systems::client_management::SessionChanged;
use crate::systems::taskbar::StatusIndicator;

/// Page enum for the indexer dashboard tabs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Status,
    Settings,
    Cache,
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
                Element::text(" !").style(Style::new().foreground(Color::var("error")).bold()),
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

/// Create an integer hour input for cache TTL settings.
fn ttl_hours_input(hours: u64) -> NumberInputState {
    NumberInputState::new(hours.max(1) as f64)
        .with_min(1.0)
        .with_max(8760.0)
        .integer()
}

/// Read a cache TTL input as a bounded integer hour value.
fn ttl_hours_value(state: &NumberInputState) -> u64 {
    state.value().clamp(1.0, 8760.0) as u64
}

/// Get display info for a status indicator.
fn status_indicator_display(status: &StatusIndicator) -> (&'static str, &'static str) {
    match status {
        StatusIndicator::Idle => ("●", "muted"),
        StatusIndicator::Running => ("●", "warning"),
        StatusIndicator::Paused => ("●", "warning"),
        StatusIndicator::Done => ("●", "success"),
        StatusIndicator::Available => ("●", "success"),
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
    cache_entity_list_ttl: NumberInputState,
    cache_entity_metadata_ttl: NumberInputState,
    cache_attribute_metadata_ttl: NumberInputState,
    cache_global_optionset_ttl: NumberInputState,
    cache_relationship_ttl: NumberInputState,
    cache_query_ttl: NumberInputState,
    settings_dirty: bool,

    // Cache state
    active_env_name: Option<String>,
}

impl IndexerDashboardModal {
    pub fn with_status(
        status: IndexerStatusResponse,
        settings: SyncSettings,
        active_env_name: Option<String>,
    ) -> Self {
        let items: Vec<EnvListItem> = status.environments.iter().map(to_list_item).collect();

        Self {
            is_paused: State::new(status.is_paused),
            overall_status: State::new(status.overall_status),
            env_list: State::new(ListState::new(items).with_selection(SelectionMode::Single)),
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
            cache_entity_list_ttl: State::new(ttl_hours_input(
                settings.cache_entity_list_ttl_hours,
            )),
            cache_entity_metadata_ttl: State::new(ttl_hours_input(
                settings.cache_entity_metadata_ttl_hours,
            )),
            cache_attribute_metadata_ttl: State::new(ttl_hours_input(
                settings.cache_attribute_metadata_ttl_hours,
            )),
            cache_global_optionset_ttl: State::new(ttl_hours_input(
                settings.cache_global_optionset_ttl_hours,
            )),
            cache_relationship_ttl: State::new(ttl_hours_input(
                settings.cache_relationship_ttl_hours,
            )),
            cache_query_ttl: State::new(ttl_hours_input(settings.cache_query_ttl_hours)),
            settings_dirty: State::new(false),
            active_env_name: State::new(active_env_name),
            __page: State::new(Page::default()),
            __handler_registry: rafter::HandlerRegistry::default(),
            __derived_cache: Default::default(),
            __watch_state: Default::default(),
        }
    }
}

#[modal_impl(kind = System, layout = layout)]
impl IndexerDashboardModal {
    fn default_result(&self) {}

    // =========================================================================
    // Derived State
    // =========================================================================

    /// Get the current status text based on pause state and overall status.
    #[derived]
    fn status_text(&self) -> &'static str {
        if self.is_paused.get() {
            "Paused"
        } else {
            match self.overall_status.get() {
                StatusIndicator::Idle => "Idle",
                StatusIndicator::Running => "Syncing",
                StatusIndicator::Error => "Error",
                StatusIndicator::PartialError => "Partial Error",
                _ => "Unknown",
            }
        }
    }

    /// Get the pause/resume button label.
    #[derived]
    fn pause_button_label(&self) -> &'static str {
        if self.is_paused.get() {
            "Resume"
        } else {
            "Pause"
        }
    }

    // =========================================================================
    // Keybinds
    // =========================================================================

    #[keybinds]
    fn keys() {
        bind("escape", close);
        bind("1", tab_status);
        bind("2", tab_settings);
        bind("3", tab_cache);
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

    #[handler]
    async fn tab_cache(&self) {
        self.navigate(Page::Cache);
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
    async fn on_cache_ttl_change(&self) {
        self.settings_dirty.set(true);
    }

    #[handler]
    async fn save_settings(&self, gx: &GlobalContext) {
        let interval = self.check_interval.with_ref(|s| s.value().max(1.0) as u64);
        let threshold = self
            .refresh_threshold
            .with_ref(|s| s.value().clamp(1.0, 100.0) as u64);
        let cache_entity_list_ttl_hours = self.cache_entity_list_ttl.with_ref(ttl_hours_value);
        let cache_entity_metadata_ttl_hours =
            self.cache_entity_metadata_ttl.with_ref(ttl_hours_value);
        let cache_attribute_metadata_ttl_hours =
            self.cache_attribute_metadata_ttl.with_ref(ttl_hours_value);
        let cache_global_optionset_ttl_hours =
            self.cache_global_optionset_ttl.with_ref(ttl_hours_value);
        let cache_relationship_ttl_hours = self.cache_relationship_ttl.with_ref(ttl_hours_value);
        let cache_query_ttl_hours = self.cache_query_ttl.with_ref(ttl_hours_value);

        gx.publish(UpdateIndexerSettingsEvent {
            check_interval_secs: interval,
            refresh_threshold_pct: threshold,
            cache_entity_list_ttl_hours,
            cache_entity_metadata_ttl_hours,
            cache_attribute_metadata_ttl_hours,
            cache_global_optionset_ttl_hours,
            cache_relationship_ttl_hours,
            cache_query_ttl_hours,
        });

        self.settings_dirty.set(false);
        gx.toast(Toast::success("Settings saved"));
    }

    // =========================================================================
    // Cache handlers
    // =========================================================================

    // -- Active environment clear handlers --

    #[handler]
    async fn clear_entities(&self, gx: &GlobalContext) {
        self.confirm_and_clear(gx, CacheCategory::Entities, false)
            .await;
    }

    #[handler]
    async fn clear_attributes(&self, gx: &GlobalContext) {
        self.confirm_and_clear(gx, CacheCategory::Attributes, false)
            .await;
    }

    #[handler]
    async fn clear_relationships(&self, gx: &GlobalContext) {
        self.confirm_and_clear(gx, CacheCategory::Relationships, false)
            .await;
    }

    #[handler]
    async fn clear_global_option_sets(&self, gx: &GlobalContext) {
        self.confirm_and_clear(gx, CacheCategory::GlobalOptionSets, false)
            .await;
    }

    #[handler]
    async fn clear_queries(&self, gx: &GlobalContext) {
        self.confirm_and_clear(gx, CacheCategory::Queries, false)
            .await;
    }

    #[handler]
    async fn clear_all(&self, gx: &GlobalContext) {
        self.confirm_and_clear(gx, CacheCategory::All, false).await;
    }

    // -- All environments clear handlers --

    #[handler]
    async fn clear_entities_all(&self, gx: &GlobalContext) {
        self.confirm_and_clear(gx, CacheCategory::Entities, true)
            .await;
    }

    #[handler]
    async fn clear_attributes_all(&self, gx: &GlobalContext) {
        self.confirm_and_clear(gx, CacheCategory::Attributes, true)
            .await;
    }

    #[handler]
    async fn clear_relationships_all(&self, gx: &GlobalContext) {
        self.confirm_and_clear(gx, CacheCategory::Relationships, true)
            .await;
    }

    #[handler]
    async fn clear_global_option_sets_all(&self, gx: &GlobalContext) {
        self.confirm_and_clear(gx, CacheCategory::GlobalOptionSets, true)
            .await;
    }

    #[handler]
    async fn clear_queries_all(&self, gx: &GlobalContext) {
        self.confirm_and_clear(gx, CacheCategory::Queries, true)
            .await;
    }

    #[handler]
    async fn clear_all_all(&self, gx: &GlobalContext) {
        self.confirm_and_clear(gx, CacheCategory::All, true).await;
    }

    /// Show a confirmation modal and, if confirmed, publish the clear event.
    async fn confirm_and_clear(
        &self,
        gx: &GlobalContext,
        category: CacheCategory,
        all_environments: bool,
    ) {
        let message = if all_environments {
            format!("Clear {} cache for all environments?", category)
        } else {
            let env_name = self
                .active_env_name
                .get()
                .unwrap_or_else(|| "active environment".to_string());
            format!("Clear {} cache for {}?", category, env_name)
        };

        let confirmed = gx.modal(ConfirmModal::with_message(message)).await;

        if confirmed {
            gx.publish(ClearCacheCategoryEvent {
                category,
                all_environments,
            });
        }
    }

    // =========================================================================
    // Event handlers
    // =========================================================================

    #[event_handler]
    async fn on_session_changed(&self, event: SessionChanged, _gx: &GlobalContext) {
        log::debug!("[IndexerModal] Received SessionChanged event");
        self.active_env_name.set(event.environment_name);
    }

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
            self.cache_entity_list_ttl
                .set(ttl_hours_input(event.settings.cache_entity_list_ttl_hours));
            self.cache_entity_metadata_ttl.set(ttl_hours_input(
                event.settings.cache_entity_metadata_ttl_hours,
            ));
            self.cache_attribute_metadata_ttl.set(ttl_hours_input(
                event.settings.cache_attribute_metadata_ttl_hours,
            ));
            self.cache_global_optionset_ttl.set(ttl_hours_input(
                event.settings.cache_global_optionset_ttl_hours,
            ));
            self.cache_relationship_ttl
                .set(ttl_hours_input(event.settings.cache_relationship_ttl_hours));
            self.cache_query_ttl
                .set(ttl_hours_input(event.settings.cache_query_ttl_hours));
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
                    button (label: "Cache", hint: "3", id: "tab-cache")
                        style (fg: if current == Page::Cache { interact } else { muted })
                        on_activate: tab_cache()
                }

                { content }
            }
        }
    }

    #[page(Status)]
    fn status_page(&self) -> Element {
        let overall = self.overall_status.get();
        let (indicator, color) = status_indicator_display(&overall);
        let status_text = self.status_text();
        let pause_label = self.pause_button_label();

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
                    text (content: "Indexer") style (fg: muted)

                    row (gap: 1, width: fill) {
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
                    }

                    text (content: "Cache TTLs (hours)") style (fg: muted)

                    row (gap: 1, width: fill) {
                        column (gap: 1, width: fill) {
                            number_input (
                                state: self.cache_entity_list_ttl,
                                id: "cache-entity-list-ttl",
                                label: "Entity List"
                            ) on_change: on_cache_ttl_change()

                            number_input (
                                state: self.cache_entity_metadata_ttl,
                                id: "cache-entity-metadata-ttl",
                                label: "Entity Metadata"
                            ) on_change: on_cache_ttl_change()

                            number_input (
                                state: self.cache_attribute_metadata_ttl,
                                id: "cache-attribute-metadata-ttl",
                                label: "Attribute Metadata"
                            ) on_change: on_cache_ttl_change()
                        }

                        column (gap: 1, width: fill) {
                            number_input (
                                state: self.cache_global_optionset_ttl,
                                id: "cache-global-optionset-ttl",
                                label: "Global Option Sets"
                            ) on_change: on_cache_ttl_change()

                            number_input (
                                state: self.cache_relationship_ttl,
                                id: "cache-relationship-ttl",
                                label: "Relationships"
                            ) on_change: on_cache_ttl_change()

                            number_input (
                                state: self.cache_query_ttl,
                                id: "cache-query-ttl",
                                label: "Queries"
                            ) on_change: on_cache_ttl_change()
                        }
                    }

                    // Help text
                    column (gap: 0) style (fg: muted) {
                        text (content: "Check interval: how often to check for stale cache")
                        text (content: "Refresh threshold: % of TTL elapsed before refresh")
                        text (content: "Cache TTLs apply when new Dataverse clients are created")
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

    #[page(Cache)]
    fn cache_page(&self) -> Element {
        let env_name = self.active_env_name.get();
        let has_env = env_name.is_some();
        let env_label = env_name.unwrap_or_else(|| "No active environment".to_string());

        page! {
            column (gap: 1, width: fill, height: fill, justify: between) {
                column (gap: 1, width: fill) {
                    // Active environment
                    row (gap: 1) {
                        text (content: "Environment:") style (fg: muted)
                        text (content: env_label)
                            style (fg: if has_env { primary } else { error })
                    }

                    if has_env {
                        // Category headers
                        row (gap: 1, width: fill) {
                            text (content: "Clear by category") style (fg: muted)
                        }

                        // Category rows: Active + All Envs buttons
                        column (gap: 0, width: fill) {
                            row (gap: 1, width: fill) {
                                button (label: "Entities", id: "clear-entities")
                                    on_activate: clear_entities()
                                button (label: "All Envs", id: "clear-entities-all")
                                    style (fg: muted)
                                    on_activate: clear_entities_all()
                            }
                            row (gap: 1, width: fill) {
                                button (label: "Attributes", id: "clear-attributes")
                                    on_activate: clear_attributes()
                                button (label: "All Envs", id: "clear-attributes-all")
                                    style (fg: muted)
                                    on_activate: clear_attributes_all()
                            }
                            row (gap: 1, width: fill) {
                                button (label: "Relationships", id: "clear-relationships")
                                    on_activate: clear_relationships()
                                button (label: "All Envs", id: "clear-relationships-all")
                                    style (fg: muted)
                                    on_activate: clear_relationships_all()
                            }
                            row (gap: 1, width: fill) {
                                button (label: "Global Option Sets", id: "clear-global-option-sets")
                                    on_activate: clear_global_option_sets()
                                button (label: "All Envs", id: "clear-global-option-sets-all")
                                    style (fg: muted)
                                    on_activate: clear_global_option_sets_all()
                            }
                            row (gap: 1, width: fill) {
                                button (label: "Queries", id: "clear-queries")
                                    on_activate: clear_queries()
                                button (label: "All Envs", id: "clear-queries-all")
                                    style (fg: muted)
                                    on_activate: clear_queries_all()
                            }
                        }

                        // Clear everything
                        text (content: "Clear everything") style (fg: muted)
                        row (gap: 1, width: fill) {
                            button (label: "Clear All", id: "clear-all")
                                style (fg: error)
                                on_activate: clear_all()
                            button (label: "All Envs", id: "clear-all-all")
                                style (fg: error)
                                on_activate: clear_all_all()
                        }
                    }
                }

                // Bottom button
                row (width: fill, justify: between) {
                    button (label: "Close", hint: "esc", id: "close")
                        on_activate: close()
                }
            }
        }
    }
}
