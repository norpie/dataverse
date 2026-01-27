//! Entity Explorer app for browsing and discovering Dataverse entities.

mod service;

use dataverse_lib::DataverseClient;
use dataverse_lib::model::Entity;
use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Input, List, ListItem, ListState, SelectionMode, Text};
use tuidom::{Element, Size, Style};

use crate::systems::client_management::{ActiveClientInfo, ClientManagement, GetActiveClient};
use crate::widgets::loading_overlay;

use service::fetch_all_entities;

/// An entity entry for display in the list.
#[derive(Clone, Debug)]
pub struct EntityItem {
    pub logical_name: String,
    pub display_name: String,
    /// Indices of matched characters in display_name for highlighting
    pub match_indices: Vec<usize>,
}

impl ListItem for EntityItem {
    type Key = String;

    fn key(&self) -> String {
        self.logical_name.clone()
    }

    fn render(&self) -> Element {
        // Build display name with highlighted match characters
        let name_element = if self.match_indices.is_empty() {
            Element::text(&self.display_name)
        } else {
            // Create spans with highlighted characters
            let mut spans = Vec::new();
            let chars: Vec<char> = self.display_name.chars().collect();
            let mut current_span = String::new();
            let mut current_is_match = false;

            for (i, ch) in chars.iter().enumerate() {
                let is_match = self.match_indices.contains(&i);

                if i == 0 {
                    current_is_match = is_match;
                    current_span.push(*ch);
                } else if is_match == current_is_match {
                    current_span.push(*ch);
                } else {
                    // Flush current span
                    if current_is_match {
                        spans.push(
                            Element::text(&current_span)
                                .style(Style::new().foreground(tuidom::Color::var("primary"))),
                        );
                    } else {
                        spans.push(Element::text(&current_span));
                    }
                    current_span = ch.to_string();
                    current_is_match = is_match;
                }
            }

            // Flush final span
            if !current_span.is_empty() {
                if current_is_match {
                    spans.push(
                        Element::text(&current_span)
                            .style(Style::new().foreground(tuidom::Color::var("primary"))),
                    );
                } else {
                    spans.push(Element::text(&current_span));
                }
            }

            Element::row().children(spans)
        };

        // Logical name on right (muted)
        let logical_element = Element::text(&self.logical_name)
            .style(Style::new().foreground(tuidom::Color::var("muted")));

        Element::row()
            .width(Size::Fill)
            .justify(tuidom::Justify::SpaceBetween)
            .child(name_element)
            .child(logical_element)
    }
}

/// Result of fuzzy matching with indices
struct FuzzyMatch {
    index: usize,
    score: u32,
    indices: Vec<usize>,
}

/// Perform fuzzy filtering with match indices (copied from launcher modal)
fn fuzzy_filter(query: &str, entities: &[(String, String)]) -> Vec<FuzzyMatch> {
    if query.is_empty() {
        return entities
            .iter()
            .enumerate()
            .map(|(index, _)| FuzzyMatch {
                index,
                score: 0,
                indices: Vec::new(),
            })
            .collect();
    }

    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::new(
        query,
        CaseMatching::Ignore,
        Normalization::Smart,
        AtomKind::Fuzzy,
    );

    let mut matches: Vec<FuzzyMatch> = entities
        .iter()
        .enumerate()
        .filter_map(|(index, (_, display))| {
            let mut buf = Vec::new();
            let haystack = Utf32Str::new(display, &mut buf);

            let score = pattern.score(haystack, &mut matcher)?;

            let mut indices_buf = Vec::new();
            let mut buf2 = Vec::new();
            let haystack2 = Utf32Str::new(display, &mut buf2);
            pattern.indices(haystack2, &mut matcher, &mut indices_buf);

            let indices: Vec<usize> = indices_buf.iter().map(|&i| i as usize).collect();

            Some(FuzzyMatch {
                index,
                score,
                indices,
            })
        })
        .collect();

    matches.sort_by(|a, b| b.score.cmp(&a.score));
    matches
}

#[app(name = "Entity Explorer")]
pub struct EntityExplorer {
    /// Full connection context.
    #[state(skip)]
    client_info: ActiveClientInfo,

    /// Loading overlay
    loading_message: Option<String>,

