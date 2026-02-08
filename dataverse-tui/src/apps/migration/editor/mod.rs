//! Migration editor app for editing a migration's phases and entity mappings.

mod config_operations;
mod detail_views;
mod entity_operations;
mod helpers;
mod item_operations;
mod phase_operations;
mod transform_operations;
mod tree;

use dataverse_lib::DataverseClient;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::Text;
use rafter::widgets::Tree;
use rafter::widgets::TreeState;
use crate::apps::migration::types::CoalesceChain;
use crate::apps::migration::types::EntityMapping;
use crate::apps::migration::types::FieldMapping;
use crate::apps::migration::types::FindCondition;
use crate::apps::migration::types::MatchBranch;
use crate::apps::migration::types::Migration;
use crate::apps::migration::types::Mode;
use crate::apps::migration::types::Phase;
use crate::apps::migration::types::Transform;
use crate::apps::migration::types::Variable;

use tree::MigrationTreeNode;

/// Migration editor app.
#[app(name = "Migration Editor", on_blur = Continue)]
pub struct MigrationEditor {
    /// The migration being edited.
    migration: Migration,
    /// Client for the source environment.
    source_client: DataverseClient,
    /// Client for the target environment.
    target_client: DataverseClient,
    /// Tree state for phases and entity mappings.
    tree_state: TreeState<MigrationTreeNode>,
    /// All phases (for tree building).
    phases: Vec<Phase>,
    /// All entity mappings (for tree building).
    entity_mappings: Vec<EntityMapping>,
    /// All variables (for tree building).
    variables: Vec<Variable>,
    /// All field mappings (for tree building).
    field_mappings: Vec<FieldMapping>,
    /// All transforms (for tree building).
    transforms: Vec<Transform>,
    /// All match branches (for tree building).
    match_branches: Vec<MatchBranch>,
    /// All coalesce chains (for tree building).
    coalesce_chains: Vec<CoalesceChain>,
    /// All find conditions (for tree building).
    find_conditions: Vec<FindCondition>,
}

impl MigrationEditor {
    /// Create a new editor for the given migration and clients.
    pub fn new_editor(
        migration: Migration,
        source_client: DataverseClient,
        target_client: DataverseClient,
    ) -> Self {
        Self::new(
            migration,
            source_client,
            target_client,
            TreeState::default(),
            Vec::new(), // phases
            Vec::new(), // entity_mappings
            Vec::new(), // variables
            Vec::new(), // field_mappings
            Vec::new(), // transforms
            Vec::new(), // match_branches
            Vec::new(), // coalesce_chains
            Vec::new(), // find_conditions
        )
    }
}

#[app_impl]
impl MigrationEditor {
    #[on_start]
    async fn on_start(&self, gx: &GlobalContext) {
        self.load_db_data(gx).await;
    }

