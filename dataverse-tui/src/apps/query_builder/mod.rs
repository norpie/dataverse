//! Query Builder app for constructing OData queries visually.

pub mod convert;
pub mod data;
mod migrations;
mod modals;
pub mod repository;
mod tree;

use dataverse_lib::DataverseClient;
use dataverse_lib::error::Error as DataverseError;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Text, Tree, TreeNode, TreeState};

use crate::apps::{Export, RecordExplorer};
use crate::modals::{ListEntry, LoadingModal, SearchableListModal};
use crate::paths;
use crate::systems::client_management::{ActiveClientInfo, ClientManagement, GetActiveClient};
use data::{FilterNode, QueryData, SortField};
use modals::{
    ConditionData, ConditionEditorModal, EntityPickerModal, FieldPickerModal, LoadQueryModal,
    NumberEditorModal, SaveQueryModal, SortFieldEditorModal,
};
use repository::QueryRepository;
use tree::{QueryTreeNode, build_tree};

/// Query Builder app: visual tree-based OData query construction.
#[app(name = "Query Builder")]
pub struct QueryBuilder {
    /// Full connection context.
    #[state(skip)]
    client_info: ActiveClientInfo,

    /// Tree widget state.
    tree_state: TreeState<QueryTreeNode>,
    /// The query being constructed.
    query: QueryData,
    /// Repository for saving/loading queries.
    repo: Option<QueryRepository>,
    /// ID of the currently loaded saved query (for update-in-place).
    saved_query_id: Option<i64>,
    /// Name of the currently loaded saved query.
    saved_query_name: Option<String>,
}

impl QueryBuilder {
    pub fn with_client(client_info: ActiveClientInfo) -> Self {
        Self::new(
            client_info,
            TreeState::default(),
            QueryData::default(),
            None,
            None,
            None,
        )
    }
}

#[app_impl]
impl QueryBuilder {
    #[on_start]
    async fn on_start(&self, gx: &GlobalContext) {
        // Initialize repository
        if let Some(db_path) = paths::queries_db() {
            match QueryRepository::new(&db_path).await {
                Ok(repo) => self.repo.set(Some(repo)),
                Err(e) => {
                    log::error!("Failed to open queries database: {}", e);
                    gx.toast(Toast::error("Failed to open queries database"));
                }
            }
        }

        // Sync tree nodes and expand all
        let nodes = self.tree_nodes();
        self.tree_state.update(|s| {
            s.set_roots(nodes);
            s.expand_all();
        });
    }

    fn title(&self) -> String {
        self.query.with_ref(|q| match &q.entity {
            Some(entity) => format!("Query Builder ({})", entity.name()),
            None => "Query Builder".to_string(),
        })
    }

    /// Automatically compute tree nodes from query state.
    #[derived]
    fn tree_nodes(&self) -> Vec<TreeNode<QueryTreeNode>> {
        self.query.with_ref(build_tree)
    }

    // =========================================================================
    // Keybinds
    // =========================================================================

    #[keybinds]
    fn keybinds() {
        bind("escape", close_app);
        bind("a", add_node);
        bind("g", add_group);
        bind("t", toggle_group);
        bind("n", toggle_not);
        bind("d", delete_node);
        bind("s", save_query);
        bind("l", load_query);
        bind("x", send_query);
        bind("ctrl+r", reset_query);
    }

    #[handler]
    async fn close_app(&self, gx: &GlobalContext, cx: &AppContext) {
        let confirmed = gx
            .modal(crate::modals::ConfirmModal::with_message(
                "Close the query builder?",
            ))
            .await;
        if confirmed {
            cx.close();
        }
    }

    /// Toggle AND/OR on the focused filter group.
    #[handler]
    async fn toggle_group(&self) {
        let Some(key) = self.focused_key() else {
            return;
        };

        let tree::QueryTreeKey::FilterGroup(id) = key else {
            return;
        };

        self.query.update(|q| q.filter.toggle_group(id));
    }

    /// Toggle NOT on the focused filter group.
    #[handler]
    async fn toggle_not(&self) {
        let Some(key) = self.focused_key() else {
            return;
        };

        let tree::QueryTreeKey::FilterGroup(id) = key else {
            return;
        };

        self.query.update(|q| q.filter.toggle_negation(id));
    }

