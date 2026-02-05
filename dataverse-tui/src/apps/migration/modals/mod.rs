//! Modals for the migration app.

mod add_field_mapping;
mod add_variable;
mod copy_transform;
mod edit_entity_mapping;
mod edit_phase;
mod new_migration;
mod new_phase;
mod passes;
mod select_transform;
mod test_guids;
mod unmatched_handling;

pub use add_field_mapping::AddFieldMappingModal;
pub use add_field_mapping::AddFieldMappingResult;
pub use add_variable::AddVariableModal;
pub use add_variable::AddVariableResult;
pub use copy_transform::CopyTransformModal;
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
pub use select_transform::SelectTransformModal;
pub use select_transform::TransformType;
pub use test_guids::TestGuidsModal;
pub use unmatched_handling::UnmatchedHandlingModal;
pub use unmatched_handling::UnmatchedHandlingResult;
