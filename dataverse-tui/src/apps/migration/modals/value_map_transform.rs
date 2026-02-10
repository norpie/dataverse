//! Modal for editing value map (option set mapping) transform configuration.

use dataverse_lib::model::OptionInfo;
use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::List;
use rafter::widgets::ListState;
use rafter::widgets::SelectState;
use rafter::widgets::Text;
use tuidom::Color;
use tuidom::Style;

use crate::apps::migration::types::OptionSetMapping;

// =============================================================================
// Mapping Display Item
// =============================================================================

/// A display item for a single option set mapping in the list.
#[derive(Debug, Clone)]
struct MappingDisplay {
    /// Source option value.
    from: i32,
    /// Target option value.
    to: i32,
    /// Source option label.
    from_label: String,
    /// Target option label.
    to_label: String,
}

impl ListItem for MappingDisplay {
    type Key = String;

    fn key(&self) -> Self::Key {
        format!("{}:{}", self.from, self.to)
    }

    fn render(&self) -> Element {
        Element::row()
            .gap(1)
            .child(
                Element::text(&format!("{} ({})", self.from_label, self.from))
                    .style(Style::new().foreground(Color::var("primary"))),
            )
            .child(Element::text("\u{2192}").style(Style::new().foreground(Color::var("muted"))))
            .child(
                Element::text(&format!("{} ({})", self.to_label, self.to))
                    .style(Style::new().foreground(Color::var("primary"))),
            )
    }
}

// =============================================================================
// Fuzzy Matching
// =============================================================================

/// Minimum fuzzy match score threshold (roughly 85% confidence).
const FUZZY_THRESHOLD: u32 = 80;

/// Find the best fuzzy match for a label in a list of candidates.
/// Returns the index of the best match if it exceeds the threshold.
fn best_fuzzy_match(query: &str, candidates: &[OptionInfo]) -> Option<usize> {
    if query.is_empty() {
        return None;
    }

    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::new(
        query,
        CaseMatching::Ignore,
        Normalization::Smart,
        AtomKind::Fuzzy,
    );

    let mut best_score = 0u32;
    let mut best_idx = None;

    for (idx, opt) in candidates.iter().enumerate() {
        let mut buf = Vec::new();
        let haystack = Utf32Str::new(&opt.label, &mut buf);
        if let Some(score) = pattern.score(haystack, &mut matcher) {
            if score > best_score {
                best_score = score;
                best_idx = Some(idx);
            }
        }
    }

    if best_score >= FUZZY_THRESHOLD {
        best_idx
    } else {
        None
    }
}

// =============================================================================
// Modal
// =============================================================================

/// Modal for editing value map transform configuration.
///
/// Shows source and target option sets with their display names,
/// and allows creating mappings between them.
#[modal(size = Lg)]
pub struct ValueMapTransformModal {
    /// Source option set options (from type tracking).
    #[state(skip)]
    source_options: Vec<OptionInfo>,
    /// Target option set options (from target field cache).
    #[state(skip)]
    target_options: Vec<OptionInfo>,
    /// Current mappings displayed in the list.
    mappings: ListState<MappingDisplay>,
    /// Select for picking a source option.
    source_select: SelectState<i32>,
    /// Select for picking a target option.
    target_select: SelectState<i32>,
    /// Status/error message.
    message: Option<String>,
}

impl ValueMapTransformModal {
    /// Create a new value map transform modal.
    pub fn new_modal(
        source_options: Vec<OptionInfo>,
        target_options: Vec<OptionInfo>,
        current_mappings: Vec<OptionSetMapping>,
    ) -> Self {
        let source_select = SelectState::new(
            source_options
                .iter()
                .map(|o| (o.value, format!("{} ({})", o.label, o.value))),
        );
        let target_select = SelectState::new(
            target_options
                .iter()
                .map(|o| (o.value, format!("{} ({})", o.label, o.value))),
        );

        let display_items: Vec<MappingDisplay> = current_mappings
            .iter()
            .map(|m| to_display(m, &source_options, &target_options))
            .collect();

        Self::new(
            source_options,
            target_options,
            ListState::new(display_items),
            source_select,
            target_select,
            None,
        )
    }
}

/// Convert an OptionSetMapping to a display item using option labels.
fn to_display(
    mapping: &OptionSetMapping,
    source_options: &[OptionInfo],
    target_options: &[OptionInfo],
) -> MappingDisplay {
    let from_label = source_options
        .iter()
        .find(|o| o.value == mapping.from)
        .map(|o| o.label.clone())
        .unwrap_or_else(|| "?".to_string());
    let to_label = target_options
        .iter()
        .find(|o| o.value == mapping.to)
        .map(|o| o.label.clone())
        .unwrap_or_else(|| "?".to_string());

    MappingDisplay {
        from: mapping.from,
        to: mapping.to,
        from_label,
        to_label,
    }
}