    /// Delete the focused node.
    #[handler]
    async fn delete_node(&self, gx: &GlobalContext) {
        let Some(key) = self.focused_key() else {
            return;
        };

        match key {
            tree::QueryTreeKey::SelectField(idx) => {
                let len = self.query.with_ref(|q| q.select.len());
                if idx < len {
                    self.query.update(|q| {
                        q.select.remove(idx);
                    });
                }
            }
            tree::QueryTreeKey::FilterCondition(id) => {
                self.query.update(|q| {
                    q.filter.remove_node(id);
                });
            }
            tree::QueryTreeKey::FilterGroup(id) => {
                let has_children = self.query.with_ref(|q| q.filter.group_has_children(id));
                if has_children {
                    let confirmed = gx
                        .modal(crate::modals::ConfirmModal::with_message(
                            "Delete this group and all its children?",
                        ))
                        .await;
                    if !confirmed {
                        return;
                    }
                }
                self.query.update(|q| {
                    q.filter.remove_node(id);
                });
            }
            tree::QueryTreeKey::SortItem(id) => {
                self.query.update(|q| {
                    q.order_by.retain(|sf| sf.id != id);
                });
            }
            tree::QueryTreeKey::TopValue => {
                self.query.update(|q| {
                    q.top = None;
                });
            }
            _ => {}
        }
    }

    /// Tree node activated (Enter/Space).
    #[handler]
    async fn on_activate(&self, cx: &AppContext, gx: &GlobalContext) {
        let Some(key) = self.focused_key() else {
            return;
        };

        match key {
            tree::QueryTreeKey::Section(tree::Section::Entity)
            | tree::QueryTreeKey::EntityValue => {
                self.open_entity_picker(gx).await;
            }
            tree::QueryTreeKey::Section(tree::Section::Top) | tree::QueryTreeKey::TopValue => {
                self.open_top_editor(gx).await;
            }
            tree::QueryTreeKey::Section(tree::Section::Select) => {
                self.open_field_picker(gx).await;
            }
            tree::QueryTreeKey::SelectField(idx) => {
                self.show_select_menu(idx, cx, gx).await;
            }
            tree::QueryTreeKey::Section(tree::Section::OrderBy) => {
                self.open_sort_editor(gx).await;
            }
            tree::QueryTreeKey::SortItem(id) => {
                self.show_sort_menu(id, cx, gx).await;
            }
            tree::QueryTreeKey::Section(tree::Section::Filter) => {
                self.open_condition_editor(gx, None).await;
            }
            tree::QueryTreeKey::FilterCondition(id) => {
                self.open_condition_editor_for(gx, id).await;
            }
            _ => {}
        }
    }

    /// Add a new node based on focused section.
    #[handler]
    async fn add_node(&self, gx: &GlobalContext) {
        let Some(key) = self.focused_key() else {
            return;
        };

        match &key {
            tree::QueryTreeKey::Section(tree::Section::Entity)
            | tree::QueryTreeKey::EntityValue => {
                self.open_entity_picker(gx).await;
            }
            tree::QueryTreeKey::Section(tree::Section::Top) | tree::QueryTreeKey::TopValue => {
                self.open_top_editor(gx).await;
            }
            tree::QueryTreeKey::Section(tree::Section::Select)
            | tree::QueryTreeKey::SelectField(_) => {
                self.open_field_picker(gx).await;
            }
            tree::QueryTreeKey::Section(tree::Section::OrderBy)
            | tree::QueryTreeKey::SortItem(_) => {
                self.open_sort_editor(gx).await;
            }
            tree::QueryTreeKey::Section(tree::Section::Filter)
            | tree::QueryTreeKey::FilterGroup(_)
            | tree::QueryTreeKey::FilterCondition(_) => {
                // Determine which group to add to
                let group_id = self.focused_filter_group_id(&key);
                self.open_condition_editor(gx, group_id).await;
            }
        }
    }

