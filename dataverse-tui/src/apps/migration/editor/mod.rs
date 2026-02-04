//! Migration editor app for editing a migration's phases and entity mappings.

mod tree;

use std::fs;
use std::path::PathBuf;

use dataverse_lib::DataverseClient;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::Text;
use rafter::widgets::Tree;
use rafter::widgets::TreeState;

use crate::apps::migration::modals::EditPhaseModal;
use crate::apps::migration::modals::NewEntityMappingModal;
use crate::apps::migration::modals::NewPhaseModal;
use crate::apps::migration::repository::MigrationRepository;
use crate::apps::migration::repository::NewEntityMapping;
use crate::apps::migration::repository::NewPhase;
use crate::apps::migration::repository::UpdatePhase;
use crate::apps::migration::types::EntityMapping;
use crate::apps::migration::types::MatchStrategy;
use crate::apps::migration::types::Migration;
use crate::apps::migration::types::Mode;
use crate::apps::migration::types::NoMatchFallback;
use crate::apps::migration::types::OrphanStrategy;
use crate::apps::migration::types::Phase;
use crate::modals::parallel_load;
use crate::modals::FileBrowserModal;
use crate::paths;

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
        bind("l", load_script);
        bind("x", export_script);
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
            None => {}
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
            MigrationTreeNode::EntityMapping(_em) => {
                // TODO: Open entity mapping editor
            }
        }
    }

    #[handler]
    async fn load_script(&self, gx: &GlobalContext) {
        let Some(MigrationTreeNode::Phase(phase)) = self.focused_node() else {
            return;
        };

        if phase.mode != Mode::Lua {
            gx.toast(Toast::warning("Phase must be in Lua mode to load a script"));
            return;
        }

        let start_dir = paths::downloads_dir().unwrap_or_else(|| PathBuf::from("."));
        let Some(result) = gx
            .modal(FileBrowserModal::browse(&start_dir, vec!["lua".to_string()]).require_existing())
            .await
        else {
            return;
        };

        // Read the file
        let content = match fs::read_to_string(&result.path) {
            Ok(content) => content,
            Err(e) => {
                log::error!("Failed to read script file: {}", e);
                gx.toast(Toast::error("Failed to read script file"));
                return;
            }
        };

        // Update the phase
        let repo = gx.data::<MigrationRepository>();
        let update = UpdatePhase {
            name: None,
            mode: None,
            lua_script: Some(content),
        };

        match repo.update_phase(phase.id, update).await {
            Ok(()) => {
                gx.toast(Toast::info("Script loaded"));
                self.refresh_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to update phase: {}", e);
                gx.toast(Toast::error("Failed to save script"));
            }
        }
    }

    #[handler]
    async fn export_script(&self, gx: &GlobalContext) {
        let Some(MigrationTreeNode::Phase(phase)) = self.focused_node() else {
            return;
        };

        if phase.mode != Mode::Lua {
            gx.toast(Toast::warning("Phase must be in Lua mode to export a script"));
            return;
        }

        let Some(script) = &phase.lua_script else {
            gx.toast(Toast::warning("No script to export"));
            return;
        };

        let start_dir = paths::downloads_dir().unwrap_or_else(|| PathBuf::from("."));
        let Some(result) = gx
            .modal(FileBrowserModal::browse(&start_dir, vec!["lua".to_string()]).with_filename(&phase.name))
            .await
        else {
            return;
        };

        // Write the file
        match fs::write(&result.path, script) {
            Ok(()) => {
                gx.toast(Toast::info("Script exported"));
            }
            Err(e) => {
                log::error!("Failed to write script file: {}", e);
                gx.toast(Toast::error("Failed to export script"));
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
            lua_script: None,
        };

        match repo.update_phase(phase.id, update).await {
            Ok(()) => {
                gx.toast(Toast::info("Phase updated"));
                self.refresh_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to update phase: {}", e);
                gx.toast(Toast::error("Failed to update phase"));
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
            .modal(NewEntityMappingModal::with_entities(
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

        let new_mapping = NewEntityMapping {
            phase_id,
            order,
            source_entity: result.source_entity,
            target_entity: result.target_entity,
            mode: Mode::Declarative,
            lua_script: None,
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

    fn element(&self) -> Element {
        let focused = self.focused_node();
        let has_selection = focused.is_some();
        let add_label = if has_selection { "Add Entity" } else { "Add Phase" };

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: background) {
                text (content: {self.title()}) style (bold, fg: interact)

                row (width: fill, height: fill, gap: 1) {
                    box_ (id: "migration-tree-container", height: fill, width: fill) style (bg: surface) {
                        tree (state: self.tree_state, id: "migration-tree", width: fill, height: fill)
                            on_activate: edit_item()
                    }

                    column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
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
                                    if phase.mode == Mode::Lua {
                                        column {
                                            text (content: "Script") style (fg: muted)
                                            box_ (width: fill, height: 10) style (bg: background) {
                                                text (content: {script_preview(&phase.lua_script)}) style (fg: muted)
                                            }
                                            row (gap: 2) {
                                                row (gap: 1) {
                                                    text (content: "l") style (fg: primary)
                                                    text (content: "load") style (fg: muted)
                                                }
                                                row (gap: 1) {
                                                    text (content: "x") style (fg: primary)
                                                    text (content: "export") style (fg: muted)
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Some(MigrationTreeNode::EntityMapping(em)) => {
                                column (gap: 1) {
                                    text (content: "Entity Mapping") style (bold, fg: interact)
                                    column {
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
                                        row (gap: 1) {
                                            text (content: "Match") style (fg: muted)
                                            text (content: {if em.match_strategy == MatchStrategy::Find { "Find" } else { "Same ID" }})
                                        }
                                        row (gap: 1) {
                                            text (content: "Create") style (fg: muted)
                                            text (content: {if em.create_pass_enabled { "Yes" } else { "No" }})
                                        }
                                        row (gap: 1) {
                                            text (content: "Update") style (fg: muted)
                                            text (content: {if em.update_pass_enabled { "Yes" } else { "No" }})
                                        }
                                    }
                                }
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

/// Format a Lua script for preview display (first 10 lines).
fn script_preview(script: &Option<String>) -> String {
    match script {
        None => "No script defined".to_string(),
        Some(s) if s.is_empty() => "No script defined".to_string(),
        Some(s) => {
            let lines: Vec<&str> = s.lines().take(10).collect();
            let preview = lines.join("\n");
            if s.lines().count() > 10 {
                format!("{}\n...", preview)
            } else {
                preview
            }
        }
    }
}