    /// Reactive rebuild: auto-triggers whenever any of the 8 data Vec fields change.
    /// Fetches entity metadata, discovers navigation entities, builds the tree.
    #[watch]
    async fn rebuild(&self, gx: &GlobalContext) {
        use helpers::collect_navigation_paths;
        use helpers::discover_navigation_entities;
        use helpers::fetch_entity_field_types;
        use tree::build_tree_nodes;
        use tree::FieldTypeCache;

        // 1. Read all dependencies (registers for change detection)
        let phases = self.phases.get();
        let entity_mappings = self.entity_mappings.get();
        let variables = self.variables.get();
        let field_mappings = self.field_mappings.get();
        let transforms = self.transforms.get();
        let match_branches = self.match_branches.get();
        let coalesce_chains = self.coalesce_chains.get();
        let find_conditions = self.find_conditions.get();

        // 2. Collect unique entity names
        let mut source_entities: Vec<String> = Vec::new();
        let mut target_entities: Vec<String> = Vec::new();
        for em in entity_mappings.iter() {
            if !em.source_entity.is_empty() && !source_entities.contains(&em.source_entity) {
                source_entities.push(em.source_entity.clone());
            }
            if !em.target_entity.is_empty() && !target_entities.contains(&em.target_entity) {
                target_entities.push(em.target_entity.clone());
            }
        }

        // 3. Fetch metadata (DataverseClient has API-level cache with TTL)
        let source_client = self.source_client.get().clone();
        let target_client = self.target_client.get().clone();

        let mut source_field_types = if source_entities.is_empty() {
            FieldTypeCache::new()
        } else {
            log::debug!(
                "watch rebuild: fetching metadata for {} source entities",
                source_entities.len(),
            );
            let result: FieldTypeCache = gx
                .modal(crate::modals::LoadingModal::run(
                    "Loading source entity metadata...",
                    fetch_entity_field_types(source_client.clone(), source_entities),
                ))
                .await;
            result
        };

        // 4. Discover and fetch navigation entities (dotted copy paths + variable navigation)
        let nav_paths = collect_navigation_paths(&transforms, &entity_mappings, &variables);
        if !nav_paths.is_empty() {
            log::debug!(
                "watch rebuild: found {} navigation paths for entity scanning",
                nav_paths.len(),
            );
            loop {
                let nav_entities =
                    discover_navigation_entities(&nav_paths, &source_field_types);
                if nav_entities.is_empty() {
                    break;
                }
                log::debug!(
                    "watch rebuild: fetching metadata for {} navigation entities: {:?}",
                    nav_entities.len(),
                    nav_entities,
                );
                let result: FieldTypeCache = gx
                    .modal(crate::modals::LoadingModal::run(
                        "Loading navigation entity metadata...",
                        fetch_entity_field_types(source_client.clone(), nav_entities),
                    ))
                    .await;
                let fetched_any = !result.is_empty();
                for (entity, fields) in result {
                    source_field_types.insert(entity, fields);
                }
                if !fetched_any {
                    break;
                }
            }
        }

        let target_field_types = if target_entities.is_empty() {
            FieldTypeCache::new()
        } else {
            log::debug!(
                "watch rebuild: fetching metadata for {} target entities",
                target_entities.len(),
            );
            let result: FieldTypeCache = gx
                .modal(crate::modals::LoadingModal::run(
                    "Loading target entity metadata...",
                    fetch_entity_field_types(target_client, target_entities),
                ))
                .await;
            result
        };

        // 5. Build tree — type tracking is embedded in tree nodes
        let nodes = build_tree_nodes(
            phases,
            entity_mappings,
            variables,
            field_mappings,
            transforms,
            match_branches,
            coalesce_chains,
            find_conditions,
            &source_field_types,
            &target_field_types,
        );

        // 6. Update tree
        self.tree_state.update(|s| {
            s.set_roots(nodes);
            s.expand_all();
        });
    }

    fn title(&self) -> String {
        format!("Edit: {}", self.migration.get().name)
    }

    // =========================================================================
    // Keybinds
    // =========================================================================

    #[keybinds]
    fn keybinds() {
        bind("escape", close_app);
        bind("a", add_item);
        bind("d", delete_item);
        bind("J", move_item_down);
        bind("K", move_item_up);
    }

    #[handler]
    async fn close_app(&self, cx: &AppContext, gx: &GlobalContext) {
        let confirmed = gx
            .modal(crate::modals::ConfirmModal::with_message(
                "Close the migration editor?",
            ))
            .await;

        if confirmed {
            cx.close();
        }
    }

    #[handler]
    async fn add_item(&self, gx: &GlobalContext) {
        let focused_node = self.tree_state.with_ref(|s| {
            s.focused_key
                .as_ref()
                .and_then(|key| s.find_node(key))
                .map(|node| node.value.clone())
        });

        match focused_node {
            None => {
                // No selection -> add new phase
                self.add_phase_impl(gx).await;
            }
            Some(MigrationTreeNode::Phase(phase)) => {
                // Phase selected -> add entity mapping under it
                self.add_entity_mapping_impl(phase.id, gx).await;
            }
            Some(MigrationTreeNode::EntityMapping(em)) => {
                // Entity mapping selected -> add sibling entity mapping
                self.add_entity_mapping_impl(em.phase_id, gx).await;
            }
            Some(MigrationTreeNode::Variables { entity_mapping_id }) => {
                // Variables section -> add new variable
                self.add_variable_impl(entity_mapping_id, gx).await;
            }
            Some(MigrationTreeNode::Variable(_)) => {
                // Variable selected -> add transform to its chain
                self.add_transform_impl(gx).await;
            }
            Some(MigrationTreeNode::FieldMappings { entity_mapping_id }) => {
                // Field mappings section -> add new field mapping
                self.add_field_mapping_impl(entity_mapping_id, gx).await;
            }
            Some(MigrationTreeNode::FieldMapping(_)) => {
                // Field mapping selected -> add transform to its chain
                self.add_transform_impl(gx).await;
            }
            Some(MigrationTreeNode::Transform(..)) => {
                // Transform selected -> add transform after it in the chain
                self.add_transform_impl(gx).await;
            }
            Some(MigrationTreeNode::MatchBranch(_)) => {
                // Match branch selected -> add transform to the branch
                self.add_transform_impl(gx).await;
            }
            Some(MigrationTreeNode::CoalesceChain(_)) => {
                // Coalesce chain selected -> add transform to the chain
                self.add_transform_impl(gx).await;
            }
            Some(MigrationTreeNode::FindCondition(_)) => {
                // Find condition selected -> add transform to the condition
                self.add_transform_impl(gx).await;
            }
            Some(MigrationTreeNode::Chain { .. }) => {
                // Chain wrapper selected -> add transform to the chain
                self.add_transform_impl(gx).await;
            }
            // Other config nodes don't support adding children
            Some(_) => {}
        }
    }