    /// Add a new AND/OR group to the focused filter group.
    #[handler]
    async fn add_group(&self) {
        let Some(key) = self.focused_key() else {
            return;
        };

        // Only valid in filter context
        if !matches!(
            key,
            tree::QueryTreeKey::Section(tree::Section::Filter)
                | tree::QueryTreeKey::FilterGroup(_)
                | tree::QueryTreeKey::FilterCondition(_)
        ) {
            return;
        }

        let group_id = self.focused_filter_group_id(&key);
        self.query.update(|q| {
            let id = q.next_id();
            let new_group = FilterNode::Group {
                id,
                is_and: true,
                is_negated: false,
                children: vec![],
            };
            match group_id {
                Some(gid) => {
                    q.filter.add_to_group(gid, new_group);
                }
                None => {
                    // No root group yet, create one and add the new group inside
                    let root_id = q.next_id();
                    q.filter = FilterNode::Group {
                        id: root_id,
                        is_and: true,
                        is_negated: false,
                        children: vec![new_group],
                    };
                }
            }
        });
    }

    // =========================================================================
    // Context Menus
    // =========================================================================

    /// Show context menu for a select field node.
    #[handler]
    async fn show_select_menu(&self, idx: usize, cx: &AppContext, gx: &GlobalContext) {
        let (x, y) = if let Some(rect) = gx.focused_element_rect() {
            (
                rect.x.max(0) as u16,
                (rect.y + rect.height as i16).max(0) as u16,
            )
        } else {
            gx.mouse_position()
        };
        let menu = self.select_field_menu(idx);
        cx.context_menu(menu, x, y);
    }

    #[context_menu]
    fn select_field_menu(&self, idx: usize) {
        context_menu! {
            option("Edit", edit_select_fields());
            option("Remove", remove_select_field(idx));
        }
    }

    #[handler]
    async fn edit_select_fields(&self, gx: &GlobalContext) {
        self.open_field_picker(gx).await;
    }

    #[handler]
    async fn remove_select_field(&self, idx: usize) {
        let len = self.query.with_ref(|q| q.select.len());
        if idx < len {
            self.query.update(|q| {
                q.select.remove(idx);
            });
        }
    }

    /// Show context menu for a sort field node.
    #[handler]
    async fn show_sort_menu(&self, id: usize, cx: &AppContext, gx: &GlobalContext) {
        let (x, y) = if let Some(rect) = gx.focused_element_rect() {
            (
                rect.x.max(0) as u16,
                (rect.y + rect.height as i16).max(0) as u16,
            )
        } else {
            gx.mouse_position()
        };
        let menu = self.sort_field_menu(id);
        cx.context_menu(menu, x, y);
    }

    #[context_menu]
    fn sort_field_menu(&self, id: usize) {
        context_menu! {
            option("Toggle Direction", toggle_sort_direction(id));
            option("Remove", remove_sort_field(id));
        }
    }

    #[handler]
    async fn toggle_sort_direction(&self, id: usize) {
        use dataverse_lib::api::query::Direction;
        self.query.update(|q| {
            if let Some(sf) = q.order_by.iter_mut().find(|sf| sf.id == id) {
                sf.direction = match sf.direction {
                    Direction::Asc => Direction::Desc,
                    Direction::Desc => Direction::Asc,
                };
            }
        });
    }

    #[handler]
    async fn remove_sort_field(&self, id: usize) {
        self.query.update(|q| {
            q.order_by.retain(|sf| sf.id != id);
        });
    }

    /// Open the condition editor pre-filled for an existing condition.
    async fn open_condition_editor_for(&self, gx: &GlobalContext, id: usize) {
        let cond = self.query.with_ref(|q| q.filter.find_condition(id));
        let Some((field, operator, value)) = cond else {
            return;
        };

        let entity = self.query.with_ref(|q| q.entity.clone());
        let Some(entity) = entity else {
            return;
        };

        let Some(client) = self.get_client(gx).await else {
            return;
        };

        let entity_clone = entity.clone();
        let metadata = match gx
            .modal(LoadingModal::run_with_default(
                "Loading entity metadata",
                || Err(DataverseError::Cancelled),
                async move { client.metadata().entity(entity_clone).await },
            ))
            .await
        {
            Ok(m) => m,
            Err(e) if e.is_cancelled() => return,
            Err(e) => {
                gx.toast(Toast::error(format!("Failed to load metadata: {}", e)));
                return;
            }
        };

        let options: Vec<(String, String)> = metadata
            .attributes
            .iter()
            .filter(|a| a.is_valid_for_read && a.attribute_of.is_none())
            .map(|a| {
                let display_name = a.display_name.text_or(&a.logical_name);
                let display = format!("{} ({})", display_name, a.logical_name);
                (a.logical_name.clone(), display)
            })
            .collect();

        let initial = ConditionData {
            field,
            operator,
            value,
        };

        let result = gx
            .modal(ConditionEditorModal::with_condition(
                options, metadata, initial,
            ))
            .await;

        if let Some(cond) = result {
            self.query.update(|q| {
                q.filter
                    .update_condition(id, cond.field, cond.operator, cond.value);
            });
        }
    }

