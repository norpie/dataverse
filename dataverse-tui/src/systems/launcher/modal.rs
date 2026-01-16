//! Launcher modal for app selection with fuzzy search.

use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Input, List, ListItem, ListState, SelectionMode, Text};
use tuidom::{Element, Size, Style};

/// A launcher entry (app with category).
#[derive(Clone, Debug)]
pub struct LauncherEntry {
    pub id: String,
    pub name: String,
    pub category: String,
}

impl LauncherEntry {
    fn new(id: &str, name: &str, category: &str) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            category: category.into(),
        }
    }
}

/// A launcher item for display in the list, with match highlighting info.
#[derive(Clone, Debug)]
pub struct LauncherItem {
    pub id: String,
    pub name: String,
    pub category: String,
    /// Indices of matched characters in the name for highlighting.
    pub match_indices: Vec<usize>,
}

impl LauncherItem {
    fn from_entry(entry: &LauncherEntry, match_indices: Vec<usize>) -> Self {
        Self {
            id: entry.id.clone(),
            name: entry.name.clone(),
            category: entry.category.clone(),
            match_indices,
        }
    }
}

impl ListItem for LauncherItem {
    type Key = String;

    fn key(&self) -> String {
        self.id.clone()
    }

    fn render(&self) -> Element {
        // Build name with highlighted match characters
        let name_element = if self.match_indices.is_empty() {
            Element::text(&self.name)
        } else {
            // Create spans with highlighted characters
            let mut spans = Vec::new();
            let chars: Vec<char> = self.name.chars().collect();
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

        // Category on the right, dimmed
        let category_element = Element::text(&self.category)
            .style(Style::new().foreground(tuidom::Color::var("muted")));

        Element::row()
            .width(Size::Fill)
            .justify(tuidom::Justify::SpaceBetween)
            .child(name_element)
            .child(category_element)
    }
}

/// Result of fuzzy matching with indices.
struct FuzzyMatch {
    /// Index of the entry in the original list.
    index: usize,
    /// Match score (higher is better).
    score: u32,
    /// Matched character indices in the name.
    indices: Vec<usize>,
}

/// Perform fuzzy filtering with match indices.
fn fuzzy_filter(query: &str, entries: &[LauncherEntry]) -> Vec<FuzzyMatch> {
    if query.is_empty() {
        // Return all entries with no highlighting
        return entries
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

    let mut matches: Vec<FuzzyMatch> = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let mut buf = Vec::new();
            let haystack = Utf32Str::new(&entry.name, &mut buf);

            // Get score first
            let score = pattern.score(haystack, &mut matcher)?;

            // Get match indices
            let mut indices_buf = Vec::new();
            let mut buf2 = Vec::new();
            let haystack2 = Utf32Str::new(&entry.name, &mut buf2);
            pattern.indices(haystack2, &mut matcher, &mut indices_buf);

            let indices: Vec<usize> = indices_buf.iter().map(|&i| i as usize).collect();

            Some(FuzzyMatch {
                index,
                score,
                indices,
            })
        })
        .collect();

    // Sort by score descending
    matches.sort_by(|a, b| b.score.cmp(&a.score));

    matches
}

fn create_launcher_entries() -> Vec<LauncherEntry> {
    vec![
        // Data
        LauncherEntry::new("record-viewer", "Record Viewer", "Data"),
        LauncherEntry::new("entity-viewer", "Entity Viewer", "Data"),
        LauncherEntry::new("collection-browser", "Collection Browser", "Data"),
        LauncherEntry::new("search", "Search", "Data"),
        LauncherEntry::new("relationships", "Relationships", "Data"),
        LauncherEntry::new("query-builder", "Query Builder", "Data"),
        // Transfer
        LauncherEntry::new("import", "Import", "Transfer"),
        LauncherEntry::new("export", "Export", "Transfer"),
        LauncherEntry::new("queue", "Queue", "Transfer"),
        LauncherEntry::new("transform", "Transform", "Transfer"),
        // System
        LauncherEntry::new("indexer", "Indexer", "System"),
        LauncherEntry::new("cache", "Cache", "System"),
        LauncherEntry::new("settings", "Settings", "System"),
        LauncherEntry::new("connections", "Connections", "System"),
        LauncherEntry::new("logs", "Logs", "System"),
        LauncherEntry::new("test", "Test", "System"),
    ]
}

#[modal(size = Sm)]
pub struct LauncherModal {
    input: String,
    entries: Vec<LauncherEntry>,
    filtered: ListState<LauncherItem>,
}

#[modal_impl]
impl LauncherModal {
    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<String>>) {
        let entries = create_launcher_entries();

        // Initialize with all entries shown
        let items: Vec<LauncherItem> = entries
            .iter()
            .map(|e| LauncherItem::from_entry(e, Vec::new()))
            .collect();

        self.entries.set(entries);
        self.filtered.set(
            ListState::new(items)
                .with_selection(SelectionMode::Single),
        );

        mx.focus("launcher-input");
    }

    #[keybinds]
    fn keys() {
        bind("escape", close);
    }

    #[handler]
    async fn close(&self, mx: &ModalContext<Option<String>>) {
        mx.close(None);
    }

    #[handler]
    async fn on_input_change(&self) {
        let query = self.input.get();
        let entries = self.entries.get();

        let matches = fuzzy_filter(&query, &entries);

        let items: Vec<LauncherItem> = matches
            .into_iter()
            .map(|m| LauncherItem::from_entry(&entries[m.index], m.indices))
            .collect();

        self.filtered.set(
            ListState::new(items)
                .with_selection(SelectionMode::Single),
        );
    }

    #[handler]
    async fn on_input_submit(&self, mx: &ModalContext<Option<String>>) {
        let state = self.filtered.get();
        if let Some(first_item) = state.items.first() {
            let item_id = format!("launcher-list-item-{}", first_item.key());
            mx.focus(&item_id);
        }
    }

    #[handler]
    async fn on_activate(&self, mx: &ModalContext<Option<String>>) {
        let state = self.filtered.get();
        if let Some(key) = &state.last_activated {
            mx.close(Some(key.clone()));
        }
    }

    fn element(&self) -> Element {
        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: background) {
                text (content: "Launcher") style (bold, fg: interact)

                input (state: self.input, id: "launcher-input", placeholder: "Search apps...")
                    on_change: on_input_change()
                    on_submit: on_input_submit()

                box_ (id: "list-container", height: fill, width: fill) style (bg: surface) {
                    list (state: self.filtered, id: "launcher-list")
                        on_activate: on_activate()
                }

                row (width: fill, justify: between) {
                    row (gap: 1) {
                        text (content: "esc") style (fg: primary)
                        text (content: "close") style (fg: muted)
                    }
                    row (gap: 1) {
                        text (content: "enter") style (fg: primary)
                        text (content: "select") style (fg: muted)
                    }
                }
            }
        }
    }
}