#[modal_impl]
impl ValueMapTransformModal {
    fn default_result(&self) -> Option<Vec<OptionSetMapping>> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<Vec<OptionSetMapping>>>) {
        if self.mappings.with_ref(|m| m.items.is_empty()) {
            mx.focus("source-select");
        } else {
            mx.focus("mappings-list");
        }
    }

    // =========================================================================
    // Derived State
    // =========================================================================

    #[derived]
    fn mapping_count(&self) -> usize {
        self.mappings.with_ref(|m| m.items.len())
    }

    #[derived]
    fn has_focused(&self) -> bool {
        self.mappings.with_ref(|m| m.focused_key.is_some())
    }

    #[derived]
    fn can_add(&self) -> bool {
        self.source_select.with_ref(|s| s.value().is_some())
            && self.target_select.with_ref(|t| t.value().is_some())
    }

    // =========================================================================
    // Keybinds
    // =========================================================================

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
        bind("ctrl+s", submit);
        bind("a", add_mapping);
        bind("d", remove_mapping);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<Vec<OptionSetMapping>>>) {
        mx.close(None);
    }

    #[handler]
    async fn submit(&self, mx: &ModalContext<Option<Vec<OptionSetMapping>>>) {
        let mappings = self.mappings.with_ref(|m| {
            m.items
                .iter()
                .map(|d| OptionSetMapping {
                    from: d.from,
                    to: d.to,
                })
                .collect()
        });
        mx.close(Some(mappings));
    }

    #[handler]
    async fn add_mapping(&self, cx: &AppContext) {
        let source_val = self.source_select.with_ref(|s| s.value().copied());
        let target_val = self.target_select.with_ref(|t| t.value().copied());

        let (Some(from), Some(to)) = (source_val, target_val) else {
            self.message
                .set(Some("Select both source and target options".to_string()));
            cx.focus("source-select");
            return;
        };

        // Check for duplicate source value
        let has_duplicate = self
            .mappings
            .with_ref(|m| m.items.iter().any(|d| d.from == from));
        if has_duplicate {
            self.message
                .set(Some(format!("Source value {} is already mapped", from)));
            return;
        }

        let display = to_display(
            &OptionSetMapping { from, to },
            &self.source_options,
            &self.target_options,
        );

        self.mappings.update(|state| {
            state.push_item(display);
        });
        self.message.set(None);

        // Clear selections for next mapping
        self.source_select.update(|s| s.selection.clear());
        self.target_select.update(|s| s.selection.clear());

        cx.focus("source-select");
    }

    #[handler]
    async fn remove_mapping(&self, cx: &AppContext) {
        let focused_key = self.mappings.with_ref(|m| m.focused_key.clone());

        if let Some(key) = focused_key {
            self.mappings.update(|state| {
                let new_items: Vec<_> = state
                    .items
                    .iter()
                    .filter(|d| d.key() != key)
                    .cloned()
                    .collect();
                state.set_items(new_items);
            });
        }

        cx.focus("mappings-list");
    }

    #[handler]
    async fn match_by_value(&self, _cx: &AppContext) {
        let mut added = 0;
        self.mappings.update(|state| {
            let existing_froms: std::collections::HashSet<i32> =
                state.items.iter().map(|d| d.from).collect();

            for source in &self.source_options {
                if existing_froms.contains(&source.value) {
                    continue;
                }
                // Find target option with same integer value
                if self.target_options.iter().any(|t| t.value == source.value) {
                    let display = to_display(
                        &OptionSetMapping {
                            from: source.value,
                            to: source.value,
                        },
                        &self.source_options,
                        &self.target_options,
                    );
                    state.push_item(display);
                    added += 1;
                }
            }
        });

        self.message.set(Some(format!(
            "Matched {} option{} by value",
            added,
            if added == 1 { "" } else { "s" }
        )));
    }

    #[handler]
    async fn match_by_name(&self, _cx: &AppContext) {
        let mut added = 0;
        self.mappings.update(|state| {
            let existing_froms: std::collections::HashSet<i32> =
                state.items.iter().map(|d| d.from).collect();

            for source in &self.source_options {
                if existing_froms.contains(&source.value) {
                    continue;
                }
                // Find best fuzzy match in target options
                if let Some(target_idx) = best_fuzzy_match(&source.label, &self.target_options) {
                    let target = &self.target_options[target_idx];
                    let display = to_display(
                        &OptionSetMapping {
                            from: source.value,
                            to: target.value,
                        },
                        &self.source_options,
                        &self.target_options,
                    );
                    state.push_item(display);
                    added += 1;
                }
            }
        });

        self.message.set(Some(format!(
            "Matched {} option{} by name",
            added,
            if added == 1 { "" } else { "s" }
        )));
    }

    // =========================================================================
    // Element
    // =========================================================================

    fn element(&self) -> Element {
        let message = self.message.get();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Value Map") style (bold, fg: interact)

                if let Some(msg) = message {
                    text (content: {msg}) style (fg: muted)
                }

                // Mappings list
                text (content: {format!("Mappings ({})", self.mapping_count())}) style (fg: muted)

                box_ (id: "mappings-container", height: fill, width: fill) style (bg: surface2) {
                    list (state: self.mappings, id: "mappings-list", width: fill, height: fill)
                }

                // Add mapping controls
                row (width: fill, gap: 1) {
                    column (width: fill) {
                        text (content: "Source") style (fg: muted)
                        select (state: self.source_select, id: "source-select", width: fill, placeholder: "Source option...")
                    }

                    column (width: fill) {
                        text (content: "Target") style (fg: muted)
                        select (state: self.target_select, id: "target-select", width: fill, placeholder: "Target option...")
                    }
                }

                // Buttons
                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn")
                        on_activate: cancel()

                    row (gap: 1) {
                        button (label: "Match Value", id: "match-value-btn")
                            on_activate: match_by_value()

                        button (label: "Match Name", id: "match-name-btn")
                            on_activate: match_by_name()

                        button (
                            label: "Remove",
                            hint: "d",
                            id: "remove-btn",
                            disabled: {!self.has_focused()}
                        )
                            on_activate: remove_mapping()

                        button (
                            label: "Add",
                            hint: "a",
                            id: "add-btn",
                            disabled: {!self.can_add()}
                        )
                            on_activate: add_mapping()

                        button (label: "Save", hint: "ctrl+s", id: "save-btn")
                            on_activate: submit()
                    }
                }
            }
        }
    }
}