    /// Save the current query.
    #[handler]
    async fn save_query(&self, gx: &GlobalContext) {
        let Some(repo) = self.repo.get() else {
            gx.toast(Toast::error("Database not available"));
            return;
        };

        let current_name = self.saved_query_name.get();
        let result = gx
            .modal(SaveQueryModal::with_name(current_name.clone()))
            .await;
        let Some(name) = result else { return };

        let data = self.query.with_ref(|q| q.clone());
        let id = if current_name.as_deref() == Some(name.as_str()) {
            self.saved_query_id.get()
        } else {
            None
        };

        match repo.save(id, name.clone(), &data).await {
            Ok(id) => {
                self.saved_query_id.set(Some(id));
                self.saved_query_name.set(Some(name.clone()));
                gx.toast(Toast::info(format!("Saved: {}", name)));
            }
            Err(e) => {
                gx.toast(Toast::error(format!("Failed to save: {}", e)));
            }
        }
    }

    /// Load a saved query.
    #[handler]
    async fn load_query(&self, gx: &GlobalContext) {
        let Some(repo) = self.repo.get() else {
            gx.toast(Toast::error("Database not available"));
            return;
        };

        let queries = match repo.list().await {
            Ok(list) => list,
            Err(e) => {
                gx.toast(Toast::error(format!("Failed to list queries: {}", e)));
                return;
            }
        };

        if queries.is_empty() {
            gx.toast(Toast::info("No saved queries"));
            return;
        }

        let options: Vec<(i64, String)> = queries
            .into_iter()
            .map(|q| {
                let label = match &q.entity {
                    Some(entity) => format!("{} ({})", q.name, entity),
                    None => q.name.clone(),
                };
                (q.id, label)
            })
            .collect();

        let result = gx.modal(LoadQueryModal::with_queries(options)).await;

        let Some((to_load, to_delete)) = result else {
            return;
        };

        // Commit staged deletions
        let mut delete_count = 0;
        for id in to_delete {
            match repo.delete(id).await {
                Ok(()) => {
                    delete_count += 1;
                }
                Err(e) => {
                    gx.toast(Toast::error(format!("Failed to delete query: {}", e)));
                }
            }
        }

        if delete_count > 0 {
            gx.toast(Toast::info(format!(
                "Deleted {} {}",
                delete_count,
                if delete_count == 1 {
                    "query"
                } else {
                    "queries"
                }
            )));
        }

        // Load selected query if any
        if let Some(id) = to_load {
            match repo.load(id).await {
                Ok(saved) => {
                    self.saved_query_id.set(Some(saved.id));
                    self.saved_query_name.set(Some(saved.name.clone()));
                    self.query.set(saved.data);

                    // Sync tree nodes and expand all
                    let nodes = self.tree_nodes();
                    self.tree_state.update(|s| {
                        s.set_roots(nodes);
                        s.clear_expansion();
                        s.expand_all();
                    });

                    gx.toast(Toast::info(format!("Loaded: {}", saved.name)));
                }
                Err(e) => {
                    gx.toast(Toast::error(format!("Failed to load: {}", e)));
                }
            }
        }
    }

    /// Reset the query to a blank state.
    #[handler]
    async fn reset_query(&self, gx: &GlobalContext) {
        let confirmed = gx
            .modal(crate::modals::ConfirmModal::with_message(
                "Reset the query? All unsaved changes will be lost.",
            ))
            .await;
        if confirmed {
            self.query.set(QueryData::default());
            self.saved_query_id.set(None);
            self.saved_query_name.set(None);
            gx.toast(Toast::info("Query reset"));
        }
    }