    #[handler]
    async fn delete_item(&self, cx: &AppContext, gx: &GlobalContext) {
        let focused_node = self.tree_state.with_ref(|s| {
            s.focused_key
                .as_ref()
                .and_then(|key| s.find_node(key))
                .map(|node| node.value.clone())
        });

        match focused_node {
            Some(MigrationTreeNode::Phase(phase)) => {
                self.delete_phase_impl(phase.id, cx, gx).await;
            }
            Some(MigrationTreeNode::EntityMapping(em)) => {
                self.delete_entity_mapping_impl(em.id, cx, gx).await;
            }
            Some(MigrationTreeNode::Variable(vn)) => {
                self.delete_variable_impl(vn.variable.id, vn.variable.entity_mapping_id, cx, gx)
                    .await;
            }
            Some(MigrationTreeNode::FieldMapping(fmn)) => {
                self.delete_field_mapping_impl(fmn.field_mapping.id, fmn.field_mapping.entity_mapping_id, cx, gx)
                    .await;
            }
            Some(MigrationTreeNode::Transform(tn)) => {
                self.delete_transform_impl(&tn.transform, cx, gx).await;
            }
            // Other config nodes can't be deleted
            Some(_) | None => {}
        }
    }

    #[handler]
    async fn move_item_up(&self, gx: &GlobalContext) {
        log::debug!("move_item_up called");
        self.move_item(-1, gx).await;
    }

    #[handler]
    async fn move_item_down(&self, gx: &GlobalContext) {
        log::debug!("move_item_down called");
        self.move_item(1, gx).await;
    }

    async fn move_item(&self, direction: i32, gx: &GlobalContext) {
        let Some(focused) = self.focused_node() else {
            log::debug!("move_item: no focused node");
            return;
        };
        log::debug!("move_item: focused node = {:?}", focused);

        match focused {
            MigrationTreeNode::Variable(vn) => {
                self.reorder_variable_impl(vn.variable.id, vn.variable.entity_mapping_id, direction, gx)
                    .await;
            }
            MigrationTreeNode::FieldMapping(fmn) => {
                self.reorder_field_mapping_impl(fmn.field_mapping.id, fmn.field_mapping.entity_mapping_id, direction, gx)
                    .await;
            }
            MigrationTreeNode::Transform(tn) => {
                self.reorder_transform_impl(&tn.transform, direction, gx).await;
            }
            // Other nodes don't support reordering (yet)
            _ => {}
        }
    }

