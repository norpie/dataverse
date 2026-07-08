//! Combined entity + record-id selection modal for the Audit Log app.
//!
//! Shows a fuzzy-searchable entity list and a text field for the record GUID,
//! returning the chosen `(logical_name, id)` as an [`AuditTarget`].

use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Input, List, ListItem, ListState, SelectionMode, Text};
use tuidom::{Element, Size, Style};
use uuid::Uuid;

use super::AuditTarget;

/// An entity entry in the selection list.
#[derive(Clone, Debug)]
struct EntityItem {
    logical_name: String,
    display_name: String,
}

impl ListItem for EntityItem {
    type Key = String;

    fn key(&self) -> String {
        self.logical_name.clone()
    }

    fn render(&self) -> Element {
        Element::row()
            .width(Size::Fill)
            .justify(tuidom::Justify::SpaceBetween)
            .child(Element::text(&self.display_name))
            .child(
                Element::text(&self.logical_name)
                    .style(Style::new().foreground(tuidom::Color::var("muted"))),
            )
    }
}

/// Fuzzy-filter entities by display name, returning matching indices sorted by score.
fn fuzzy_filter(query: &str, entities: &[(String, String)]) -> Vec<usize> {
    if query.is_empty() {
        return (0..entities.len()).collect();
    }

    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::new(
        query,
        CaseMatching::Ignore,
        Normalization::Smart,
        AtomKind::Fuzzy,
    );

    let mut matches: Vec<(usize, u32)> = entities
        .iter()
        .enumerate()
        .filter_map(|(index, (_, display))| {
            let mut buf = Vec::new();
            let haystack = Utf32Str::new(display, &mut buf);
            let score = pattern.score(haystack, &mut matcher)?;
            Some((index, score))
        })
        .collect();

    matches.sort_by(|a, b| b.1.cmp(&a.1));
    matches.into_iter().map(|(index, _)| index).collect()
}

/// Combined entity + record-id selection modal.
#[modal(default, size = Md, aspect_ratio = 0.6)]
pub struct AuditSelectModal {
    #[state(skip)]
    entities: Vec<(String, String)>, // (logical, display)

    search: String,
    filtered: ListState<EntityItem>,
    selected_logical: Option<String>,
    selected_display: Option<String>,
    id_input: String,
    error: String,
}

impl AuditSelectModal {
    /// Create the modal with the available entities (logical, display).
    pub fn with_entities(entities: Vec<(String, String)>) -> Self {
        Self {
            entities,
            ..Default::default()
        }
    }

    fn build_items(&self, indices: Vec<usize>) -> Vec<EntityItem> {
        indices
            .into_iter()
            .map(|i| {
                let (logical, display) = &self.entities[i];
                EntityItem {
                    logical_name: logical.clone(),
                    display_name: display.clone(),
                }
            })
            .collect()
    }
}

#[modal_impl]
impl AuditSelectModal {
    fn default_result(&self) -> Option<AuditTarget> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<AuditTarget>>) {
        let items = self.build_items((0..self.entities.len()).collect());
        self.filtered
            .set(ListState::new(items).with_selection(SelectionMode::Single));
        mx.focus("audit-entity-search");
    }

    #[keybinds]
    fn keys() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<AuditTarget>>) {
        mx.close(None);
    }

    #[handler]
    async fn on_search_change(&self) {
        let query = self.search.get();
        let indices = fuzzy_filter(&query, &self.entities);
        let items = self.build_items(indices);
        self.filtered
            .set(ListState::new(items).with_selection(SelectionMode::Single));
    }

    #[handler]
    async fn on_search_submit(&self, mx: &ModalContext<Option<AuditTarget>>) {
        let state = self.filtered.get();
        if let Some(first) = state.items.first() {
            mx.focus(&format!("audit-entity-item-{}", first.key()));
        }
    }

    #[handler]
    async fn on_entity_activate(&self, mx: &ModalContext<Option<AuditTarget>>) {
        let state = self.filtered.get();
        if let Some(logical) = &state.last_activated {
            let display = self
                .entities
                .iter()
                .find(|(l, _)| l == logical)
                .map(|(_, d)| d.clone())
                .unwrap_or_else(|| logical.clone());
            self.selected_logical.set(Some(logical.clone()));
            self.selected_display.set(Some(display));
            self.error.set(String::new());
            mx.focus("audit-id-input");
        }
    }

    #[handler]
    async fn confirm(&self, mx: &ModalContext<Option<AuditTarget>>) {
        let logical = match self.selected_logical.get() {
            Some(l) => l,
            None => {
                self.error.set("Select an entity first".into());
                return;
            }
        };

        match Uuid::parse_str(self.id_input.get().trim()) {
            Ok(id) => mx.close(Some(AuditTarget {
                logical_name: logical,
                id,
            })),
            Err(_) => self.error.set("Invalid record ID (expected a GUID)".into()),
        }
    }

    fn element(&self) -> Element {
        let selected = self.selected_display.get();
        let error_msg = self.error.get();
        let has_error = !error_msg.is_empty();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: background) {
                text (content: "Audit Log — Select Record") style (bold, fg: interact)

                input (state: self.search, id: "audit-entity-search", placeholder: "Search entities...")
                    on_change: on_search_change()
                    on_submit: on_search_submit()

                box_ (id: "audit-entity-list-container", height: fill, width: fill) style (bg: surface) {
                    list (state: self.filtered, id: "audit-entity-list")
                        on_activate: on_entity_activate()
                }

                if let Some(display) = selected {
                    text (content: {format!("Entity: {}", display)}) style (fg: muted)
                } else {
                    text (content: "Entity: (none selected — pick one above)") style (fg: muted)
                }

                input (state: self.id_input, id: "audit-id-input", label: "Record ID", placeholder: "GUID...")
                    on_submit: confirm()

                if has_error {
                    text (content: {error_msg.clone()}) style (fg: error)
                }

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "audit-cancel") on_activate: cancel()
                    button (label: "Ok", id: "audit-ok") on_activate: confirm()
                }
            }
        }
    }
}
