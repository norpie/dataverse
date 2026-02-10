//! ValueMap transform creation helpers.
//!
//! Handles option set metadata fetching, type resolution for source/target
//! option sets, and the ValueMap creation flow.

use dataverse_lib::DataverseClient;
use dataverse_lib::model::FieldType;
use dataverse_lib::model::OptionInfo;
use dataverse_lib::model::ValueType;
use dataverse_lib::model::metadata::AttributeType;
use rafter::prelude::*;

use crate::apps::migration::modals::ValueMapTransformModal;
use crate::apps::migration::types::OptionSetContext;
use crate::apps::migration::types::ParentType;
use crate::apps::migration::types::TransformData;

use super::MigrationEditor;
use super::insert_target::InsertTarget;

/// Extracted option set kind + name for ValueMap resolution.
pub(super) struct OptionSetInfo {
    pub kind: AttributeType,
    pub name: String,
}

impl MigrationEditor {
    /// Create ValueMap transform data by determining source/target option set
    /// contexts from type tracking and entity metadata.
    pub(super) async fn create_value_map_data(
        &self,
        target: &InsertTarget,
        gx: &GlobalContext,
    ) -> Option<TransformData> {
        // --- Step 1: Determine source option set type ---
        let source_type = self.resolve_input_type_at(target);

        let source_os_info = match Self::extract_option_set_info(&source_type) {
            Some(info) => info,
            None => {
                gx.toast(Toast::error(
                    "No option set type flowing in — add a source transform first",
                ));
                return None;
            }
        };

        // --- Step 2: Determine target option set type ---
        let target_os_info = match self.resolve_target_option_set(target) {
            Some(info) => info,
            None => {
                gx.toast(Toast::error(
                    "Target is not an option set field — ValueMap requires an option set target",
                ));
                return None;
            }
        };

        // --- Step 3: Fetch option set metadata for source and target ---
        let source_client = self.source_client.get();
        let target_client = self.target_client.get();
        let source_entity = self
            .entity_mappings
            .get()
            .iter()
            .find(|em| em.id == target.entity_mapping_id)
            .map(|em| em.source_entity.clone())
            .unwrap_or_default();
        let target_entity = self
            .entity_mappings
            .get()
            .iter()
            .find(|em| em.id == target.entity_mapping_id)
            .map(|em| em.target_entity.clone())
            .unwrap_or_default();

        log::debug!(
            "create_value_map_data: source option set kind={:?} name='{}' on entity '{}'",
            source_os_info.kind,
            source_os_info.name,
            source_entity,
        );
        log::debug!(
            "create_value_map_data: target option set kind={:?} name='{}' on entity '{}'",
            target_os_info.kind,
            target_os_info.name,
            target_entity,
        );

        let src_name = source_os_info.name.clone();
        let src_entity = source_entity.clone();
        let tgt_name = target_os_info.name.clone();
        let tgt_entity = target_entity.clone();

        let (source_result, target_result) = crate::modals::parallel_load!(gx, {
            "Loading source option set" => async move {
                fetch_option_set_options(source_client, &src_entity, &src_name).await
            },
            "Loading target option set" => async move {
                fetch_option_set_options(target_client, &tgt_entity, &tgt_name).await
            },
        });

        let source_options = match source_result {
            Ok(Ok(opts)) => opts,
            Ok(Err(e)) => {
                log::error!("Failed to fetch source option set: {}", e);
                gx.toast(Toast::error("Failed to fetch source option set metadata"));
                return None;
            }
            Err(e) => {
                log::warn!("Source option set load cancelled: {e}");
                gx.toast(Toast::error("Failed to load source option set"));
                return None;
            }
        };

        let target_options = match target_result {
            Ok(Ok(opts)) => opts,
            Ok(Err(e)) => {
                log::error!("Failed to fetch target option set: {}", e);
                gx.toast(Toast::error("Failed to fetch target option set metadata"));
                return None;
            }
            Err(e) => {
                log::warn!("Target option set load cancelled: {e}");
                gx.toast(Toast::error("Failed to load target option set"));
                return None;
            }
        };

        log::debug!(
            "create_value_map_data: source options count={}, target options count={}",
            source_options.len(),
            target_options.len(),
        );

        let source_ctx = OptionSetContext {
            name: source_os_info.name,
            kind: source_os_info.kind,
            options: source_options,
        };

        let target_ctx = OptionSetContext {
            name: target_os_info.name,
            kind: target_os_info.kind,
            options: target_options,
        };

        // --- Step 4: Open ValueMap modal ---
        let modal = ValueMapTransformModal::new_modal(
            source_ctx.options.clone(),
            target_ctx.options.clone(),
            vec![],
        );

        gx.modal(modal)
            .await
            .map(|mappings| TransformData::ValueMap {
                source: source_ctx,
                target: target_ctx,
                mappings,
            })
    }