    #[handler]
    async fn edit_item(&self, gx: &GlobalContext) {
        let Some(focused) = self.focused_node() else {
            return;
        };

        match focused {
            MigrationTreeNode::Phase(phase) => {
                self.edit_phase_impl(&phase, gx).await;
            }
            MigrationTreeNode::EntityMapping(em) => {
                self.edit_entity_mapping_impl(&em, gx).await;
            }
            MigrationTreeNode::MatchConfig { entity_mapping_id } => {
                // TODO: Open match config editor
                let _ = entity_mapping_id;
            }
            MigrationTreeNode::SourceFilter { entity_mapping_id } => {
                self.edit_source_filter_impl(entity_mapping_id, gx).await;
            }
            MigrationTreeNode::TargetFilter { entity_mapping_id } => {
                self.edit_target_filter_impl(entity_mapping_id, gx).await;
            }
            MigrationTreeNode::UnmatchedHandling { entity_mapping_id } => {
                self.edit_unmatched_handling_impl(entity_mapping_id, gx)
                    .await;
            }
            MigrationTreeNode::Passes { entity_mapping_id } => {
                self.edit_passes_impl(entity_mapping_id, gx).await;
            }
            MigrationTreeNode::TestGuids { entity_mapping_id } => {
                self.edit_test_guids_impl(entity_mapping_id, gx).await;
            }
            MigrationTreeNode::Variables { entity_mapping_id } => {
                self.add_variable_impl(entity_mapping_id, gx).await;
            }
            MigrationTreeNode::Variable(vn) => {
                self.edit_variable_impl(&vn.variable, gx).await;
            }
            MigrationTreeNode::FieldMappings { .. } => {
                // Section header - no action, use 'a' to add
            }
            MigrationTreeNode::FieldMapping(_) => {
                self.add_transform_impl(gx).await;
            }
            MigrationTreeNode::Transform(tn) => {
                self.edit_transform_impl(&tn.transform, gx).await;
            }
            MigrationTreeNode::MatchBranch(_mb) => {
                // TODO: Open match branch editor (condition editor)
                gx.toast(Toast::info("Match branch editor not yet implemented"));
            }
            MigrationTreeNode::CoalesceChain(_cc) => {
                // Coalesce chains don't have configuration - just add transforms under them
            }
            MigrationTreeNode::FindCondition(_fc) => {
                // TODO: Open find condition editor (target_field edit)
                gx.toast(Toast::info("Find condition editor not yet implemented"));
            }
            MigrationTreeNode::Chain { .. } => {
                // Chain wrappers don't have configuration - just add transforms under them
            }
        }
    }

