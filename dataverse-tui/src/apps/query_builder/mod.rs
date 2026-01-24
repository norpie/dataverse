//! Query Builder app for constructing OData queries visually.

pub mod convert;
pub mod data;
mod migrations;
mod modals;
pub mod repository;
mod tree;

use dataverse_lib::DataverseClient;
use dataverse_lib::model::Entity;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Text, Tree, TreeState};

use crate::apps::RecordExplorer;

use crate::paths;
use crate::systems::client_management::{ClientManagement, GetActiveClient};
use crate::widgets::loading_overlay;
use data::{FilterNode, QueryData, SortField};
use modals::{
    ConditionEditorModal, EntityPickerModal, FieldPickerModal, LoadQueryModal, NumberEditorModal,
    SaveQueryModal, SortFieldEditorModal,
};
use repository::QueryRepository;
use tree::{QueryTreeNode, build_tree};

/// Query Builder app: visual tree-based OData query construction.
#[app(name = "Query Builder")]
pub struct QueryBuilder {
    /// Tree widget state.
    tree_state: TreeState<QueryTreeNode>,
    /// The query being constructed.
    query: QueryData,
    /// Loading overlay message.
    loading_message: Option<String>,
    /// Repository for saving/loading queries.
    repo: Option<QueryRepository>,
    /// ID of the currently loaded saved query (for update-in-place).
    saved_query_id: Option<i64>,
    /// Name of the currently loaded saved query.
    saved_query_name: Option<String>,
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