    /// Entity data
    all_entities: Vec<(String, String)>, // (logical, display)

    /// Search & display
    search_input: String,
    filtered_list: ListState<EntityItem>,
}

impl EntityExplorer {
    pub fn new(client_info: ActiveClientInfo) -> Self {
        Self {
            client_info,
            loading_message: State::default(),
            all_entities: State::default(),
            search_input: State::default(),
            filtered_list: State::default(),
            __handler_registry: Default::default(),
            __derived_cache: Default::default(),
        }
    }
}

#[app_impl]
impl EntityExplorer {
    #[on_start]
    async fn on_start(&self, gx: &GlobalContext, cx: &AppContext) {
        self.loading_message
            .set(Some("Loading entities...".to_string()));

        // Fetch all entities
        let result = match fetch_all_entities(&self.client_info.client).await {
            Ok(r) => r,
            Err(e) => {
                gx.toast(Toast::error(format!("Failed to load entities: {}", e)));
                self.loading_message.set(None);
                return;
            }
        };

        self.all_entities.set(result.entities.clone());

        // Initialize list with all entities (no match highlighting)
        let items: Vec<EntityItem> = result
            .entities
            .iter()
            .map(|(logical, display)| EntityItem {
                logical_name: logical.clone(),
                display_name: display.clone(),
                match_indices: Vec::new(),
            })
            .collect();

        self.filtered_list
            .set(ListState::new(items).with_selection(SelectionMode::Single));

        self.loading_message.set(None);
        cx.focus("entity-search");
    }

    fn title(&self) -> String {
        format!("Entity Explorer ({})", self.client_info.environment_name)
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", close_app);
    }

    #[handler]
    async fn close_app(&self, cx: &AppContext) {
        cx.close();
    }

    #[handler]
    async fn on_search_change(&self) {
        let query = self.search_input.get();
        let all = self.all_entities.get();

        // Use fuzzy matching (same as launcher modal)
        let matches = fuzzy_filter(&query, &all);

        let items: Vec<EntityItem> = matches
            .into_iter()
            .map(|m| {
                let (logical, display) = &all[m.index];
                EntityItem {
                    logical_name: logical.clone(),
                    display_name: display.clone(),
                    match_indices: m.indices,
                }
            })
            .collect();

        self.filtered_list
            .set(ListState::new(items).with_selection(SelectionMode::Single));
    }

    #[handler]
    async fn on_search_submit(&self, cx: &AppContext) {
        // Focus first item in list
        let state = self.filtered_list.get();
        if let Some(first_item) = state.items.first() {
            let item_id = format!("entity-list-item-{}", first_item.key());
            cx.focus(&item_id);
        }
    }

    #[handler]
    async fn on_activate(&self, gx: &GlobalContext) {
        let state = self.filtered_list.get();
        if let Some(key) = &state.last_activated {
            let query = self.client_info.client.query(Entity::logical(key));
            let _ = gx.spawn_and_focus(crate::apps::RecordExplorer::new(query, self.client_info.clone()));
        }
    }

    fn element(&self) -> Element {
        let loading_message = self.loading_message.get();
        let (item_count, total_count) = self
            .filtered_list
            .with_ref(|list| (list.items.len(), self.all_entities.get().len()));

        page! {
            box_ (width: fill, height: fill) {
                column (padding: (1, 2), gap: 1, height: fill, width: fill) style (bg: background) {
                    text (content: "Entity Explorer") style (bold, fg: interact)

                    input (
                        state: self.search_input,
                        id: "entity-search",
                        placeholder: "Search entities..."
                    )
                        on_change: on_search_change()
                        on_submit: on_search_submit()

                    box_ (id: "entity-list-container", height: fill, width: fill) style (bg: surface) {
                        list (state: self.filtered_list, id: "entity-list")
                            on_activate: on_activate()
                    }

                    row (width: fill, justify: between) {
                        text (content: {format!("{} / {}", item_count, total_count)}) style (fg: muted)

                        row (gap: 1) {
                            text (content: "esc") style (fg: primary)
                            text (content: "close") style (fg: muted)
                        }
                    }
                }

                if let Some(msg) = loading_message {
                    { loading_overlay("loading-overlay", &msg) }
                }
            }
        }
    }
}