    fn element(&self) -> Element {
        let focused = self.focused_node();
        let has_selection = focused.is_some();
        let (can_add, add_label) = match &focused {
            None => (true, "Add Phase"),
            Some(MigrationTreeNode::Phase(_)) => (true, "Add Entity"),
            Some(MigrationTreeNode::EntityMapping(_)) => (true, "Add Entity"),
            Some(MigrationTreeNode::Variables { .. }) => (true, "Add Variable"),
            Some(MigrationTreeNode::Variable(_)) => (true, "Add Transform"),
            Some(MigrationTreeNode::FieldMappings { .. }) => (true, "Add Field"),
            Some(MigrationTreeNode::FieldMapping(_)) => (true, "Add Transform"),
            Some(MigrationTreeNode::Transform(..)) => (true, "Add Transform"),
            Some(MigrationTreeNode::MatchBranch(_)) => (true, "Add Transform"),
            Some(MigrationTreeNode::CoalesceChain(_)) => (true, "Add Transform"),
            Some(MigrationTreeNode::FindCondition(_)) => (true, "Add Transform"),
            Some(MigrationTreeNode::Chain { .. }) => (true, "Add Transform"),
            Some(_) => (false, "Add"), // Other config nodes - can't add
        };

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: background) {
                text (content: {self.title()}) style (bold, fg: interact)

                row (width: fill, height: fill) {
                    row (width: fill, height: fill) {
                        box_ (id: "migration-tree-container", height: fill, width: fill) style (bg: surface) {
                            tree (state: self.tree_state, id: "migration-tree", width: fill, height: fill)
                                on_activate: edit_item()
                        }
                        column (width: 1)
                    }

                    column (padding: 1, width: fill, height: fill) style (bg: surface) {
                        match focused {
                            None => {
                                column (width: fill, height: fill, justify: center, align: center) {
                                    text (content: "Select a phase or entity mapping") style (fg: muted)
                                }
                            }
                            Some(MigrationTreeNode::Phase(phase)) => {
                                column (gap: 1) {
                                    text (content: "Phase") style (bold, fg: interact)
                                    column {
                                        row (gap: 1) {
                                            text (content: "Name") style (fg: muted)
                                            text (content: {phase.name.clone()})
                                        }
                                        row (gap: 1) {
                                            text (content: "Mode") style (fg: muted)
                                            text (content: {if phase.mode == Mode::Lua { "Lua" } else { "Declarative" }})
                                        }
                                        row (gap: 1) {
                                            text (content: "Entities") style (fg: muted)
                                            text (content: {format!("{}", self.entity_count_for_phase(phase.id))})
                                        }
                                    }
                                }
                            }
                            Some(MigrationTreeNode::EntityMapping(em)) => {
                                column (gap: 1) {
                                    text (content: "Entity Mapping") style (bold, fg: interact)
                                    column {
                                        row (gap: 1) {
                                            text (content: "Name") style (fg: muted)
                                            text (content: {em.name.clone()})
                                        }
                                        row (gap: 1) {
                                            text (content: "Source") style (fg: muted)
                                            text (content: {em.source_entity.clone()})
                                        }
                                        row (gap: 1) {
                                            text (content: "Target") style (fg: muted)
                                            text (content: {em.target_entity.clone()})
                                        }
                                        row (gap: 1) {
                                            text (content: "Mode") style (fg: muted)
                                            text (content: {if em.mode == Mode::Lua { "Lua" } else { "Declarative" }})
                                        }
                                    }
                                }
                            }
                            Some(MigrationTreeNode::MatchConfig { entity_mapping_id }) => {
                                { self.render_config_detail("Match Config", entity_mapping_id, "Configure how source records are matched to target records") }
                            }
                            Some(MigrationTreeNode::SourceFilter { entity_mapping_id }) => {
                                { self.render_config_detail("Source Filter", entity_mapping_id, "Filter which source records to process") }
                            }
                            Some(MigrationTreeNode::TargetFilter { entity_mapping_id }) => {
                                { self.render_config_detail("Target Filter", entity_mapping_id, "Filter which target records to consider for matching") }
                            }
                            Some(MigrationTreeNode::UnmatchedHandling { entity_mapping_id }) => {
                                { self.render_config_detail("Unmatched Handling", entity_mapping_id, "Configure behavior for unmatched source and target records") }
                            }
                            Some(MigrationTreeNode::Passes { entity_mapping_id }) => {
                                { self.render_config_detail("Passes", entity_mapping_id, "Enable or disable migration passes (create, update, delete, etc.)") }
                            }
                            Some(MigrationTreeNode::TestGuids { entity_mapping_id }) => {
                                { self.render_config_detail("Test GUIDs", entity_mapping_id, "Specify record GUIDs to test the migration with") }
                            }
                            Some(MigrationTreeNode::Variables { entity_mapping_id }) => {
                                { self.render_variables_detail(entity_mapping_id) }
                            }
                            Some(MigrationTreeNode::Variable(vn)) => {
                                { self.render_variable_detail(&vn) }
                            }
                            Some(MigrationTreeNode::FieldMappings { entity_mapping_id }) => {
                                { self.render_field_mappings_detail(entity_mapping_id) }
                            }
                            Some(MigrationTreeNode::FieldMapping(fmn)) => {
                                { self.render_field_mapping_detail(&fmn) }
                            }
                            Some(MigrationTreeNode::Transform(tn)) => {
                                { self.render_transform_detail(&tn) }
                            }
                            Some(MigrationTreeNode::MatchBranch(mb)) => {
                                { self.render_match_branch_detail(&mb) }
                            }
                            Some(MigrationTreeNode::CoalesceChain(cc)) => {
                                { self.render_coalesce_chain_detail(&cc) }
                            }
                            Some(MigrationTreeNode::FindCondition(fc)) => {
                                { self.render_find_condition_detail(&fc) }
                            }
                            Some(MigrationTreeNode::Chain { parent_type, parent_id }) => {
                                { self.render_chain_detail(parent_type, parent_id) }
                            }
                        }
                    }
                }

                row (width: fill, justify: between) {
                    button (label: "Close", hint: "esc", id: "close-btn") on_activate: close_app()
                    row (gap: 1) {
                        button (label: {add_label}, hint: "a", id: "add-btn", disabled: {!can_add}) on_activate: add_item()
                        if has_selection {
                            button (label: "Edit", hint: "enter", id: "edit-btn") on_activate: edit_item()
                        }
                        if has_selection {
                            button (label: "Delete", hint: "d", id: "delete-btn") on_activate: delete_item()
                        }
                    }
                }
            }
        }
    }
}
