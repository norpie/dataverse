//! Modal for building filter conditions.

use dataverse_lib::DataverseClient;
use dataverse_lib::model::metadata::EntityMetadata;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::Text;
use rafter::widgets::Tree;
use rafter::widgets::TreeState;

use crate::modals::LoadingModal;
use crate::widgets::filter_builder::ConditionData;
use crate::widgets::filter_builder::ConditionEditorModal;
use crate::widgets::filter_builder::FilterNode;
use crate::widgets::filter_builder::FilterTreeItem;
use crate::widgets::filter_builder::FilterTreeKey;
use crate::widgets::filter_builder::build_tree;

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
        bind("g", add_group);
        bind("n", toggle_not);
        bind("d", delete_item);
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
