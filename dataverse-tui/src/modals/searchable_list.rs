//! Generic searchable list modal with fuzzy search.
//!
//! Displays a list of items with an input field for fuzzy filtering.
//! Returns the ID of the selected item.

use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Input, List, ListItem, ListState, SelectionMode, Text};
use tuidom::{Element, Size, Style};

/// An entry in the searchable list.
#[derive(Clone, Debug)]
pub struct ListEntry {
    pub id: String,
    pub label: String,
    pub category: Option<String>,
}

impl ListEntry {
    /// Create a new list entry without a category.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            category: None,
        }
    }

    /// Create a new list entry with a category.
    pub fn with_category(
        id: impl Into<String>,
        label: impl Into<String>,
        category: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            category: Some(category.into()),
        }
    }
}

/// A list item for display, with match highlighting info.
#[derive(Clone, Debug)]
struct DisplayItem {
    id: String,
    label: String,
    category: Option<String>,
    /// Indices of matched characters in the label for highlighting.
    match_indices: Vec<usize>,
}

impl DisplayItem {
    fn from_entry(entry: &ListEntry, match_indices: Vec<usize>) -> Self {
        Self {
            id: entry.id.clone(),
            label: entry.label.clone(),
            category: entry.category.clone(),
            match_indices,
        }
    }
}

impl ListItem for DisplayItem {
    type Key = String;

    fn key(&self) -> String {
        self.id.clone()
    }

    fn render(&self) -> Element {
        // Build label with highlighted match characters
        let label_element = if self.match_indices.is_empty() {
            Element::text(&self.label)
        } else {
            // Create spans with highlighted characters
            let mut spans = Vec::new();
            let chars: Vec<char> = self.label.chars().collect();
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

        // Category on the right, dimmed (if present)
        if let Some(category) = &self.category {
            let category_element =
                Element::text(category).style(Style::new().foreground(tuidom::Color::var("muted")));

            Element::row()
                .width(Size::Fill)
                .justify(tuidom::Justify::SpaceBetween)
                .child(label_element)
                .child(category_element)
        } else {
            label_element
        }
    }
}

/// Result of fuzzy matching with indices.
struct FuzzyMatch {
    /// Index of the entry in the original list.
    index: usize,
    /// Match score (higher is better).
    score: u32,
    /// Matched character indices in the label.
    indices: Vec<usize>,
}

/// Perform fuzzy filtering with match indices.
fn fuzzy_filter(query: &str, entries: &[ListEntry]) -> Vec<FuzzyMatch> {
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
            let haystack = Utf32Str::new(&entry.label, &mut buf);

            // Get score first
            let score = pattern.score(haystack, &mut matcher)?;

            // Get match indices
            let mut indices_buf = Vec::new();
            let mut buf2 = Vec::new();
            let haystack2 = Utf32Str::new(&entry.label, &mut buf2);
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

/// A generic searchable list modal with fuzzy search.
///
/// # Example
///
/// ```ignore
/// let items = vec![
///     ListEntry::with_category("entity-explorer", "Entity Explorer", "Data"),
///     ListEntry::with_category("query-builder", "Query Builder", "Tools"),
/// ];
///
/// let result = gx.modal(SearchableListModal::with_entries("Select App", items)).await;
/// if let Some(id) = result {
///     // User selected item with this ID
/// }
/// ```
#[modal(default, size = Md, aspect_ratio = 0.6)]
pub struct SearchableListModal {
    #[state(skip)]
    title: String,

    #[state(skip)]
    entries: Vec<ListEntry>,

    input: String,
    filtered: ListState<DisplayItem>,
}

impl SearchableListModal {
    /// Create a searchable list modal with entries.
    pub fn with_entries(title: impl Into<String>, entries: Vec<ListEntry>) -> Self {
        Self {
            title: title.into(),
            entries,
            ..Default::default()
        }
    }
}

#[modal_impl]
impl SearchableListModal {
    fn default_result(&self) -> Option<String> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<String>>) {
        // Initialize with all entries shown
        let items: Vec<DisplayItem> = self
            .entries
            .iter()
            .map(|e| DisplayItem::from_entry(e, Vec::new()))
            .collect();

        self.filtered
            .set(ListState::new(items).with_selection(SelectionMode::Single));

        mx.focus("searchable-list-input");
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
        let matches = fuzzy_filter(&query, &self.entries);

        let items: Vec<DisplayItem> = matches
            .into_iter()
            .map(|m| DisplayItem::from_entry(&self.entries[m.index], m.indices))
            .collect();

        self.filtered
            .set(ListState::new(items).with_selection(SelectionMode::Single));
    }

    #[handler]
    async fn on_input_submit(&self, mx: &ModalContext<Option<String>>) {
        let state = self.filtered.get();
        if let Some(first_item) = state.items.first() {
            let item_id = format!("searchable-list-item-{}", first_item.key());
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
                text (content: self.title.clone()) style (bold, fg: interact)

                input (state: self.input, id: "searchable-list-input", placeholder: "Search...")
                    on_change: on_input_change()
                    on_submit: on_input_submit()

                box_ (id: "list-container", height: fill, width: fill) style (bg: surface) {
                    list (state: self.filtered, id: "searchable-list")
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
