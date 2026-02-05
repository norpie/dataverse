//! Migration editor app for editing a migration's phases and entity mappings.

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

use crate::apps::migration::modals::EditPhaseModal;
use crate::apps::migration::modals::EditEntityMappingModal;
use crate::apps::migration::modals::NewPhaseModal;
use crate::apps::migration::repository::MigrationRepository;
use crate::apps::migration::repository::NewEntityMapping;
use crate::apps::migration::repository::NewPhase;
use crate::apps::migration::repository::UpdateEntityMapping;
use crate::apps::migration::repository::UpdatePhase;
use crate::apps::migration::types::EntityMapping;
use crate::apps::migration::types::MatchStrategy;
use crate::apps::migration::types::Migration;
use crate::apps::migration::types::Mode;
use crate::apps::migration::types::NoMatchFallback;
use crate::apps::migration::types::OrphanStrategy;
use crate::apps::migration::types::Phase;
use crate::modals::parallel_load;

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
                self.add_phase(gx).await;
            }
            Some(MigrationTreeNode::Phase(phase)) => {
                // Phase selected -> add entity mapping under it
                self.add_entity_mapping_to_phase(phase.id, gx).await;
            }
            Some(MigrationTreeNode::EntityMapping(em)) => {
                // Entity mapping selected -> add sibling entity mapping
                self.add_entity_mapping_to_phase(em.phase_id, gx).await;
            }
            // Config nodes don't support adding children
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
                self.delete_phase(phase.id, cx, gx).await;
            }
            Some(MigrationTreeNode::EntityMapping(em)) => {
                self.delete_entity_mapping(em.id, cx, gx).await;
            }
            // Config nodes can't be deleted
            Some(_) | None => {}
        }
    }

    #[handler]
    async fn edit_item(&self, gx: &GlobalContext) {
        let Some(focused) = self.focused_node() else {
            return;
        };

        match focused {
            MigrationTreeNode::Phase(phase) => {
                self.edit_phase(&phase, gx).await;
            }
            MigrationTreeNode::EntityMapping(em) => {
                self.edit_entity_mapping(&em, gx).await;
            }
            MigrationTreeNode::MatchConfig { entity_mapping_id } => {
                // TODO: Open match config editor
                let _ = entity_mapping_id;
            }
            MigrationTreeNode::SourceFilter { entity_mapping_id } => {
                // TODO: Open source filter editor
                let _ = entity_mapping_id;
            }
            MigrationTreeNode::TargetFilter { entity_mapping_id } => {
                // TODO: Open target filter editor
                let _ = entity_mapping_id;
            }
            MigrationTreeNode::UnmatchedHandling { entity_mapping_id } => {
                // TODO: Open unmatched handling editor
                let _ = entity_mapping_id;
            }
            MigrationTreeNode::Passes { entity_mapping_id } => {
                // TODO: Open passes editor
                let _ = entity_mapping_id;
            }
            MigrationTreeNode::TestGuids { entity_mapping_id } => {
                // TODO: Open test guids editor
                let _ = entity_mapping_id;
            }
        }
    }

    // =========================================================================
    // Phase Operations
    // =========================================================================

    async fn edit_phase(&self, phase: &Phase, gx: &GlobalContext) {
        let Some(result) = gx.modal(EditPhaseModal::for_phase(phase)).await else {
            return;
        };

        let repo = gx.data::<MigrationRepository>();
        let update = UpdatePhase {
            name: Some(result.name),
            mode: Some(result.mode),
            lua_script: result.lua_script,
        };

        match repo.update_phase(phase.id, update).await {
            Ok(()) => {
                self.refresh_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to update phase: {}", e);
                gx.toast(Toast::error("Failed to update phase"));
            }
        }
    }

    async fn edit_entity_mapping(&self, em: &EntityMapping, gx: &GlobalContext) {
        // Fetch entity lists from both environments in parallel
        let source_client = self.source_client.get();
        let target_client = self.target_client.get();

        let (source_result, target_result) = parallel_load!(gx, {
            "Loading source entities" => async move {
                source_client
                    .metadata()
                    .all_entities()
                    .await
                    .map(|entities| {
                        entities.into_iter().map(|e| e.logical_name).collect::<Vec<_>>()
                    })
            },
            "Loading target entities" => async move {
                target_client
                    .metadata()
                    .all_entities()
                    .await
                    .map(|entities| {
                        entities.into_iter().map(|e| e.logical_name).collect::<Vec<_>>()
                    })
            },
        });

        let source_entities = match source_result {
            Some(Ok(entities)) => entities,
            Some(Err(e)) => {
                log::error!("Failed to fetch source entities: {}", e);
                gx.toast(Toast::error("Failed to fetch source entities"));
                return;
            }
            None => {
                return;
            }
        };

        let target_entities = match target_result {
            Some(Ok(entities)) => entities,
            Some(Err(e)) => {
                log::error!("Failed to fetch target entities: {}", e);
                gx.toast(Toast::error("Failed to fetch target entities"));
                return;
            }
            None => {
                return;
            }
        };

        let Some(result) = gx
            .modal(EditEntityMappingModal::edit_mapping(
                em,
                source_entities,
                target_entities,
            ))
            .await
        else {
            return;
        };

        let repo = gx.data::<MigrationRepository>();
        let update = UpdateEntityMapping {
            name: Some(result.name),
            source_entity: Some(result.source_entity),
            target_entity: Some(result.target_entity),
            mode: Some(result.mode),
            lua_script: result.lua_script,
            match_strategy: None,
            match_find_config: None,
            no_match_fallback: None,
            orphan_strategy: None,
            create_pass_enabled: None,
            update_pass_enabled: None,
            source_filter: None,
            target_filter: None,
            test_guids: None,
        };

        match repo.update_entity_mapping(em.id, update).await {
            Ok(()) => {
                self.refresh_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to update entity mapping: {}", e);
                gx.toast(Toast::error("Failed to update entity mapping"));
            }
        }
    }

    async fn add_phase(&self, gx: &GlobalContext) {
        let Some(result) = gx.modal(NewPhaseModal::new_modal()).await else {
            return;
        };

        let repo = gx.data::<MigrationRepository>();
        let order = self.phases.get().len() as i32;

        let new_phase = NewPhase {
            migration_id: self.migration.get().id,
            order,
            name: result.name,
            mode: result.mode,
            lua_script: None,
        };

        match repo.create_phase(new_phase).await {
            Ok(_id) => {
                gx.toast(Toast::info("Phase created"));
                self.refresh_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to create phase: {}", e);
                gx.toast(Toast::error("Failed to create phase"));
            }
        }
    }

    async fn delete_phase(&self, phase_id: i64, cx: &AppContext, gx: &GlobalContext) {
        let confirmed = gx
            .modal(crate::modals::ConfirmModal::with_message(
                "Delete this phase and all its entity mappings?",
            ))
            .await;

        if !confirmed {
            return;
        }

        // Compute next focus before deletion
        let phases = self.phases.get();
        let current_idx = phases.iter().position(|p| p.id == phase_id);
        let next_focus = current_idx.and_then(|idx| {
            // Try previous phase, then next phase
            if idx > 0 {
                phases.get(idx - 1).map(|p| format!("phase-{}", p.id))
            } else {
                phases.get(idx + 1).map(|p| format!("phase-{}", p.id))
            }
        });

        let repo = gx.data::<MigrationRepository>();
        match repo.delete_phase(phase_id).await {
            Ok(()) => {
                gx.toast(Toast::info("Phase deleted"));
                self.refresh_data(gx).await;

                // Focus next item
                if let Some(key) = next_focus {
                    cx.focus(&format!("migration-tree-node-{}", key));
                }
            }
            Err(e) => {
                log::error!("Failed to delete phase: {}", e);
                gx.toast(Toast::error("Failed to delete phase"));
            }
        }
    }

    // =========================================================================
    // Entity Mapping Operations
    // =========================================================================

    async fn add_entity_mapping_to_phase(&self, phase_id: i64, gx: &GlobalContext) {
        // Fetch entity lists from both environments in parallel
        let source_client = self.source_client.get();
        let target_client = self.target_client.get();

        let (source_result, target_result) = parallel_load!(gx, {
            "Loading source entities" => async move {
                source_client
                    .metadata()
                    .all_entities()
                    .await
                    .map(|entities| {
                        entities.into_iter().map(|e| e.logical_name).collect::<Vec<_>>()
                    })
            },
            "Loading target entities" => async move {
                target_client
                    .metadata()
                    .all_entities()
                    .await
                    .map(|entities| {
                        entities.into_iter().map(|e| e.logical_name).collect::<Vec<_>>()
                    })
            },
        });

        let source_entities = match source_result {
            Some(Ok(entities)) => entities,
            Some(Err(e)) => {
                log::error!("Failed to fetch source entities: {}", e);
                gx.toast(Toast::error("Failed to fetch source entities"));
                return;
            }
            None => {
                return;
            }
        };

        let target_entities = match target_result {
            Some(Ok(entities)) => entities,
            Some(Err(e)) => {
                log::error!("Failed to fetch target entities: {}", e);
                gx.toast(Toast::error("Failed to fetch target entities"));
                return;
            }
            None => {
                return;
            }
        };

        // Show modal
        let Some(result) = gx
            .modal(EditEntityMappingModal::new_mapping(
                source_entities,
                target_entities,
            ))
            .await
        else {
            return;
        };

        // Create entity mapping
        let repo = gx.data::<MigrationRepository>();
        let order = self
            .entity_mappings
            .get()
            .iter()
            .filter(|em| em.phase_id == phase_id)
            .count() as i32;

        let lua_script = match result.lua_script {
            crate::apps::migration::repository::Update::Keep => None,
            crate::apps::migration::repository::Update::Set(s) => Some(s),
            crate::apps::migration::repository::Update::Clear => None,
        };

        let new_mapping = NewEntityMapping {
            phase_id,
            order,
            name: result.name,
            source_entity: result.source_entity,
            target_entity: result.target_entity,
            mode: result.mode,
            lua_script,
            match_strategy: MatchStrategy::SameId,
            match_find_config: None,
            no_match_fallback: NoMatchFallback::Error,
            orphan_strategy: OrphanStrategy::Ignore,
            create_pass_enabled: true,
            update_pass_enabled: true,
            source_filter: None,
            target_filter: None,
            test_guids: None,
        };

        match repo.create_entity_mapping(new_mapping).await {
            Ok(_id) => {
                gx.toast(Toast::info("Entity mapping created"));
                self.refresh_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to create entity mapping: {}", e);
                gx.toast(Toast::error("Failed to create entity mapping"));
            }
        }
    }

    async fn delete_entity_mapping(&self, entity_id: i64, cx: &AppContext, gx: &GlobalContext) {
        // Find parent phase and sibling info before deletion
        let entity_mappings = self.entity_mappings.get();
        let current = entity_mappings.iter().find(|em| em.id == entity_id);

        let (phase_id, next_focus) = if let Some(em) = current {
            let phase_id = em.phase_id;
            let siblings: Vec<_> = entity_mappings
                .iter()
                .filter(|e| e.phase_id == phase_id)
                .collect();
            let current_idx = siblings.iter().position(|e| e.id == entity_id);

            let next = current_idx.and_then(|idx| {
                // Try previous sibling, then next sibling, then parent phase
                if idx > 0 {
                    siblings.get(idx - 1).map(|e| format!("entity-{}", e.id))
                } else if idx + 1 < siblings.len() {
                    siblings.get(idx + 1).map(|e| format!("entity-{}", e.id))
                } else {
                    Some(format!("phase-{}", phase_id))
                }
            });

            (phase_id, next)
        } else {
            return;
        };

        let confirmed = gx
            .modal(crate::modals::ConfirmModal::with_message(
                "Delete this entity mapping?",
            ))
            .await;

        if !confirmed {
            return;
        }

        let repo = gx.data::<MigrationRepository>();
        match repo.delete_entity_mapping(entity_id).await {
            Ok(()) => {
                gx.toast(Toast::info("Entity mapping deleted"));
                self.refresh_data(gx).await;

                // Focus next item
                if let Some(key) = next_focus {
                    cx.focus(&format!("migration-tree-node-{}", key));
                }
            }
            Err(e) => {
                log::error!("Failed to delete entity mapping: {}", e);
                gx.toast(Toast::error("Failed to delete entity mapping"));
            }
        }
    }

    // =========================================================================
    // Internal
    // =========================================================================

    async fn refresh_data(&self, gx: &GlobalContext) {
        let migration_id = self.migration.get().id;
        let repo = gx.data::<MigrationRepository>();

        // Reload phases
        if let Ok(phases) = repo.get_phases(migration_id).await {
            self.phases.set(phases);
        }

        // Reload entity mappings
        let phases = self.phases.get();
        let mut all_mappings = Vec::new();
        for phase in &phases {
            if let Ok(mappings) = repo.get_entity_mappings(phase.id).await {
                all_mappings.extend(mappings);
            }
        }
        self.entity_mappings.set(all_mappings);

        self.rebuild_tree();
    }

    fn rebuild_tree(&self) {
        let phases = self.phases.get();
        let entity_mappings = self.entity_mappings.get();

        let nodes = build_tree_nodes(phases, entity_mappings);
        self.tree_state.update(|s| {
            s.set_roots(nodes);
        });
    }

    fn focused_node(&self) -> Option<MigrationTreeNode> {
        self.tree_state.with_ref(|s| {
            s.focused_key
                .as_ref()
                .and_then(|key| s.find_node(key))
                .map(|node| node.value.clone())
        })
    }

    fn entity_count_for_phase(&self, phase_id: i64) -> usize {
        self.entity_mappings
            .get()
            .iter()
            .filter(|em| em.phase_id == phase_id)
            .count()
    }

    fn render_config_detail(&self, title: &str, entity_mapping_id: i64, description: &str) -> Element {
        let em = self
            .entity_mappings
            .get()
            .iter()
            .find(|em| em.id == entity_mapping_id)
            .cloned();

        let parent_name = em.map(|e| e.name).unwrap_or_else(|| "Unknown".to_string());

        Element::col()
            .gap(1)
            .child(Element::text(title).style(Style::new().bold().foreground(Color::var("interact"))))
            .child(
                Element::col()
                    .child(
                        Element::row()
                            .gap(1)
                            .child(Element::text("Parent").style(Style::new().foreground(Color::var("muted"))))
                            .child(Element::text(&parent_name)),
                    )
                    .child(
                        Element::text(description).style(Style::new().foreground(Color::var("muted"))),
                    ),
            )
    }

    fn element(&self) -> Element {
        let focused = self.focused_node();
        let has_selection = focused.is_some();
        let add_label = if has_selection { "Add Entity" } else { "Add Phase" };

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
                        }
                    }
                }

                row (width: fill, justify: between) {
                    button (label: "Close", hint: "esc", id: "close-btn") on_activate: close_app()
                    row (gap: 1) {
                        button (label: {add_label}, hint: "a", id: "add-btn") on_activate: add_item()
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