    /// Get the input type at the insert position.
    ///
    /// If inserting after an existing transform, returns that transform's output type
    /// by looking it up in the tree. If inserting at position 0, returns `Null`.
    pub(super) fn resolve_input_type_at(&self, target: &InsertTarget) -> ValueType {
        if target.insert_order == 0 {
            return ValueType::Null;
        }

        // Find the transform right before the insert position
        let transforms = self.transforms.get();
        let mut siblings: Vec<_> = transforms
            .iter()
            .filter(|t| t.parent_type == target.parent_type && t.parent_id == target.parent_id)
            .collect();
        siblings.sort_by_key(|t| t.order);

        let prev_transform = siblings.iter().rfind(|t| t.order < target.insert_order);

        match prev_transform {
            Some(t) => {
                // Look up the transform node in the tree to get its output_type
                let key = format!("transform-{}", t.id);
                self.tree_state
                    .with_ref(|s| {
                        s.find_node(&key)
                            .and_then(|node| node.value.as_transform_node())
                            .and_then(|tn| tn.output_type.clone())
                    })
                    .unwrap_or(ValueType::Null)
            }
            None => ValueType::Null,
        }
    }

    /// Extract option set kind + name from a ValueType, if it's an option set.
    pub(super) fn extract_option_set_info(vt: &ValueType) -> Option<OptionSetInfo> {
        match vt {
            ValueType::Known(FieldType::OptionSet { kind, name }) => Some(OptionSetInfo {
                kind: *kind,
                name: name.clone(),
            }),
            // If it's a union, pick the first option set
            ValueType::Union(types) => types.iter().find_map(|ft| match ft {
                FieldType::OptionSet { kind, name } => Some(OptionSetInfo {
                    kind: *kind,
                    name: name.clone(),
                }),
                _ => None,
            }),
            _ => None,
        }
    }

    /// Resolve the target option set info by walking up from the insert position
    /// to find the owning FieldMapping or Variable.
    pub(super) fn resolve_target_option_set(&self, target: &InsertTarget) -> Option<OptionSetInfo> {
        match target.parent_type {
            ParentType::Variable => {
                // Look up the variable's declared_type
                let variables = self.variables.get();
                let variable = variables.iter().find(|v| v.id == target.parent_id)?;
                Self::extract_option_set_info(&variable.declared_type)
            }
            ParentType::FieldMapping => {
                // Look up the field mapping node in the tree to get its target_type
                let key = format!("field-mapping-{}", target.parent_id);
                let target_type = self.tree_state.with_ref(|s| {
                    s.find_node(&key)
                        .and_then(|node| node.value.as_field_mapping_node())
                        .and_then(|fmn| fmn.target_type.clone())
                })?;
                Self::extract_option_set_info(&target_type)
            }
            ParentType::MatchBranch => {
                // Walk up: MatchBranch → parent Transform → its parent (FieldMapping/Variable)
                let mb = self
                    .match_branches
                    .get()
                    .iter()
                    .find(|mb| mb.id == target.parent_id)
                    .cloned()?;
                let parent_transform = self
                    .transforms
                    .get()
                    .iter()
                    .find(|t| t.id == mb.transform_id)
                    .cloned()?;
                self.resolve_target_option_set(&InsertTarget {
                    entity_mapping_id: target.entity_mapping_id,
                    parent_type: parent_transform.parent_type,
                    parent_id: parent_transform.parent_id,
                    insert_order: 0,
                })
            }
            ParentType::CoalesceChain => {
                let cc = self
                    .coalesce_chains
                    .get()
                    .iter()
                    .find(|cc| cc.id == target.parent_id)
                    .cloned()?;
                let parent_transform = self
                    .transforms
                    .get()
                    .iter()
                    .find(|t| t.id == cc.transform_id)
                    .cloned()?;
                self.resolve_target_option_set(&InsertTarget {
                    entity_mapping_id: target.entity_mapping_id,
                    parent_type: parent_transform.parent_type,
                    parent_id: parent_transform.parent_id,
                    insert_order: 0,
                })
            }
            ParentType::FindCondition => {
                let fc = self
                    .find_conditions
                    .get()
                    .iter()
                    .find(|fc| fc.id == target.parent_id)
                    .cloned()?;
                let parent_transform = self
                    .transforms
                    .get()
                    .iter()
                    .find(|t| t.id == fc.transform_id)
                    .cloned()?;
                self.resolve_target_option_set(&InsertTarget {
                    entity_mapping_id: target.entity_mapping_id,
                    parent_type: parent_transform.parent_type,
                    parent_id: parent_transform.parent_id,
                    insert_order: 0,
                })
            }
            ParentType::GuardFallback | ParentType::MatchDefault | ParentType::FindDefault => {
                // parent_id is the transform_id of the guard/match/find
                let parent_transform = self
                    .transforms
                    .get()
                    .iter()
                    .find(|t| t.id == target.parent_id)
                    .cloned()?;
                self.resolve_target_option_set(&InsertTarget {
                    entity_mapping_id: target.entity_mapping_id,
                    parent_type: parent_transform.parent_type,
                    parent_id: parent_transform.parent_id,
                    insert_order: 0,
                })
            }
            ParentType::MatchCondition => {
                // Match conditions produce match values, not mapped to a target field type
                None
            }
        }
    }