    /// Send the query to Record Explorer for execution.
    #[handler]
    async fn send_query(&self, gx: &GlobalContext, cx: &AppContext) {
        // Validate entity is set
        let has_entity = self.query.with_ref(|q| q.entity.is_some());
        if !has_entity {
            gx.toast(Toast::info("Select an entity first"));
            return;
        }

        // Show app selection modal
        let apps = vec![
            ListEntry::with_category("record-explorer", "Record Explorer", "Data"),
            ListEntry::with_category("export", "Export", "Data"),
            // Future apps will go here:
            // ListEntry::with_category("chart", "Chart Builder", "Visualization"),
        ];

        let selected = gx
            .modal(SearchableListModal::with_entries(
                "Execute Query In...",
                apps,
            ))
            .await;

        let Some(app_id) = selected else {
            return;
        };

        // Get client + environment info
        let info = match gx
            .request_system::<ClientManagement, GetActiveClient>(GetActiveClient)
            .await
        {
            Ok(Ok(info)) => info,
            Ok(Err(e)) => {
                gx.toast(Toast::error(format!("Client error: {}", e)));
                return;
            }
            Err(e) => {
                gx.toast(Toast::error(format!("No active connection: {:?}", e)));
                return;
            }
        };

        // Build the OData query
        let data = self.query.with_ref(|q| q.clone());
        let query = match convert::build_query(&data) {
            Ok(q) => q,
            Err(e) => {
                gx.toast(Toast::error(format!("Query error: {}", e)));
                return;
            }
        };

        // Launch the selected app
        match app_id.as_str() {
            "record-explorer" => {
                let _ = gx.spawn_and_focus(RecordExplorer::with_query(
                    query,
                    info,
                    Some(cx.instance_id()),
                ));
            }
            "export" => {
                let _ = gx.spawn_and_focus(Export::with_query(query, info, Some(cx.instance_id())));
            }
            _ => {
                gx.toast(Toast::info(format!("App not implemented: {}", app_id)));
            }
        }
    }

    // =========================================================================
    // Modal openers
    // =========================================================================

    async fn open_entity_picker(&self, gx: &GlobalContext) {
        // Check if there's existing data that will be cleared
        let has_data = self.query.with_ref(|q| {
            !q.select.is_empty() || !matches!(q.filter, FilterNode::Empty) || !q.order_by.is_empty()
        });

        // Confirm if changing entity will clear existing data
        if has_data {
            let confirmed = gx
                .modal(crate::modals::ConfirmModal::with_message(
                    "Changing entity will clear all fields, filters, and sorting. Continue?",
                ))
                .await;
            if !confirmed {
                return;
            }
        }

        let Some(client) = self.get_client(gx).await else {
            return;
        };

        let entities = match gx
            .modal(LoadingModal::run_with_default(
                "Loading entities",
                || Err(DataverseError::Cancelled),
                async move { client.metadata().all_entities().await },
            ))
            .await
        {
            Ok(all) => all
                .iter()
                .map(|e| {
                    let display = e
                        .display_name
                        .text()
                        .map(|d| format!("{} ({})", d, e.entity_set_name))
                        .unwrap_or_else(|| e.entity_set_name.clone());
                    (e.entity_set_name.clone(), display)
                })
                .collect(),
            Err(e) if e.is_cancelled() => return,
            Err(e) => {
                gx.toast(Toast::error(format!("Failed to load entities: {}", e)));
                return;
            }
        };

        let result = gx.modal(EntityPickerModal::with_options(entities)).await;
        if let Some(entity) = result {
            self.query.update(|q| {
                q.entity = Some(entity);
                // Clear fields that depend on entity
                q.select.clear();
                q.filter = FilterNode::Empty;
                q.order_by.clear();
            });
        }
    }

