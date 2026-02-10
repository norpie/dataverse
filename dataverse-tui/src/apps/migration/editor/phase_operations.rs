//! Phase add/edit/delete operations.

use rafter::prelude::*;

use crate::apps::migration::modals::EditPhaseModal;
use crate::apps::migration::modals::NewPhaseModal;
use crate::apps::migration::repository::MigrationRepository;
use crate::apps::migration::repository::NewPhase;
use crate::apps::migration::repository::UpdatePhase;
use crate::apps::migration::types::Mode;
use crate::apps::migration::types::Phase;

use super::MigrationEditor;

impl MigrationEditor {
    /// Add a new phase.
    pub(super) async fn add_phase_impl(&self, gx: &GlobalContext) {
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
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to create phase: {}", e);
                gx.toast(Toast::error("Failed to create phase"));
            }
        }
    }

    /// Edit an existing phase.
    pub(super) async fn edit_phase_impl(&self, phase: &Phase, gx: &GlobalContext) {
        let Some(result) = gx.modal(EditPhaseModal::for_phase(phase)).await else {
            return;
        };

        let repo = gx.data::<MigrationRepository>();
        let update = match result {
            crate::apps::migration::modals::EditPhaseResult::Declarative { name } => UpdatePhase {
                name: Some(name),
                mode: Some(Mode::Declarative),
                lua_script: crate::apps::migration::repository::Update::Clear,
            },
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
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to update phase: {}", e);
                gx.toast(Toast::error("Failed to update phase"));
            }
        }
    }

    /// Delete a phase.
    pub(super) async fn delete_phase_impl(
        &self,
        phase_id: i64,
        cx: &AppContext,
        gx: &GlobalContext,
    ) {
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
                self.load_db_data(gx).await;

                // Focus next item
                if let Some(key) = next_focus {
                    cx.focus(format!("migration-tree-node-{}", key));
                }
            }
            Err(e) => {
                log::error!("Failed to delete phase: {}", e);
                gx.toast(Toast::error("Failed to delete phase"));
            }
        }
    }
}
