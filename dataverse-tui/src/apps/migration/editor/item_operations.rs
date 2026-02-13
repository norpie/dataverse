//! Variable and field mapping add/delete/reorder operations.

use dataverse_lib::error::Error as DataverseError;
use rafter::prelude::*;

use crate::apps::migration::modals::AddFieldMappingModal;
use crate::apps::migration::modals::AddVariableModal;
use crate::apps::migration::repository::MigrationRepository;
use crate::apps::migration::repository::NewFieldMapping;
use crate::apps::migration::repository::NewVariable;
use crate::apps::migration::repository::UpdateVariable;
use crate::apps::migration::types::Variable;
use crate::modals::LoadingModal;

use super::MigrationEditor;

impl MigrationEditor {
    /// Add a new variable to an entity mapping.
    pub(super) async fn add_variable_impl(&self, entity_mapping_id: i64, gx: &GlobalContext) {
        let client = self.source_client.get();
        let existing_names: Vec<String> = self
            .variables
            .get()
            .iter()
            .filter(|v| v.entity_mapping_id == entity_mapping_id)
            .map(|v| v.name.clone())
            .collect();
        let Some(result) = gx
            .modal(AddVariableModal::new_modal(client, existing_names))
            .await
        else {
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
            declared_type: result.declared_type,
        };

        match repo.create_variable(new_variable).await {
            Ok(_id) => {
                gx.toast(Toast::info("Variable created"));
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to create variable: {}", e);
                gx.toast(Toast::error("Failed to create variable"));
            }
        }
    }

    /// Edit an existing variable (name and type).
    pub(super) async fn edit_variable_impl(&self, variable: &Variable, gx: &GlobalContext) {
        let client = self.source_client.get();
        let existing_names: Vec<String> = self
            .variables
            .get()
            .iter()
            .filter(|v| v.entity_mapping_id == variable.entity_mapping_id)
            .map(|v| v.name.clone())
            .collect();
        let Some(result) = gx
            .modal(AddVariableModal::edit_modal(
                client,
                &variable.name,
                variable.declared_type.clone(),
                existing_names,
            ))
            .await
        else {
            return;
        };

        let repo = gx.data::<MigrationRepository>();
        let update = UpdateVariable {
            name: Some(result.name),
            declared_type: Some(result.declared_type),
        };

        match repo.update_variable(variable.id, update).await {
            Ok(()) => {
                gx.toast(Toast::info("Variable updated"));
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to update variable: {}", e);
                gx.toast(Toast::error("Failed to update variable"));
            }
        }
    }

    /// Delete a variable.
    pub(super) async fn delete_variable_impl(
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
                self.load_db_data(gx).await;

                if let Some(key) = next_focus {
                    cx.focus(format!("migration-tree-node-{}", key));
                }
            }
            Err(e) => {
                log::error!("Failed to delete variable: {}", e);
                gx.toast(Toast::error("Failed to delete variable"));
            }
        }
    }

    /// Reorder a variable.
    pub(super) async fn reorder_variable_impl(
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
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to reorder variables: {}", e);
                gx.toast(Toast::error("Failed to reorder variables"));
            }
        }
    }

    /// Add a new field mapping to an entity mapping.
    pub(super) async fn add_field_mapping_impl(&self, entity_mapping_id: i64, gx: &GlobalContext) {
        // Find the entity mapping to get target entity
        let entity_mappings = self.entity_mappings.get();
        let Some(em) = entity_mappings.iter().find(|em| em.id == entity_mapping_id) else {
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

        // Collect already-mapped target fields for this entity mapping
        let field_mappings = self.field_mappings.get();
        let already_mapped: Vec<&str> = field_mappings
            .iter()
            .filter(|fm| fm.entity_mapping_id == entity_mapping_id)
            .map(|fm| fm.target_field.as_str())
            .collect();

        // Build options for autocomplete: logical_name (Display Name), excluding already-mapped fields
        let options: Vec<(String, String)> = attributes
            .iter()
            .filter(|a| !already_mapped.contains(&a.logical_name.as_str()))
            .map(|a| {
                let display_name = a.display_name.text_or(&a.logical_name);
                let display = if display_name == a.logical_name {
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
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to create field mapping: {}", e);
                gx.toast(Toast::error("Failed to create field mapping"));
            }
        }
    }

    /// Delete a field mapping.
    pub(super) async fn delete_field_mapping_impl(
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
                self.load_db_data(gx).await;

                if let Some(key) = next_focus {
                    cx.focus(format!("migration-tree-node-{}", key));
                }
            }
            Err(e) => {
                log::error!("Failed to delete field mapping: {}", e);
                gx.toast(Toast::error("Failed to delete field mapping"));
            }
        }
    }

    /// Reorder a field mapping.
    pub(super) async fn reorder_field_mapping_impl(
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
        match repo
            .reorder_field_mappings(entity_mapping_id, ordered_ids)
            .await
        {
            Ok(()) => {
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to reorder field mappings: {}", e);
                gx.toast(Toast::error("Failed to reorder field mappings"));
            }
        }
    }
}
