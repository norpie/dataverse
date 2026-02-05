//! Migration editor app for editing a migration's phases and entity mappings.

mod tree;

use dataverse_lib::error::Error as DataverseError;
use dataverse_lib::DataverseClient;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::Text;
use rafter::widgets::Tree;
use rafter::widgets::TreeState;
use tuidom::Color;
use tuidom::Style;

use crate::apps::migration::modals::AddFieldMappingModal;
use crate::apps::migration::modals::AddVariableModal;
use crate::apps::migration::modals::EditEntityMappingModal;
use crate::apps::migration::modals::EditPhaseModal;
use crate::apps::migration::modals::NewPhaseModal;
use crate::apps::migration::modals::PassesModal;
use crate::apps::migration::modals::TestGuidsModal;
use crate::apps::migration::modals::UnmatchedHandlingModal;
use crate::apps::migration::repository::MigrationRepository;
use crate::apps::migration::repository::NewEntityMapping;
use crate::apps::migration::repository::NewFieldMapping;
use crate::apps::migration::repository::NewPhase;
use crate::apps::migration::repository::NewVariable;
use crate::apps::migration::repository::UpdateEntityMapping;
use crate::apps::migration::repository::UpdatePhase;
use crate::modals::FilterBuilderModal;
use crate::modals::LoadingModal;
use crate::widgets::filter_builder::FilterNode;
use crate::apps::migration::types::EntityMapping;
use crate::apps::migration::types::FieldMapping;
use crate::apps::migration::types::MatchStrategy;
use crate::apps::migration::types::Migration;
use crate::apps::migration::types::Mode;
use crate::apps::migration::types::NoMatchFallback;
use crate::apps::migration::types::OrphanStrategy;
use crate::apps::migration::types::Phase;
use crate::apps::migration::types::Variable;
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
            Some(MigrationTreeNode::Variables { entity_mapping_id }) => {
                // Variables section -> add new variable
                self.add_variable(entity_mapping_id, gx).await;
            }
            Some(MigrationTreeNode::Variable(v)) => {
                // Variable selected -> add sibling variable
                self.add_variable(v.entity_mapping_id, gx).await;
            }
            Some(MigrationTreeNode::FieldMappings { entity_mapping_id }) => {
                // Field mappings section -> add new field mapping
                self.add_field_mapping(entity_mapping_id, gx).await;
            }
            Some(MigrationTreeNode::FieldMapping(fm)) => {
                // Field mapping selected -> add sibling field mapping
                self.add_field_mapping(fm.entity_mapping_id, gx).await;
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
                self.delete_phase(phase.id, cx, gx).await;
            }
            Some(MigrationTreeNode::EntityMapping(em)) => {
                self.delete_entity_mapping(em.id, cx, gx).await;
            }
            Some(MigrationTreeNode::Variable(v)) => {
                self.delete_variable(v.id, v.entity_mapping_id, cx, gx).await;
            }
            Some(MigrationTreeNode::FieldMapping(fm)) => {
                self.delete_field_mapping(fm.id, fm.entity_mapping_id, cx, gx)
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
                self.reorder_variable(v.id, v.entity_mapping_id, direction, gx)
                    .await;
            }
            MigrationTreeNode::FieldMapping(fm) => {
                self.reorder_field_mapping(fm.id, fm.entity_mapping_id, direction, gx)
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
                self.edit_source_filter(entity_mapping_id, gx).await;
            }
            MigrationTreeNode::TargetFilter { entity_mapping_id } => {
                self.edit_target_filter(entity_mapping_id, gx).await;
            }
            MigrationTreeNode::UnmatchedHandling { entity_mapping_id } => {
                self.edit_unmatched_handling(entity_mapping_id, gx).await;
            }
            MigrationTreeNode::Passes { entity_mapping_id } => {
                self.edit_passes(entity_mapping_id, gx).await;
            }
            MigrationTreeNode::TestGuids { entity_mapping_id } => {
                self.edit_test_guids(entity_mapping_id, gx).await;
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

    // =========================================================================
    // Phase Operations
    // =========================================================================

    async fn edit_phase(&self, phase: &Phase, gx: &GlobalContext) {
        let Some(result) = gx.modal(EditPhaseModal::for_phase(phase)).await else {
            return;
        };

        let repo = gx.data::<MigrationRepository>();
        let update = match result {
            crate::apps::migration::modals::EditPhaseResult::Declarative { name } => {
                UpdatePhase {
                    name: Some(name),
                    mode: Some(Mode::Declarative),
                    lua_script: crate::apps::migration::repository::Update::Clear,
                }
            }
            crate::apps::migration::modals::EditPhaseResult::Lua { name, lua_script } => {
                UpdatePhase {
                    name: Some(name),
                    mode: Some(Mode::Lua),
                    lua_script: crate::apps::migration::repository::Update::Set(lua_script),
                }
            }
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
        let update = match result {
            crate::apps::migration::modals::EntityMappingResult::Declarative {
                name,
                source_entity,
                target_entity,
            } => UpdateEntityMapping {
                name: Some(name),
                source_entity: Some(source_entity),
                target_entity: Some(target_entity),
                mode: Some(Mode::Declarative),
                lua_script: crate::apps::migration::repository::Update::Clear,
                match_strategy: None,
                match_find_config: None,
                no_match_fallback: None,
                orphan_strategy: None,
                create_pass_enabled: None,
                update_pass_enabled: None,
                delete_pass_enabled: None,
                deactivate_pass_enabled: None,
                associate_pass_enabled: None,
                disassociate_pass_enabled: None,
                source_filter: None,
                target_filter: None,
                test_guids: None,
            },
            crate::apps::migration::modals::EntityMappingResult::Lua { name, lua_script } => {
                UpdateEntityMapping {
                    name: Some(name),
                    source_entity: Some(String::new()),
                    target_entity: Some(String::new()),
                    mode: Some(Mode::Lua),
                    lua_script: crate::apps::migration::repository::Update::Set(lua_script),
                    match_strategy: None,
                    match_find_config: None,
                    no_match_fallback: None,
                    orphan_strategy: None,
                    create_pass_enabled: None,
                    update_pass_enabled: None,
                    delete_pass_enabled: None,
                    deactivate_pass_enabled: None,
                    associate_pass_enabled: None,
                    disassociate_pass_enabled: None,
                    source_filter: None,
                    target_filter: None,
                    test_guids: None,
                }
            }
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

        let new_mapping = match result {
            crate::apps::migration::modals::EntityMappingResult::Declarative {
                name,
                source_entity,
                target_entity,
            } => NewEntityMapping {
                phase_id,
                order,
                name,
                source_entity,
                target_entity,
                mode: Mode::Declarative,
                lua_script: None,
                match_strategy: MatchStrategy::SameId,
                match_find_config: None,
                no_match_fallback: NoMatchFallback::Create,
                orphan_strategy: OrphanStrategy::Ignore,
                create_pass_enabled: true,
                update_pass_enabled: true,
                delete_pass_enabled: true,
                deactivate_pass_enabled: true,
                associate_pass_enabled: true,
                disassociate_pass_enabled: true,
                source_filter: None,
                target_filter: None,
                test_guids: None,
            },
            crate::apps::migration::modals::EntityMappingResult::Lua { name, lua_script } => {
                NewEntityMapping {
                    phase_id,
                    order,
                    name,
                    source_entity: String::new(),
                    target_entity: String::new(),
                    mode: Mode::Lua,
                    lua_script: Some(lua_script),
                    match_strategy: MatchStrategy::SameId,
                    match_find_config: None,
                    no_match_fallback: NoMatchFallback::Create,
                    orphan_strategy: OrphanStrategy::Ignore,
                    create_pass_enabled: true,
                    update_pass_enabled: true,
                    delete_pass_enabled: true,
                    deactivate_pass_enabled: true,
                    associate_pass_enabled: true,
                    disassociate_pass_enabled: true,
                    source_filter: None,
                    target_filter: None,
                    test_guids: None,
                }
            }
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
    // Config Node Operations
    // =========================================================================

    async fn edit_test_guids(&self, entity_mapping_id: i64, gx: &GlobalContext) {
        // Find the entity mapping
        let entity_mappings = self.entity_mappings.get();
        let Some(em) = entity_mappings
            .iter()
            .find(|em| em.id == entity_mapping_id)
        else {
            return;
        };

        // Get current test GUIDs
        let initial_guids = em.test_guids.clone().unwrap_or_default();

        // Show modal
        let Some(result) = gx
            .modal(TestGuidsModal::new_modal(entity_mapping_id, initial_guids))
            .await
        else {
            return;
        };

        // Update entity mapping
        let repo = gx.data::<MigrationRepository>();
        let update = UpdateEntityMapping {
            name: None,
            source_entity: None,
            target_entity: None,
            mode: None,
            lua_script: crate::apps::migration::repository::Update::Keep,
            match_strategy: None,
            match_find_config: None,
            no_match_fallback: None,
            orphan_strategy: None,
            create_pass_enabled: None,
            update_pass_enabled: None,
            delete_pass_enabled: None,
            deactivate_pass_enabled: None,
            associate_pass_enabled: None,
            disassociate_pass_enabled: None,
            source_filter: None,
            target_filter: None,
            test_guids: Some(result),
        };

        match repo.update_entity_mapping(entity_mapping_id, update).await {
            Ok(()) => {
                gx.toast(Toast::info("Test GUIDs updated"));
                self.refresh_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to update test GUIDs: {}", e);
                gx.toast(Toast::error("Failed to update test GUIDs"));
            }
        }
    }

    async fn edit_passes(&self, entity_mapping_id: i64, gx: &GlobalContext) {
        // Find the entity mapping
        let entity_mappings = self.entity_mappings.get();
        let Some(em) = entity_mappings
            .iter()
            .find(|em| em.id == entity_mapping_id)
        else {
            return;
        };

        // Show modal
        let Some(result) = gx
            .modal(PassesModal::new_modal(
                entity_mapping_id,
                em.create_pass_enabled,
                em.update_pass_enabled,
                em.delete_pass_enabled,
                em.deactivate_pass_enabled,
                em.associate_pass_enabled,
                em.disassociate_pass_enabled,
            ))
            .await
        else {
            return;
        };

        // Update entity mapping
        let repo = gx.data::<MigrationRepository>();
        let update = UpdateEntityMapping {
            name: None,
            source_entity: None,
            target_entity: None,
            mode: None,
            lua_script: crate::apps::migration::repository::Update::Keep,
            match_strategy: None,
            match_find_config: None,
            no_match_fallback: None,
            orphan_strategy: None,
            create_pass_enabled: Some(result.create_pass),
            update_pass_enabled: Some(result.update_pass),
            delete_pass_enabled: Some(result.delete_pass),
            deactivate_pass_enabled: Some(result.deactivate_pass),
            associate_pass_enabled: Some(result.associate_pass),
            disassociate_pass_enabled: Some(result.disassociate_pass),
            source_filter: None,
            target_filter: None,
            test_guids: None,
        };

        match repo.update_entity_mapping(entity_mapping_id, update).await {
            Ok(()) => {
                gx.toast(Toast::info("Passes updated"));
                self.refresh_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to update passes: {}", e);
                gx.toast(Toast::error("Failed to update passes"));
            }
        }
    }

    async fn edit_unmatched_handling(&self, entity_mapping_id: i64, gx: &GlobalContext) {
        // Find the entity mapping
        let entity_mappings = self.entity_mappings.get();
        let Some(em) = entity_mappings
            .iter()
            .find(|em| em.id == entity_mapping_id)
        else {
            return;
        };

        // Show modal
        let Some(result) = gx
            .modal(UnmatchedHandlingModal::new_modal(
                entity_mapping_id,
                em.no_match_fallback,
                em.orphan_strategy,
            ))
            .await
        else {
            return;
        };

        // Update entity mapping
        let repo = gx.data::<MigrationRepository>();
        let update = UpdateEntityMapping {
            name: None,
            source_entity: None,
            target_entity: None,
            mode: None,
            lua_script: crate::apps::migration::repository::Update::Keep,
            match_strategy: None,
            match_find_config: None,
            no_match_fallback: Some(result.no_match_fallback),
            orphan_strategy: Some(result.orphan_strategy),
            create_pass_enabled: None,
            update_pass_enabled: None,
            delete_pass_enabled: None,
            deactivate_pass_enabled: None,
            associate_pass_enabled: None,
            disassociate_pass_enabled: None,
            source_filter: None,
            target_filter: None,
            test_guids: None,
        };

        match repo.update_entity_mapping(entity_mapping_id, update).await {
            Ok(()) => {
                gx.toast(Toast::info("Unmatched handling updated"));
                self.refresh_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to update unmatched handling: {}", e);
                gx.toast(Toast::error("Failed to update unmatched handling"));
            }
        }
    }

    async fn edit_source_filter(&self, entity_mapping_id: i64, gx: &GlobalContext) {
        let entity_mappings = self.entity_mappings.get();
        let Some(em) = entity_mappings.iter().find(|em| em.id == entity_mapping_id) else {
            return;
        };

        let entity_name = em.source_entity.clone();
        let current_filter = em.source_filter.clone();

        // Fetch attributes for the source entity
        let client = self.source_client.get();
        let entity_name_clone = entity_name.clone();
        let attributes = gx
            .modal(LoadingModal::run_with_default(
                "Loading entity metadata...",
                || Err(DataverseError::Cancelled),
                async move { client.metadata().attributes(entity_name_clone).await },
            ))
            .await;

        let attributes = match attributes {
            Ok(attrs) => attrs,
            Err(e) if e.is_cancelled() => return,
            Err(e) => {
                log::error!("Failed to fetch attributes for {}: {}", entity_name, e);
                gx.toast(Toast::error("Failed to fetch entity metadata"));
                return;
            }
        };

        // Build options for autocomplete
        let options: Vec<(String, String)> = attributes
            .iter()
            .map(|a| {
                let display = a.display_name.text_or(&a.logical_name).to_string();
                (a.logical_name.clone(), display)
            })
            .collect();

        // Open filter builder modal
        let result = gx
            .modal(FilterBuilderModal::new_modal(
                "Source Filter",
                options,
                attributes,
                current_filter,
            ))
            .await;

        let Some(filter) = result else {
            return;
        };

        // Convert Empty to None for storage
        let filter_to_store = if let FilterNode::Empty = filter {
            None
        } else {
            Some(filter)
        };

        // Update entity mapping
        let repo = gx.data::<MigrationRepository>();
        let update = UpdateEntityMapping {
            name: None,
            source_entity: None,
            target_entity: None,
            mode: None,
            lua_script: crate::apps::migration::repository::Update::Keep,
            match_strategy: None,
            match_find_config: None,
            no_match_fallback: None,
            orphan_strategy: None,
            create_pass_enabled: None,
            update_pass_enabled: None,
            delete_pass_enabled: None,
            deactivate_pass_enabled: None,
            associate_pass_enabled: None,
            disassociate_pass_enabled: None,
            source_filter: filter_to_store,
            target_filter: None,
            test_guids: None,
        };

        match repo.update_entity_mapping(entity_mapping_id, update).await {
            Ok(()) => {
                gx.toast(Toast::info("Source filter updated"));
                self.refresh_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to update source filter: {}", e);
                gx.toast(Toast::error("Failed to update source filter"));
            }
        }
    }

    async fn edit_target_filter(&self, entity_mapping_id: i64, gx: &GlobalContext) {
        let entity_mappings = self.entity_mappings.get();
        let Some(em) = entity_mappings.iter().find(|em| em.id == entity_mapping_id) else {
            return;
        };

        let entity_name = em.target_entity.clone();
        let current_filter = em.target_filter.clone();

        // Fetch attributes for the target entity
        let client = self.target_client.get();
        let entity_name_clone = entity_name.clone();
        let attributes = gx
            .modal(LoadingModal::run_with_default(
                "Loading entity metadata...",
                || Err(DataverseError::Cancelled),
                async move { client.metadata().attributes(entity_name_clone).await },
            ))
            .await;

        let attributes = match attributes {
            Ok(attrs) => attrs,
            Err(e) if e.is_cancelled() => return,
            Err(e) => {
                log::error!("Failed to fetch attributes for {}: {}", entity_name, e);
                gx.toast(Toast::error("Failed to fetch entity metadata"));
                return;
            }
        };

        // Build options for autocomplete
        let options: Vec<(String, String)> = attributes
            .iter()
            .map(|a| {
                let display = a.display_name.text_or(&a.logical_name).to_string();
                (a.logical_name.clone(), display)
            })
            .collect();

        // Open filter builder modal
        let result = gx
            .modal(FilterBuilderModal::new_modal(
                "Target Filter",
                options,
                attributes,
                current_filter,
            ))
            .await;

        let Some(filter) = result else {
            return;
        };

        // Convert Empty to None for storage
        let filter_to_store = if let FilterNode::Empty = filter {
            None
        } else {
            Some(filter)
        };

        // Update entity mapping
        let repo = gx.data::<MigrationRepository>();
        let update = UpdateEntityMapping {
            name: None,
            source_entity: None,
            target_entity: None,
            mode: None,
            lua_script: crate::apps::migration::repository::Update::Keep,
            match_strategy: None,
            match_find_config: None,
            no_match_fallback: None,
            orphan_strategy: None,
            create_pass_enabled: None,
            update_pass_enabled: None,
            delete_pass_enabled: None,
            deactivate_pass_enabled: None,
            associate_pass_enabled: None,
            disassociate_pass_enabled: None,
            source_filter: None,
            target_filter: filter_to_store,
            test_guids: None,
        };

        match repo.update_entity_mapping(entity_mapping_id, update).await {
            Ok(()) => {
                gx.toast(Toast::info("Target filter updated"));
                self.refresh_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to update target filter: {}", e);
                gx.toast(Toast::error("Failed to update target filter"));
            }
        }
    }

    // =========================================================================
    // Variable Operations
    // =========================================================================

    async fn add_variable(&self, entity_mapping_id: i64, gx: &GlobalContext) {
        let Some(result) = gx.modal(AddVariableModal::new_modal()).await else {
            return;
        };

        let repo = gx.data::<MigrationRepository>();
        let order = self
            .variables
            .get()
            .iter()
            .filter(|v| v.entity_mapping_id == entity_mapping_id)
            .count() as i32;

        let new_variable = NewVariable {
            entity_mapping_id,
            order,
            name: result.name,
        };

        match repo.create_variable(new_variable).await {
            Ok(_id) => {
                gx.toast(Toast::info("Variable created"));
                self.refresh_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to create variable: {}", e);
                gx.toast(Toast::error("Failed to create variable"));
            }
        }
    }

    async fn delete_variable(
        &self,
        variable_id: i64,
        entity_mapping_id: i64,
        cx: &AppContext,
        gx: &GlobalContext,
    ) {
        // Compute next focus before deletion
        let variables = self.variables.get();
        let siblings: Vec<_> = variables
            .iter()
            .filter(|v| v.entity_mapping_id == entity_mapping_id)
            .collect();
        let current_idx = siblings.iter().position(|v| v.id == variable_id);

        let next_focus = current_idx.and_then(|idx| {
            if idx > 0 {
                siblings.get(idx - 1).map(|v| format!("variable-{}", v.id))
            } else if idx + 1 < siblings.len() {
                siblings.get(idx + 1).map(|v| format!("variable-{}", v.id))
            } else {
                Some(format!("variables-{}", entity_mapping_id))
            }
        });

        let confirmed = gx
            .modal(crate::modals::ConfirmModal::with_message(
                "Delete this variable?",
            ))
            .await;

        if !confirmed {
            return;
        }

        let repo = gx.data::<MigrationRepository>();
        match repo.delete_variable(variable_id).await {
            Ok(()) => {
                gx.toast(Toast::info("Variable deleted"));
                self.refresh_data(gx).await;

                if let Some(key) = next_focus {
                    cx.focus(&format!("migration-tree-node-{}", key));
                }
            }
            Err(e) => {
                log::error!("Failed to delete variable: {}", e);
                gx.toast(Toast::error("Failed to delete variable"));
            }
        }
    }

    async fn reorder_variable(
        &self,
        variable_id: i64,
        entity_mapping_id: i64,
        direction: i32,
        gx: &GlobalContext,
    ) {
        let variables = self.variables.get();
        let mut siblings: Vec<_> = variables
            .iter()
            .filter(|v| v.entity_mapping_id == entity_mapping_id)
            .collect();
        siblings.sort_by_key(|v| v.order);

        let Some(current_idx) = siblings.iter().position(|v| v.id == variable_id) else {
            return;
        };

        let new_idx = (current_idx as i32 + direction).max(0) as usize;
        if new_idx >= siblings.len() || new_idx == current_idx {
            return;
        }

        // Build new order
        let mut ordered_ids: Vec<i64> = siblings.iter().map(|v| v.id).collect();
        ordered_ids.remove(current_idx);
        ordered_ids.insert(new_idx, variable_id);

        let repo = gx.data::<MigrationRepository>();
        match repo.reorder_variables(entity_mapping_id, ordered_ids).await {
            Ok(()) => {
                self.refresh_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to reorder variables: {}", e);
                gx.toast(Toast::error("Failed to reorder variables"));
            }
        }
    }

    // =========================================================================
    // Field Mapping Operations
    // =========================================================================

    async fn add_field_mapping(&self, entity_mapping_id: i64, gx: &GlobalContext) {
        // Find the entity mapping to get target entity
        let entity_mappings = self.entity_mappings.get();
        let Some(em) = entity_mappings
            .iter()
            .find(|em| em.id == entity_mapping_id)
        else {
            return;
        };

        let target_entity = em.target_entity.clone();

        // Fetch attributes for the target entity
        let client = self.target_client.get();
        let target_entity_clone = target_entity.clone();
        let attributes = gx
            .modal(LoadingModal::run_with_default(
                "Loading target entity attributes...",
                || Err(DataverseError::Cancelled),
                async move { client.metadata().attributes(target_entity_clone).await },
            ))
            .await;

        let attributes = match attributes {
            Ok(attrs) => attrs,
            Err(e) if e.is_cancelled() => return,
            Err(e) => {
                log::error!("Failed to fetch attributes for {}: {}", target_entity, e);
                gx.toast(Toast::error("Failed to fetch entity attributes"));
                return;
            }
        };

        // Build options for autocomplete: logical_name (Display Name)
        let options: Vec<(String, String)> = attributes
            .iter()
            .map(|a| {
                let display_name = a.display_name.text_or(&a.logical_name);
                let display = if display_name == &a.logical_name {
                    a.logical_name.clone()
                } else {
                    format!("{} ({})", a.logical_name, display_name)
                };
                (a.logical_name.clone(), display)
            })
            .collect();

        let Some(result) = gx.modal(AddFieldMappingModal::new_modal(options)).await else {
            return;
        };

        let repo = gx.data::<MigrationRepository>();
        let order = self
            .field_mappings
            .get()
            .iter()
            .filter(|fm| fm.entity_mapping_id == entity_mapping_id)
            .count() as i32;

        let new_field_mapping = NewFieldMapping {
            entity_mapping_id,
            order,
            target_field: result.target_field,
        };

        match repo.create_field_mapping(new_field_mapping).await {
            Ok(_id) => {
                gx.toast(Toast::info("Field mapping created"));
                self.refresh_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to create field mapping: {}", e);
                gx.toast(Toast::error("Failed to create field mapping"));
            }
        }
    }

    async fn delete_field_mapping(
        &self,
        field_mapping_id: i64,
        entity_mapping_id: i64,
        cx: &AppContext,
        gx: &GlobalContext,
    ) {
        // Compute next focus before deletion
        let field_mappings = self.field_mappings.get();
        let siblings: Vec<_> = field_mappings
            .iter()
            .filter(|fm| fm.entity_mapping_id == entity_mapping_id)
            .collect();
        let current_idx = siblings.iter().position(|fm| fm.id == field_mapping_id);

        let next_focus = current_idx.and_then(|idx| {
            if idx > 0 {
                siblings
                    .get(idx - 1)
                    .map(|fm| format!("field-mapping-{}", fm.id))
            } else if idx + 1 < siblings.len() {
                siblings
                    .get(idx + 1)
                    .map(|fm| format!("field-mapping-{}", fm.id))
            } else {
                Some(format!("field-mappings-{}", entity_mapping_id))
            }
        });

        let confirmed = gx
            .modal(crate::modals::ConfirmModal::with_message(
                "Delete this field mapping?",
            ))
            .await;

        if !confirmed {
            return;
        }

        let repo = gx.data::<MigrationRepository>();
        match repo.delete_field_mapping(field_mapping_id).await {
            Ok(()) => {
                gx.toast(Toast::info("Field mapping deleted"));
                self.refresh_data(gx).await;

                if let Some(key) = next_focus {
                    cx.focus(&format!("migration-tree-node-{}", key));
                }
            }
            Err(e) => {
                log::error!("Failed to delete field mapping: {}", e);
                gx.toast(Toast::error("Failed to delete field mapping"));
            }
        }
    }

    async fn reorder_field_mapping(
        &self,
        field_mapping_id: i64,
        entity_mapping_id: i64,
        direction: i32,
        gx: &GlobalContext,
    ) {
        let field_mappings = self.field_mappings.get();
        let mut siblings: Vec<_> = field_mappings
            .iter()
            .filter(|fm| fm.entity_mapping_id == entity_mapping_id)
            .collect();
        siblings.sort_by_key(|fm| fm.order);

        let Some(current_idx) = siblings.iter().position(|fm| fm.id == field_mapping_id) else {
            return;
        };

        let new_idx = (current_idx as i32 + direction).max(0) as usize;
        if new_idx >= siblings.len() || new_idx == current_idx {
            return;
        }

        // Build new order
        let mut ordered_ids: Vec<i64> = siblings.iter().map(|fm| fm.id).collect();
        ordered_ids.remove(current_idx);
        ordered_ids.insert(new_idx, field_mapping_id);

        let repo = gx.data::<MigrationRepository>();
        match repo.reorder_field_mappings(entity_mapping_id, ordered_ids).await {
            Ok(()) => {
                self.refresh_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to reorder field mappings: {}", e);
                gx.toast(Toast::error("Failed to reorder field mappings"));
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

        // Reload variables and field mappings
        let entity_mappings = self.entity_mappings.get();
        let mut all_variables = Vec::new();
        let mut all_field_mappings = Vec::new();
        for em in &entity_mappings {
            if let Ok(vars) = repo.get_variables(em.id).await {
                all_variables.extend(vars);
            }
            if let Ok(fms) = repo.get_field_mappings(em.id).await {
                all_field_mappings.extend(fms);
            }
        }
        self.variables.set(all_variables);
        self.field_mappings.set(all_field_mappings);

        self.rebuild_tree();
    }

    fn rebuild_tree(&self) {
        let phases = self.phases.get();
        let entity_mappings = self.entity_mappings.get();
        let variables = self.variables.get();
        let field_mappings = self.field_mappings.get();

        let nodes = build_tree_nodes(phases, entity_mappings, variables, field_mappings);
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

    fn render_variables_detail(&self, entity_mapping_id: i64) -> Element {
        let em = self
            .entity_mappings
            .get()
            .iter()
            .find(|em| em.id == entity_mapping_id)
            .cloned();

        let parent_name = em.map(|e| e.name).unwrap_or_else(|| "Unknown".to_string());

        let var_count = self
            .variables
            .get()
            .iter()
            .filter(|v| v.entity_mapping_id == entity_mapping_id)
            .count();

        Element::col()
            .gap(1)
            .child(
                Element::text("Variables")
                    .style(Style::new().bold().foreground(Color::var("interact"))),
            )
            .child(
                Element::col()
                    .child(
                        Element::row()
                            .gap(1)
                            .child(
                                Element::text("Parent")
                                    .style(Style::new().foreground(Color::var("muted"))),
                            )
                            .child(Element::text(&parent_name)),
                    )
                    .child(
                        Element::row()
                            .gap(1)
                            .child(
                                Element::text("Count")
                                    .style(Style::new().foreground(Color::var("muted"))),
                            )
                            .child(Element::text(format!("{}", var_count))),
                    )
                    .child(
                        Element::text("Computed values available in field mapping transforms")
                            .style(Style::new().foreground(Color::var("muted"))),
                    ),
            )
    }

    fn render_variable_detail(&self, variable: &Variable) -> Element {
        let em = self
            .entity_mappings
            .get()
            .iter()
            .find(|em| em.id == variable.entity_mapping_id)
            .cloned();

        let parent_name = em.map(|e| e.name).unwrap_or_else(|| "Unknown".to_string());

        Element::col()
            .gap(1)
            .child(
                Element::text("Variable")
                    .style(Style::new().bold().foreground(Color::var("interact"))),
            )
            .child(
                Element::col()
                    .child(
                        Element::row()
                            .gap(1)
                            .child(
                                Element::text("Name")
                                    .style(Style::new().foreground(Color::var("muted"))),
                            )
                            .child(Element::text(format!("${}", variable.name))),
                    )
                    .child(
                        Element::row()
                            .gap(1)
                            .child(
                                Element::text("Parent")
                                    .style(Style::new().foreground(Color::var("muted"))),
                            )
                            .child(Element::text(&parent_name)),
                    )
                    .child(
                        Element::text("Press Enter to edit transform chain")
                            .style(Style::new().foreground(Color::var("muted"))),
                    ),
            )
    }

    fn render_field_mappings_detail(&self, entity_mapping_id: i64) -> Element {
        let em = self
            .entity_mappings
            .get()
            .iter()
            .find(|em| em.id == entity_mapping_id)
            .cloned();

        let parent_name = em.map(|e| e.name).unwrap_or_else(|| "Unknown".to_string());

        let fm_count = self
            .field_mappings
            .get()
            .iter()
            .filter(|fm| fm.entity_mapping_id == entity_mapping_id)
            .count();

        Element::col()
            .gap(1)
            .child(
                Element::text("Field Mappings")
                    .style(Style::new().bold().foreground(Color::var("interact"))),
            )
            .child(
                Element::col()
                    .child(
                        Element::row()
                            .gap(1)
                            .child(
                                Element::text("Parent")
                                    .style(Style::new().foreground(Color::var("muted"))),
                            )
                            .child(Element::text(&parent_name)),
                    )
                    .child(
                        Element::row()
                            .gap(1)
                            .child(
                                Element::text("Count")
                                    .style(Style::new().foreground(Color::var("muted"))),
                            )
                            .child(Element::text(format!("{}", fm_count))),
                    )
                    .child(
                        Element::text("Mappings from source fields to target fields")
                            .style(Style::new().foreground(Color::var("muted"))),
                    ),
            )
    }

    fn render_field_mapping_detail(&self, field_mapping: &FieldMapping) -> Element {
        let em = self
            .entity_mappings
            .get()
            .iter()
            .find(|em| em.id == field_mapping.entity_mapping_id)
            .cloned();

        let parent_name = em.map(|e| e.name).unwrap_or_else(|| "Unknown".to_string());

        Element::col()
            .gap(1)
            .child(
                Element::text("Field Mapping")
                    .style(Style::new().bold().foreground(Color::var("interact"))),
            )
            .child(
                Element::col()
                    .child(
                        Element::row()
                            .gap(1)
                            .child(
                                Element::text("Target Field")
                                    .style(Style::new().foreground(Color::var("muted"))),
                            )
                            .child(Element::text(&field_mapping.target_field)),
                    )
                    .child(
                        Element::row()
                            .gap(1)
                            .child(
                                Element::text("Parent")
                                    .style(Style::new().foreground(Color::var("muted"))),
                            )
                            .child(Element::text(&parent_name)),
                    )
                    .child(
                        Element::text("Press Enter to edit transform chain")
                            .style(Style::new().foreground(Color::var("muted"))),
                    ),
            )
    }

    fn element(&self) -> Element {
        let focused = self.focused_node();
        let has_selection = focused.is_some();
        let add_label = match &focused {
            None => "Add Phase",
            Some(MigrationTreeNode::Phase(_)) => "Add Entity",
            Some(MigrationTreeNode::EntityMapping(_)) => "Add Entity",
            Some(MigrationTreeNode::Variables { .. }) => "Add Variable",
            Some(MigrationTreeNode::Variable(_)) => "Add Variable",
            Some(MigrationTreeNode::FieldMappings { .. }) => "Add Field",
            Some(MigrationTreeNode::FieldMapping(_)) => "Add Field",
            Some(_) => "Add", // Other config nodes
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
