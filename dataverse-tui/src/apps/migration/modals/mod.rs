//! Modals for the migration app.

mod new_entity_mapping;
mod new_migration;
mod new_phase;

pub use new_entity_mapping::NewEntityMappingModal;
pub use new_entity_mapping::NewEntityMappingResult;
pub use new_migration::NewMigrationModal;
pub use new_phase::NewPhaseModal;
pub use new_phase::NewPhaseResult;
