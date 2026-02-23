//! Modal for building filter conditions.

use dataverse_lib::DataverseClient;
use dataverse_lib::model::metadata::EntityMetadata;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Autocomplete;
use rafter::widgets::AutocompleteState;
use rafter::widgets::Button;
use rafter::widgets::Input;
use rafter::widgets::Select;
use rafter::widgets::SelectState;
use rafter::widgets::Text;
use rafter::widgets::Tree;
use rafter::widgets::TreeState;

use crate::formatting::parse_filter_value;
use crate::modals::LoadingModal;
use crate::widgets::filter_builder::ConditionData;
use crate::widgets::filter_builder::ConditionEditorModal;
use crate::widgets::filter_builder::FilterNode;
use crate::widgets::filter_builder::FilterTreeItem;
use crate::widgets::filter_builder::FilterTreeKey;
use crate::widgets::filter_builder::build_tree;
use crate::widgets::filter_builder::CondOp;

/// Modal for building filter conditions.
#[modal(size = Md)]
pub struct FilterBuilderModal {
    #[state(skip)]
    title: String,
    #[state(skip)]
    client: DataverseClient,
    #[state(skip)]
    entity_name: String,
    metadata: Option<EntityMetadata>,

    filter: FilterNode,
    tree_state: TreeState<FilterTreeItem>,
    next_id: usize,
}

impl FilterBuilderModal {
    /// Create a new filter builder modal.
    pub fn new_modal(
        title: impl Into<String>,
        client: DataverseClient,
        entity_name: impl Into<String>,
        initial: Option<FilterNode>,
    ) -> Self {
        let filter = initial.unwrap_or_default();
        let next_id = find_max_id(&filter) + 1;

        Self::new(
            title.into(),
            client,
            entity_name.into(),
            None,
            filter,
            TreeState::default(),
            next_id,
        )
    }
}

#[modal_impl]
impl FilterBuilderModal {
    fn default_result(&self) -> Option<FilterNode> {
        None
    }

    #[on_start]
    async fn on_start(&self, gx: &GlobalContext, mx: &ModalContext<Option<FilterNode>>) {
        // Fetch entity metadata (includes typed attributes with option sets)
        let client = self.client.clone();
        let entity_name = self.entity_name.clone();
        let result = gx
            .modal(LoadingModal::run_with_default(
                "Loading entity metadata...",
                || Err(dataverse_lib::error::Error::Cancelled),
                async move { client.metadata().entity(entity_name).await },
            ))
            .await;

        match result {
            Ok(metadata) => {
                self.metadata.set(Some(metadata));
            }
            Err(e) if e.is_cancelled() => {
                mx.close(None);
                return;
            }
            Err(e) => {
                log::error!("Failed to fetch entity metadata: {}", e);
                gx.toast(Toast::error("Failed to fetch entity metadata"));
                mx.close(None);
                return;
            }
        }

        self.rebuild_tree();
        mx.focus("filter-tree");
    }

    /// Build autocomplete options from metadata attributes.
    fn field_options(&self) -> Vec<(String, String)> {
        let metadata = self.metadata.get();
        let Some(metadata) = metadata else {
            return Vec::new();
        };
        metadata
            .attributes
            .iter()
            .map(|a| {
                let display_name = a.display_name.text_or(&a.logical_name);
                let display = format!("{} ({})", display_name, a.logical_name);
                (a.logical_name.clone(), display)
            })
            .collect()
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
        bind("a", add_condition);
        bind("p", paste_conditions);
        bind("g", add_group);
        bind("n", toggle_not);
        bind("d", delete_item);
        bind("J", move_down);
        bind("K", move_up);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<FilterNode>>) {
        mx.close(None);
    }

    #[handler]
    async fn submit(&self, mx: &ModalContext<Option<FilterNode>>) {
        mx.close(Some(self.filter.get()));
    }