    async fn open_field_picker(&self, gx: &GlobalContext) {
        let entity = self.query.with_ref(|q| q.entity.clone());
        let Some(entity) = entity else {
            gx.toast(Toast::info("Select an entity first"));
            return;
        };

        let Some(client) = self.get_client(gx).await else {
            return;
        };

        let entity_clone = entity.clone();
        let options = match gx
            .modal(LoadingModal::run_with_default(
                "Loading attributes",
                || Err(DataverseError::Cancelled),
                async move { client.metadata().attributes(entity_clone).await },
            ))
            .await
        {
            Ok(attrs) => attrs
                .iter()
                .filter(|a| a.is_valid_for_read && a.attribute_of.is_none())
                .map(|a| {
                    let display = a
                        .display_name
                        .text()
                        .map(|d| format!("{} ({})", d, a.logical_name))
                        .unwrap_or_else(|| a.logical_name.clone());
                    (a.logical_name.clone(), display)
                })
                .collect(),
            Err(e) if e.is_cancelled() => return,
            Err(e) => {
                gx.toast(Toast::error(format!("Failed to load attributes: {}", e)));
                return;
            }
        };

        let current = self.query.with_ref(|q| q.select.clone());
        let result = gx
            .modal(FieldPickerModal::with_selected(options, current))
            .await;
        if let Some(selected) = result {
            self.query.update(|q| q.select = selected);
        }
    }

    async fn open_condition_editor(&self, gx: &GlobalContext, group_id: Option<usize>) {
        let entity = self.query.with_ref(|q| q.entity.clone());
        let Some(entity) = entity else {
            gx.toast(Toast::info("Select an entity first"));
            return;
        };

        let Some(client) = self.get_client(gx).await else {
            return;
        };

        let entity_clone = entity.clone();
        let metadata = match gx
            .modal(LoadingModal::run_with_default(
                "Loading entity metadata",
                || Err(DataverseError::Cancelled),
                async move { client.metadata().entity(entity_clone).await },
            ))
            .await
        {
            Ok(m) => m,
            Err(e) if e.is_cancelled() => return,
            Err(e) => {
                gx.toast(Toast::error(format!("Failed to load metadata: {}", e)));
                return;
            }
        };

        let options: Vec<(String, String)> = metadata
            .attributes
            .iter()
            .filter(|a| a.is_valid_for_read && a.attribute_of.is_none())
            .map(|a| {
                let display_name = a.display_name.text_or(&a.logical_name);
                let display = format!("{} ({})", display_name, a.logical_name);
                (a.logical_name.clone(), display)
            })
            .collect();

        let result = gx
            .modal(ConditionEditorModal::with_options(options, metadata))
            .await;
        if let Some(cond) = result {
            self.query.update(|q| {
                let id = q.next_id();
                let node = FilterNode::Condition {
                    id,
                    field: cond.field,
                    operator: cond.operator,
                    value: cond.value,
                };
                match group_id {
                    Some(gid) => {
                        q.filter.add_to_group(gid, node);
                    }
                    None => match &q.filter {
                        FilterNode::Empty => {
                            let root_id = q.next_id();
                            q.filter = FilterNode::Group {
                                id: root_id,
                                is_and: true,
                                is_negated: false,
                                children: vec![node],
                            };
                        }
                        FilterNode::Group { .. } => {
                            // Add to root group
                            if let FilterNode::Group { children, .. } = &mut q.filter {
                                children.push(node);
                            }
                        }
                        FilterNode::Condition { .. } => {
                            // Shouldn't happen at root level
                        }
                    },
                }
            });
        }
    }

    async fn open_sort_editor(&self, gx: &GlobalContext) {
        let entity = self.query.with_ref(|q| q.entity.clone());
        let Some(entity) = entity else {
            gx.toast(Toast::info("Select an entity first"));
            return;
        };

        let Some(client) = self.get_client(gx).await else {
            return;
        };

        let entity_clone = entity.clone();
        let options = match gx
            .modal(LoadingModal::run_with_default(
                "Loading attributes",
                || Err(DataverseError::Cancelled),
                async move { client.metadata().attributes(entity_clone).await },
            ))
            .await
        {
            Ok(attrs) => attrs
                .iter()
                .filter(|a| a.is_valid_for_read && a.attribute_of.is_none())
                .map(|a| {
                    let display = a
                        .display_name
                        .text()
                        .map(|d| format!("{} ({})", d, a.logical_name))
                        .unwrap_or_else(|| a.logical_name.clone());
                    (a.logical_name.clone(), display)
                })
                .collect(),
            Err(e) if e.is_cancelled() => return,
            Err(e) => {
                gx.toast(Toast::error(format!("Failed to load attributes: {}", e)));
                return;
            }
        };

        let result = gx.modal(SortFieldEditorModal::with_options(options)).await;
        if let Some((field, direction)) = result {
            self.query.update(|q| {
                let id = q.next_id();
                q.order_by.push(SortField {
                    id,
                    field,
                    direction,
                });
            });
        }
    }