        self.rebuild_tree();
        self.tree_state.update(|s| {
            s.expand(&"section-Entity".to_string());
            s.expand(&"section-Select".to_string());
            s.expand(&"section-Filter".to_string());
            s.expand(&"section-OrderBy".to_string());
            s.expand(&"section-Top".to_string());
        });
    }

    fn title(&self) -> String {
        self.query.with_ref(|q| match &q.entity {
            Some(entity) => format!("Query Builder ({})", entity),
            None => "Query Builder".to_string(),
        })
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
        bind("d", delete_node);
        bind("s", save_query);
        bind("l", load_query);
        bind("x", send_query);
        bind("ctrl+r", reset_query);
    }

    #[handler]
    async fn close_app(&self, gx: &GlobalContext, cx: &AppContext) {
        let confirmed = gx
            .modal(crate::modals::ConfirmModal::new("Close the query builder?"))
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

        let Some(group_id) = key.strip_prefix("filter-group-") else {
            return;
        };
        let Ok(id) = group_id.parse::<usize>() else {
            return;
        };

        self.query.update(|q| q.filter.toggle_group(id));
        self.rebuild_tree();
    }

    /// Delete the focused node.
    #[handler]
    async fn delete_node(&self, gx: &GlobalContext) {
        let Some(key) = self.focused_key() else {
            return;
        };

        if let Some(idx_str) = key.strip_prefix("select-") {
            if let Ok(idx) = idx_str.parse::<usize>() {
                let len = self.query.with_ref(|q| q.select.len());
                if idx < len {
                    self.query.update(|q| {
                        q.select.remove(idx);
                    });
                    self.rebuild_tree();
                }
            }
        } else if let Some(id_str) = key.strip_prefix("filter-cond-") {
            if let Ok(id) = id_str.parse::<usize>() {
                self.query.update(|q| {
                    q.filter.remove_node(id);
                });
                self.rebuild_tree();
            }
        } else if let Some(id_str) = key.strip_prefix("filter-group-") {
            if let Ok(id) = id_str.parse::<usize>() {
                let has_children = self.query.with_ref(|q| q.filter.group_has_children(id));
                if has_children {
                    let confirmed = gx
                        .modal(crate::modals::ConfirmModal::new(
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
                self.rebuild_tree();
            }
        } else if let Some(id_str) = key.strip_prefix("sort-") {
            if let Ok(id) = id_str.parse::<usize>() {
                self.query.update(|q| {
                    q.order_by.retain(|sf| sf.id != id);
                });
                self.rebuild_tree();
            }
        } else if key == "top-value" {
            self.query.update(|q| {
                q.top = None;
            });
            self.rebuild_tree();
        }
    }

    /// Tree node activated (Enter/Space).
    #[handler]
    async fn on_activate(&self, gx: &GlobalContext) {
        let Some(key) = self.focused_key() else {
            return;
        };

        match key.as_str() {
            "section-Entity" | "entity-value" => {
                self.open_entity_picker(gx).await;
            }
            "section-Top" | "top-value" => {
                self.open_top_editor(gx).await;
            }
            k if k == "section-Select" || k.starts_with("select-") => {
                self.open_field_picker(gx).await;
            }
            k if k == "section-OrderBy" || k.starts_with("sort-") => {
                self.open_sort_editor(gx).await;
            }
            k if k == "section-Filter"
                || k.starts_with("filter-group-")
                || k.starts_with("filter-cond-") =>
            {
                self.open_condition_editor(gx, None).await;
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

        match key.as_str() {
            "section-Entity" | "entity-value" => {
                self.open_entity_picker(gx).await;
            }
            "section-Top" | "top-value" => {
                self.open_top_editor(gx).await;
            }
            k if k == "section-Select" || k.starts_with("select-") => {
                self.open_field_picker(gx).await;
            }
            k if k == "section-OrderBy" || k.starts_with("sort-") => {
                self.open_sort_editor(gx).await;
            }
            k if k == "section-Filter"
                || k.starts_with("filter-group-")
                || k.starts_with("filter-cond-") =>
            {
                // Determine which group to add to
                let group_id = self.focused_filter_group_id(&key);
                self.open_condition_editor(gx, group_id).await;
            }
            _ => {}
        }
    }

    /// Add a new AND/OR group to the focused filter group.
    #[handler]
    async fn add_group(&self) {
        let Some(key) = self.focused_key() else {
            return;
        };

        // Only valid in filter context
        if key != "section-Filter"
            && !key.starts_with("filter-group-")
            && !key.starts_with("filter-cond-")
        {
            return;
        }

        let group_id = self.focused_filter_group_id(&key);
        self.query.update(|q| {
            let id = q.next_id();
            let new_group = FilterNode::Group {
                id,
                is_and: true,
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
                        children: vec![new_group],
                    };
                }
            }
        });
        self.rebuild_tree();
    }

    /// Save the current query.
    #[handler]
    async fn save_query(&self, gx: &GlobalContext) {
        let Some(repo) = self.repo.get() else {
            gx.toast(Toast::error("Database not available"));
            return;
        };

        let current_name = self.saved_query_name.get();
        let result = gx.modal(SaveQueryModal::new(current_name)).await;
        let Some(name) = result else { return };

        let data = self.query.with_ref(|q| q.clone());
        let existing_id = self.saved_query_id.get();

        match repo.save(existing_id, name.clone(), &data).await {
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

        let result = gx.modal(LoadQueryModal::new(options)).await;
        let Some(id) = result else { return };

        match repo.load(id).await {
            Ok(saved) => {
                self.saved_query_id.set(Some(saved.id));
                self.saved_query_name.set(Some(saved.name.clone()));
                self.query.set(saved.data);
                self.rebuild_tree();
                gx.toast(Toast::info(format!("Loaded: {}", saved.name)));
            }
            Err(e) => {
                gx.toast(Toast::error(format!("Failed to load query: {}", e)));
            }
        }
    }

    /// Reset the query to a blank state.
    #[handler]
    async fn reset_query(&self, gx: &GlobalContext) {
        let confirmed = gx
            .modal(crate::modals::ConfirmModal::new(
                "Reset the query? All unsaved changes will be lost.",
            ))
            .await;
        if confirmed {
            self.query.set(QueryData::default());
            self.saved_query_id.set(None);
            self.saved_query_name.set(None);
            self.rebuild_tree();
            gx.toast(Toast::info("Query reset"));
        }
    }

    /// Send the query to Record Explorer for execution.
    #[handler]
    async fn send_query(&self, gx: &GlobalContext) {
        // Validate entity is set
        let has_entity = self.query.with_ref(|q| q.entity.is_some());
        if !has_entity {
            gx.toast(Toast::info("Select an entity first"));
            return;
        }

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
        let query = match convert::build_query(&info.client, &data) {
            Ok(q) => q,
            Err(e) => {
                gx.toast(Toast::error(format!("Query error: {}", e)));
                return;
            }
        };

        let _ = gx.spawn_and_focus(RecordExplorer::new(query, info.environment_name));
    }

    // =========================================================================
    // Modal openers
    // =========================================================================

    async fn open_entity_picker(&self, gx: &GlobalContext) {
        let Some(client) = self.get_client(gx).await else {
            return;
        };

        self.loading_message
            .set(Some("Loading entities...".to_string()));

        let entities = match client.metadata().all_entities().await {
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
            Err(e) => {
                self.loading_message.set(None);
                gx.toast(Toast::error(format!("Failed to load entities: {}", e)));
                return;
            }
        };

        self.loading_message.set(None);

        let result = gx.modal(EntityPickerModal::new(entities)).await;
        if let Some(entity) = result {
            self.query.update(|q| {
                q.entity = Some(entity);
                // Clear fields that depend on entity
                q.select.clear();
                q.filter = FilterNode::Empty;
                q.order_by.clear();
            });
            self.rebuild_tree();
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

        self.loading_message
            .set(Some("Loading attributes...".to_string()));

        let options = match self.fetch_field_options(&client, &entity).await {
            Ok(opts) => opts,
            Err(e) => {
                self.loading_message.set(None);
                gx.toast(Toast::error(e));
                return;
            }
        };

        self.loading_message.set(None);

        let result = gx.modal(FieldPickerModal::new(options)).await;
        if !result.is_empty() {
            self.query.update(|q| {
                for field in result {
                    if !q.select.contains(&field) {
                        q.select.push(field);
                    }
                }
            });
            self.rebuild_tree();
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

        self.loading_message
            .set(Some("Loading attributes...".to_string()));

        let attrs = match client.metadata().attributes(Entity::set(&entity)).await {
            Ok(a) => a,
            Err(e) => {
                self.loading_message.set(None);
                gx.toast(Toast::error(format!("Failed to load attributes: {}", e)));
                return;
            }
        };

        self.loading_message.set(None);

        let options: Vec<(String, String)> = attrs
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
            .collect();

        let result = gx.modal(ConditionEditorModal::new(options, attrs)).await;
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
            self.rebuild_tree();
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

        self.loading_message
            .set(Some("Loading attributes...".to_string()));

        let options = match self.fetch_field_options(&client, &entity).await {
            Ok(opts) => opts,
            Err(e) => {
                self.loading_message.set(None);
                gx.toast(Toast::error(e));
                return;
            }
        };

        self.loading_message.set(None);

        let result = gx.modal(SortFieldEditorModal::new(options)).await;
        if let Some((field, direction)) = result {
            self.query.update(|q| {
                let id = q.next_id();
                q.order_by.push(SortField {
                    id,
                    field,
                    direction,
                });
            });
            self.rebuild_tree();
        }
    }

    async fn open_top_editor(&self, gx: &GlobalContext) {
        let current = self.query.with_ref(|q| q.top);
        let result = gx.modal(NumberEditorModal::new(current)).await;
        if let Some(val) = result {
            self.query.update(|q| {
                q.top = Some(val);
            });
            self.rebuild_tree();
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

    async fn fetch_field_options(
        &self,
        client: &DataverseClient,
        entity: &str,
    ) -> Result<Vec<(String, String)>, String> {
        let attrs = client
            .metadata()
            .attributes(Entity::set(entity))
            .await
            .map_err(|e| format!("Failed to load attributes: {}", e))?;

        Ok(attrs
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
            .collect())
    }

    /// Get the key of the currently focused tree node.
    fn focused_key(&self) -> Option<String> {
        self.tree_state.with_ref(|s| s.focused_key.clone())
    }

    /// Determine the filter group ID to add to, based on the focused key.
    fn focused_filter_group_id(&self, key: &str) -> Option<usize> {
        if let Some(id_str) = key.strip_prefix("filter-group-") {
            id_str.parse::<usize>().ok()
        } else if key == "section-Filter" {
            // Return the root group ID if it exists
            self.query.with_ref(|q| match &q.filter {
                FilterNode::Group { id, .. } => Some(*id),
                _ => None,
            })
        } else {
            // For filter-cond-*, add to root group
            self.query.with_ref(|q| match &q.filter {
                FilterNode::Group { id, .. } => Some(*id),
                _ => None,
            })
        }
    }

    /// Rebuild the tree widget state from the current QueryData.
    fn rebuild_tree(&self) {
        let nodes = self.query.with_ref(|q| build_tree(q));
        self.tree_state.update(|s| {
            s.set_roots(nodes);
        });
    }

    // =========================================================================
    // UI
    // =========================================================================

    fn element(&self) -> Element {
        let loading_message = self.loading_message.get();

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

                if let Some(msg) = loading_message {
                    { loading_overlay("loading-overlay", &msg) }
                }
            }
        }
    }
}