    #[handler]
    async fn add_condition(&self, gx: &GlobalContext) {
        let metadata = self.metadata.get();
        let Some(metadata) = metadata else {
            return;
        };

        let result = gx
            .modal(ConditionEditorModal::with_options(
                self.field_options(),
                metadata.clone(),
            ))
            .await;

        let Some(condition) = result else {
            return;
        };

        let id = self.next_id.get();
        self.next_id.set(id + 1);

        let node = FilterNode::Condition {
            id,
            field: condition.field,
            operator: condition.operator,
            value: condition.value,
        };

        self.add_node_to_filter(node);
        self.rebuild_tree();
    }

    #[handler]
    async fn paste_conditions(&self, gx: &GlobalContext) {
        let metadata = self.metadata.get();
        let Some(metadata) = metadata else {
            return;
        };

        let result = gx
            .modal(PasteConditionsModal::new_modal(
                self.field_options(),
                metadata.clone(),
            ))
            .await;

        let Some(conditions) = result else {
            return;
        };

        if conditions.is_empty() {
            return;
        }

        // Add each condition directly to the current group
        for cond in conditions {
            let id = self.next_id.get();
            self.next_id.set(id + 1);

            let node = FilterNode::Condition {
                id,
                field: cond.field,
                operator: cond.operator,
                value: cond.value,
            };

            self.add_node_to_filter(node);
        }

        self.rebuild_tree();
    }

    #[handler]
    async fn add_group(&self) {
        let id = self.next_id.get();
        self.next_id.set(id + 1);

        let node = FilterNode::Group {
            id,
            is_and: true,
            is_negated: false,
            children: Vec::new(),
        };

        self.add_node_to_filter(node);
        self.rebuild_tree();
    }

    #[handler]
    async fn delete_item(&self) {
        let focused = self.tree_state.with_ref(|s| s.focused_key.clone());
        let Some(key) = focused else {
            return;
        };

        let id = match key {
            FilterTreeKey::Group(id) => id,
            FilterTreeKey::Condition(id) => id,
        };

        self.filter.update(|f| {
            f.remove_node(id);
        });
        self.rebuild_tree();
    }

    #[handler]
    async fn move_up(&self) {
        let focused = self.tree_state.with_ref(|s| s.focused_key.clone());
        let Some(key) = focused else {
            return;
        };

        let id = match key {
            FilterTreeKey::Group(id) => id,
            FilterTreeKey::Condition(id) => id,
        };

        self.filter.update(|f| {
            f.move_node_up(id);
        });
        self.rebuild_tree();
    }

    #[handler]
    async fn move_down(&self) {
        let focused = self.tree_state.with_ref(|s| s.focused_key.clone());
        let Some(key) = focused else {
            return;
        };

        let id = match key {
            FilterTreeKey::Group(id) => id,
            FilterTreeKey::Condition(id) => id,
        };

        self.filter.update(|f| {
            f.move_node_down(id);
        });
        self.rebuild_tree();
    }

    #[handler]
    async fn toggle_not(&self) {
        let focused = self.tree_state.with_ref(|s| s.focused_key.clone());
        let Some(FilterTreeKey::Group(id)) = focused else {
            return;
        };

        self.filter.update(|f| {
            f.toggle_negation(id);
        });
        self.rebuild_tree();
    }

    #[handler]
    async fn on_item_activate(&self, gx: &GlobalContext) {
        let focused = self.tree_state.with_ref(|s| s.focused_key.clone());
        let Some(key) = focused else {
            return;
        };

        match key {
            FilterTreeKey::Group(id) => {
                // Toggle AND/OR
                self.filter.update(|f| {
                    f.toggle_group(id);
                });
                self.rebuild_tree();
            }
            FilterTreeKey::Condition(id) => {
                let metadata = self.metadata.get();
                let Some(metadata) = metadata else {
                    return;
                };

                // Edit condition
                let filter = self.filter.get();
                let Some((field, operator, value)) = filter.find_condition(id) else {
                    return;
                };

                let condition = ConditionData {
                    field,
                    operator,
                    value,
                };

                let result = gx
                    .modal(ConditionEditorModal::with_condition(
                        self.field_options(),
                        metadata.clone(),
                        condition,
                    ))
                    .await;

                let Some(updated) = result else {
                    return;
                };

                self.filter.update(|f| {
                    f.update_condition(id, updated.field, updated.operator, updated.value);
                });
                self.rebuild_tree();
            }
        }
    }