    /// Fetch target entity names for autocomplete. Returns None if failed.
    pub(super) async fn fetch_target_entities(&self, gx: &GlobalContext) -> Option<Vec<String>> {
        let client = self.target_client.get();
        let result: Result<Vec<String>, _> = gx
            .modal(crate::modals::LoadingModal::run_with_default(
                "Loading target entities",
                || Err(dataverse_lib::error::Error::Cancelled),
                async move {
                    client.metadata().all_entities().await.map(|entities| {
                        entities
                            .into_iter()
                            .map(|e| e.logical_name)
                            .collect::<Vec<_>>()
                    })
                },
            ))
            .await;

        match result {
            Ok(entities) => Some(entities),
            Err(e) => {
                log::error!("Failed to fetch target entities: {}", e);
                gx.toast(Toast::error("Failed to fetch target entities"));
                None
            }
        }
    }
}

/// Fetch option set options by option set name on an entity.
///
/// Uses the full entity metadata (which expands typed picklist/state/status
/// attributes with their option sets) and searches by option set name.
pub(super) async fn fetch_option_set_options(
    client: DataverseClient,
    entity: &str,
    option_set_name: &str,
) -> Result<Vec<OptionInfo>, dataverse_lib::error::Error> {
    log::debug!(
        "fetch_option_set_options: fetching entity metadata for '{}' to find option set '{}'",
        entity,
        option_set_name,
    );
    let metadata = client.metadata().entity(entity).await?;

    // Search picklist attributes
    let options = metadata
        .picklist_attributes
        .iter()
        .find(|a| a.option_set.name.as_deref() == Some(option_set_name))
        .map(|a| extract_picklist_options(&a.option_set.options));

    if let Some(opts) = options {
        log::debug!(
            "fetch_option_set_options: found {} options in picklist for '{}'",
            opts.len(),
            option_set_name,
        );
        return Ok(opts);
    }

    // Search state attributes
    let options = metadata
        .state_attributes
        .iter()
        .find(|a| a.option_set.name.as_deref() == Some(option_set_name))
        .map(|a| {
            a.option_set
                .options
                .iter()
                .map(|opt| OptionInfo {
                    value: opt.value,
                    label: opt.label.text().unwrap_or("").to_string(),
                })
                .collect::<Vec<_>>()
        });

    if let Some(opts) = options {
        log::debug!(
            "fetch_option_set_options: found {} options in state for '{}'",
            opts.len(),
            option_set_name,
        );
        return Ok(opts);
    }

    // Search status attributes
    let options = metadata
        .status_attributes
        .iter()
        .find(|a| a.option_set.name.as_deref() == Some(option_set_name))
        .map(|a| {
            a.option_set
                .options
                .iter()
                .map(|opt| OptionInfo {
                    value: opt.value,
                    label: opt.label.text().unwrap_or("").to_string(),
                })
                .collect::<Vec<_>>()
        });

    if let Some(opts) = options {
        log::debug!(
            "fetch_option_set_options: found {} options in status for '{}'",
            opts.len(),
            option_set_name,
        );
        return Ok(opts);
    }

    // Search multi-select picklist attributes
    let options = metadata
        .multi_select_picklist_attributes
        .iter()
        .find(|a| a.option_set.name.as_deref() == Some(option_set_name))
        .map(|a| extract_picklist_options(&a.option_set.options));

    if let Some(opts) = options {
        log::debug!(
            "fetch_option_set_options: found {} options in multi-select for '{}'",
            opts.len(),
            option_set_name,
        );
        return Ok(opts);
    }

    log::warn!(
        "fetch_option_set_options: no option set '{}' found on entity '{}'",
        option_set_name,
        entity,
    );
    Ok(vec![])
}

/// Extract `OptionInfo` values from API option metadata.
fn extract_picklist_options(
    options: &[dataverse_lib::model::metadata::OptionMetadata],
) -> Vec<OptionInfo> {
    options
        .iter()
        .map(|opt| OptionInfo {
            value: opt.value,
            label: opt.label.text().unwrap_or("").to_string(),
        })
        .collect()
}