    async fn open_top_editor(&self, gx: &GlobalContext) {
        let current = self.query.with_ref(|q| q.top);
        let result = gx.modal(NumberEditorModal::with_value(current)).await;
        if let Some(val) = result {
            self.query.update(|q| {
                q.top = Some(val);
            });
        }
    }

    // =========================================================================
    // Helpers
    // =========================================================================

    async fn get_client(&self, gx: &GlobalContext) -> Option<DataverseClient> {
        match gx
            .request_system::<ClientManagement, GetActiveClient>(GetActiveClient)
            .await
        {
            Ok(Ok(info)) => Some(info.client),
            Ok(Err(e)) => {
                gx.toast(Toast::error(format!("Client error: {}", e)));
                None
            }
            Err(e) => {
                gx.toast(Toast::error(format!("No active connection: {:?}", e)));
                None
            }
        }
    }

    /// Get the key of the currently focused tree node.
    fn focused_key(&self) -> Option<tree::QueryTreeKey> {
        self.tree_state.with_ref(|s| s.focused_key.clone())
    }

    /// Determine the filter group ID to add to, based on the focused key.
    fn focused_filter_group_id(&self, key: &tree::QueryTreeKey) -> Option<usize> {
        match key {
            tree::QueryTreeKey::FilterGroup(id) => Some(*id),
            tree::QueryTreeKey::FilterCondition(cond_id) => {
                // Find the parent group of this condition
                self.query
                    .with_ref(|q| q.filter.find_parent_group_id(*cond_id))
            }
            tree::QueryTreeKey::Section(tree::Section::Filter) => {
                // Return the root group ID if it exists
                self.query.with_ref(|q| match &q.filter {
                    FilterNode::Group { id, .. } => Some(*id),
                    _ => None,
                })
            }
            _ => None,
        }
    }

    // =========================================================================
    // UI
    // =========================================================================

    fn element(&self) -> Element {
        // Sync tree nodes to tree state (automatically recomputed when query changes)
        let nodes = self.tree_nodes();
        self.tree_state.update(|s| s.set_roots(nodes));

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: background) {
                // Header
                row (width: fill, justify: between) {
                    text (content: "Query Builder") style (bold, fg: interact)
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
                            text (content: "d") style (fg: primary)
                            text (content: "delete") style (fg: muted)
                        }
                        row (gap: 1) {
                            text (content: "t") style (fg: primary)
                            text (content: "toggle") style (fg: muted)
                        }
                    }
                }

                // Tree
                box_ (id: "query-tree-container", height: fill, width: fill) style (bg: surface) {
                    tree (state: self.tree_state, id: "query-tree")
                        on_activate: on_activate()
                }

                // Footer
                row (width: fill, justify: between) {
                    row (gap: 2) {
                        row (gap: 1) {
                            text (content: "esc") style (fg: primary)
                            text (content: "close") style (fg: muted)
                        }
                        row (gap: 1) {
                            text (content: "ctrl+r") style (fg: primary)
                            text (content: "reset") style (fg: muted)
                        }
                    }
                    row (gap: 2) {
                        row (gap: 1) {
                            text (content: "x") style (fg: primary)
                            text (content: "run") style (fg: muted)
                        }
                        row (gap: 1) {
                            text (content: "s") style (fg: primary)
                            text (content: "save") style (fg: muted)
                        }
                        row (gap: 1) {
                            text (content: "l") style (fg: primary)
                            text (content: "load") style (fg: muted)
                        }
                    }
                }

            }
        }
    }
}