    fn add_node_to_filter(&self, node: FilterNode) {
        let focused = self.tree_state.with_ref(|s| s.focused_key.clone());

        self.filter.update(|f| {
            if let FilterNode::Empty = f {
                let root_id = self.next_id.get();
                self.next_id.set(root_id + 1);
                *f = FilterNode::Group {
                    id: root_id,
                    is_and: true,
                    is_negated: false,
                    children: vec![node],
                };
                return;
            }

            if let FilterNode::Group { id: root_id, .. } = f {
                let root_id = *root_id;
                let target_id = match &focused {
                    Some(FilterTreeKey::Group(id)) => *id,
                    Some(FilterTreeKey::Condition(id)) => {
                        f.find_parent_group_id(*id).unwrap_or(root_id)
                    }
                    None => root_id,
                };
                f.add_to_group(target_id, node);
            }
        });
    }

    fn rebuild_tree(&self) {
        let filter = self.filter.get();
        let nodes = build_tree(&filter);
        self.tree_state.update(|s| {
            s.set_roots(nodes);
            s.expand_all();
        });
    }

    #[derived]
    fn is_empty(&self) -> bool {
        self.filter.with_ref(|f| {
            if let FilterNode::Empty = f {
                true
            } else {
                false
            }
        })
    }

    fn element(&self) -> Element {
        let title = &self.title;
        let is_empty = self.is_empty();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                row (width: fill, justify: between) {
                    text (content: {title}) style (bold, fg: interact)
                    row (gap: 2) {
                        row (gap: 1) {
                            text (content: "a") style (fg: primary)
                            text (content: "add") style (fg: muted)
                        }
                        row (gap: 1) {
                            text (content: "p") style (fg: primary)
                            text (content: "paste") style (fg: muted)
                        }
                        row (gap: 1) {
                            text (content: "g") style (fg: primary)
                            text (content: "group") style (fg: muted)
                        }
                        row (gap: 1) {
                            text (content: "n") style (fg: primary)
                            text (content: "not") style (fg: muted)
                        }
                        row (gap: 1) {
                            text (content: "d") style (fg: primary)
                            text (content: "delete") style (fg: muted)
                        }
                        row (gap: 1) {
                            text (content: "J/K") style (fg: primary)
                            text (content: "move") style (fg: muted)
                        }
                    }
                }

                if is_empty {
                    column (width: fill, height: fill, justify: center, align: center) style (bg: surface2) {
                        text (content: "No conditions defined.") style (fg: muted)
                        text (content: "Press 'a' to add a condition.") style (fg: muted)
                    }
                } else {
                    tree (state: self.tree_state, id: "filter-tree") style (height: fill, bg: surface2)
                        on_activate: on_item_activate()
                }

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn")
                        on_activate: cancel()

                    button (label: "Save", id: "save-btn")
                        on_activate: submit()
                }
            }
        }
    }
}

/// Find the maximum ID used in a filter tree.
fn find_max_id(node: &FilterNode) -> usize {
    match node {
        FilterNode::Empty => 0,
        FilterNode::Condition { id, .. } => *id,
        FilterNode::Group { id, children, .. } => {
            let child_max = children.iter().map(find_max_id).max().unwrap_or(0);
            (*id).max(child_max)
        }
    }
}

// =============================================================================
// Paste Conditions Modal
// =============================================================================

