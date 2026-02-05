//! Migration editor app for editing a migration's phases and entity mappings.

mod config_operations;
mod detail_views;
mod entity_operations;
mod helpers;
mod item_operations;
mod phase_operations;
mod tree;

use dataverse_lib::DataverseClient;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::Text;
use rafter::widgets::Tree;
use rafter::widgets::TreeState;
use tuidom::Color;
use tuidom::Style;

use crate::apps::migration::repository::MigrationRepository;
use crate::apps::migration::types::EntityMapping;
use crate::apps::migration::types::FieldMapping;
use crate::apps::migration::types::Migration;
use crate::apps::migration::types::Mode;
use crate::apps::migration::types::Phase;
use crate::apps::migration::types::Variable;

use tree::build_tree_nodes;
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
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    }
}

#[app_impl]
impl MigrationEditor {
    #[on_start]
    async fn on_start(&self, gx: &GlobalContext) {
        let migration_id = self.migration.get().id;
        let repo = gx.data::<MigrationRepository>();

        // Load phases
        match repo.get_phases(migration_id).await {
            Ok(phases) => {
                self.phases.set(phases);
            }
            Err(e) => {
                log::error!("Failed to load phases: {}", e);
                gx.toast(Toast::error("Failed to load phases"));
            }
        }

        // Load entity mappings for all phases
        let phases = self.phases.get();
        let mut all_mappings = Vec::new();
        for phase in &phases {
            match repo.get_entity_mappings(phase.id).await {
                Ok(mappings) => {
                    all_mappings.extend(mappings);
                }
                Err(e) => {
                    log::error!("Failed to load entity mappings for phase {}: {}", phase.id, e);
                }
            }
        }
        self.entity_mappings.set(all_mappings);

        // Load variables and field mappings for all entity mappings
        let entity_mappings = self.entity_mappings.get();
        let mut all_variables = Vec::new();
        let mut all_field_mappings = Vec::new();
        for em in &entity_mappings {
            match repo.get_variables(em.id).await {
                Ok(vars) => all_variables.extend(vars),
                Err(e) => {
                    log::error!("Failed to load variables for entity mapping {}: {}", em.id, e);
                }
            }
            match repo.get_field_mappings(em.id).await {
                Ok(fms) => all_field_mappings.extend(fms),
                Err(e) => {
                    log::error!(
                        "Failed to load field mappings for entity mapping {}: {}",
                        em.id,
                        e
                    );
                }
            }
        }
        self.variables.set(all_variables);
        self.field_mappings.set(all_field_mappings);

        // Build tree
        self.rebuild_tree();
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
            Some(MigrationTreeNode::Variable(v)) => {
                // Variable selected -> add sibling variable
                self.add_variable_impl(v.entity_mapping_id, gx).await;
            }
            Some(MigrationTreeNode::FieldMappings { entity_mapping_id }) => {
                // Field mappings section -> add new field mapping
                self.add_field_mapping_impl(entity_mapping_id, gx).await;
            }
            Some(MigrationTreeNode::FieldMapping(fm)) => {
                // Field mapping selected -> add sibling field mapping
                self.add_field_mapping_impl(fm.entity_mapping_id, gx).await;
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
            Some(MigrationTreeNode::Variable(v)) => {
                self.delete_variable_impl(v.id, v.entity_mapping_id, cx, gx)
                    .await;
            }
            Some(MigrationTreeNode::FieldMapping(fm)) => {
                self.delete_field_mapping_impl(fm.id, fm.entity_mapping_id, cx, gx)
                    .await;
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
            MigrationTreeNode::Variable(v) => {
                self.reorder_variable_impl(v.id, v.entity_mapping_id, direction, gx)
                    .await;
            }
            MigrationTreeNode::FieldMapping(fm) => {
                self.reorder_field_mapping_impl(fm.id, fm.entity_mapping_id, direction, gx)
                    .await;
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
            MigrationTreeNode::Variables { .. } => {
                // Section header - no action, use 'a' to add
            }
            MigrationTreeNode::Variable(v) => {
                // TODO: Open transform chain editor
                gx.toast(Toast::info(format!(
                    "Transform editor for ${} not yet implemented",
                    v.name
                )));
            }
            MigrationTreeNode::FieldMappings { .. } => {
                // Section header - no action, use 'a' to add
            }
            MigrationTreeNode::FieldMapping(fm) => {
                // TODO: Open transform chain editor
                gx.toast(Toast::info(format!(
                    "Transform editor for '{}' not yet implemented",
                    fm.target_field
                )));
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
            Some(MigrationTreeNode::Variable(_)) => (true, "Add Variable"),
            Some(MigrationTreeNode::FieldMappings { .. }) => (true, "Add Field"),
            Some(MigrationTreeNode::FieldMapping(_)) => (true, "Add Field"),
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
                            Some(MigrationTreeNode::Variable(v)) => {
                                { self.render_variable_detail(&v) }
                            }
                            Some(MigrationTreeNode::FieldMappings { entity_mapping_id }) => {
                                { self.render_field_mappings_detail(entity_mapping_id) }
                            }
                            Some(MigrationTreeNode::FieldMapping(fm)) => {
                                { self.render_field_mapping_detail(&fm) }
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
