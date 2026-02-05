//! Modals for the migration app.

mod edit_entity_mapping;
mod edit_phase;
mod new_migration;
mod new_phase;
mod passes;
mod test_guids;

pub use edit_entity_mapping::EditEntityMappingModal;
pub use edit_entity_mapping::EntityMappingResult;
pub use edit_phase::EditPhaseModal;
pub use edit_phase::EditPhaseResult;
pub use new_migration::NewMigrationModal;
pub use new_migration::NewMigrationResult;
pub use new_phase::NewPhaseModal;
pub use new_phase::NewPhaseResult;
pub use passes::PassesModal;
pub use passes::PassesResult;
pub use test_guids::TestGuidsModal;