/// Parse a string containing values separated by commas, newlines, or spaces.
/// Returns a list of trimmed, non-empty, deduplicated values.
fn parse_pasted_values(input: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for part in input.split(|c: char| c == ',' || c == '\n' || c == '\r') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            values.push(trimmed.to_string());
        }
    }

    values
}

/// Modal for pasting multiple values to create bulk filter conditions.
#[modal(size = Md)]
struct PasteConditionsModal {
    #[state(skip)]
    options: Vec<(String, String)>,
    #[state(skip)]
    metadata: EntityMetadata,

    field: AutocompleteState<String>,
    operator: SelectState<CondOp>,
    paste_input: String,
    error: Option<String>,
}

impl PasteConditionsModal {
    fn new_modal(
        options: Vec<(String, String)>,
        metadata: EntityMetadata,
    ) -> Self {
        let op_options: Vec<(CondOp, String)> = vec![
            (CondOp::Eq, "eq".to_string()),
            (CondOp::Ne, "ne".to_string()),
        ];

        Self::new(
            options,
            metadata,
            AutocompleteState::default(),
            SelectState::new(op_options).with_value(CondOp::Eq),
            String::new(),
            None,
        )
    }
}

#[modal_impl]
impl PasteConditionsModal {
    fn default_result(&self) -> Option<Vec<ConditionData>> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<Vec<ConditionData>>>) {
        self.field
            .set(AutocompleteState::new(self.options.clone()));
        mx.focus("paste-field-autocomplete");
    }

    #[keybinds]
    fn keys() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<Vec<ConditionData>>>) {
        mx.close(None);
    }

    #[handler]
    async fn confirm(&self, mx: &ModalContext<Option<Vec<ConditionData>>>) {
        let field = self.field.with_ref(|s| s.value().cloned());
        let Some(field_name) = field else {
            self.error.set(Some("Select a field first".to_string()));
            return;
        };

        let op = self.operator.with_ref(|s| s.value().cloned());
        let Some(operator) = op else {
            self.error.set(Some("Select an operator".to_string()));
            return;
        };

        let input = self.paste_input.get();
        let raw_values = parse_pasted_values(&input);
        if raw_values.is_empty() {
            self.error
                .set(Some("Paste at least one value".to_string()));
            return;
        }

        // Determine the attribute type for parsing
        let attr_type = self
            .metadata
            .attributes
            .iter()
            .find(|a| a.logical_name == field_name)
            .map(|a| a.attribute_type);

        // Parse each value
        let mut conditions = Vec::new();
        let mut errors = Vec::new();

        for raw in &raw_values {
            match parse_filter_value(raw, attr_type) {
                Ok(value) => {
                    conditions.push(ConditionData {
                        field: field_name.clone(),
                        operator,
                        value,
                    });
                }
                Err(e) => {
                    errors.push(format!("'{}': {}", raw, e));
                }
            }
        }

        if !errors.is_empty() {
            let msg = format!(
                "Failed to parse {} of {} values:\n{}",
                errors.len(),
                raw_values.len(),
                errors.into_iter().take(3).collect::<Vec<_>>().join(", ")
            );
            self.error.set(Some(msg));
            return;
        }

        mx.close(Some(conditions));
    }

    fn element(&self) -> Element {
        let error = self.error.get();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Paste Conditions") style (bold, fg: interact)

                if let Some(err) = error {
                    text (content: {err}) style (fg: primary)
                }

                text (content: "Field") style (fg: muted)
                autocomplete (state: self.field, id: "paste-field-autocomplete", placeholder: "Search fields...")

                text (content: "Operator") style (fg: muted)
                select (state: self.operator, id: "paste-operator", placeholder: "Select operator...")

                text (content: "Values (comma/newline separated):") style (fg: muted)
                input (
                    state: self.paste_input,
                    id: "paste-values-input",
                    placeholder: "e.g., value1, value2, value3"
                )

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn")
                        on_activate: cancel()

                    button (label: "Add All", id: "add-btn")
                        on_activate: confirm()
                }
            }
        }
    }
}
