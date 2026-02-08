//! Transform add/delete/reorder operations.

use dataverse_lib::model::metadata::AttributeType;
use dataverse_lib::model::FieldType;
use dataverse_lib::model::OptionInfo;
use dataverse_lib::model::Value;
use dataverse_lib::model::ValueType;
use dataverse_lib::DataverseClient;
use rafter::prelude::*;

use crate::apps::migration::modals::ConstantTransformModal;
use crate::apps::migration::modals::ConvertTransformModal;
use crate::apps::migration::modals::CopyTransformModal;
use crate::apps::migration::modals::FormatTransformModal;
use crate::apps::migration::modals::GuardTransformModal;
use crate::apps::migration::modals::MatchTransformModal;
use crate::apps::migration::modals::MathTransformModal;
use crate::apps::migration::modals::ParseDateTransformModal;
use crate::apps::migration::modals::VariableInfo;
use crate::apps::migration::modals::ReplaceTransformModal;
use crate::apps::migration::modals::SelectTransformModal;
use crate::apps::migration::modals::StringOpsTransformModal;
use crate::apps::migration::modals::TransformType;
use crate::apps::migration::modals::ValueMapTransformModal;
use crate::apps::migration::repository::MigrationRepository;
use crate::apps::migration::repository::NewCoalesceChain;
use crate::apps::migration::repository::NewMatchBranch;
use crate::apps::migration::repository::NewTransform;
use crate::apps::migration::repository::UpdateMatchBranch;
use crate::apps::migration::repository::UpdateTransform;
use crate::apps::migration::types::CoalesceChain;
use crate::apps::migration::types::Condition;
use crate::apps::migration::types::Expr;
use crate::apps::migration::types::FindCondition;
use crate::apps::migration::types::FindFallback;
use crate::apps::migration::types::FindMode;
use crate::apps::migration::types::MatchBranch;
use crate::apps::migration::types::MathOp;
use crate::apps::migration::types::OptionSetContext;
use crate::apps::migration::types::ParentType;
use crate::apps::migration::types::StringOp;
use crate::apps::migration::types::SystemVar;
use crate::apps::migration::types::Transform;
use crate::apps::migration::types::TransformData;
use crate::modals::ConfirmModal;

use super::tree::MigrationTreeNode;
use super::MigrationEditor;

/// Information about where to insert a new transform.
struct InsertTarget {
    /// The entity mapping this transform belongs to.
    entity_mapping_id: i64,
    /// The parent type for the transform.
    parent_type: ParentType,
    /// The parent id for the transform.
    parent_id: i64,
    /// The order at which to insert (existing transforms at this order and after will be shifted).
    insert_order: i32,
}

impl MigrationEditor {
    /// Add a new transform based on the focused node.
    pub(super) async fn add_transform_impl(&self, gx: &GlobalContext) {
        let Some(target) = self.get_transform_insert_target() else {
            log::debug!("add_transform_impl: focused node doesn't support adding transforms");
            return;
        };

        // Show transform type picker
        let Some(transform_type) = gx.modal(SelectTransformModal::new_modal()).await else {
            return;
        };

        // Create transform data — config transforms open their edit modal first
        let Some(data) = self
            .create_transform_data(transform_type, &target, gx)
            .await
        else {
            return; // User cancelled
        };

        let repo = gx.data::<MigrationRepository>();

        let new_transform = NewTransform {
            entity_mapping_id: target.entity_mapping_id,
            parent_type: target.parent_type,
            parent_id: target.parent_id,
            order: target.insert_order,
            data,
        };

        match repo.create_transform(new_transform).await {
            Ok(new_id) => {
                // Reorder to ensure proper sequence
                if let Ok(all_transforms) = repo
                    .get_transforms(target.parent_type, target.parent_id)
                    .await
                {
                    let mut sorted: Vec<_> = all_transforms.iter().collect();
                    sorted.sort_by_key(|t| {
                        if t.id == new_id {
                            (target.insert_order, 0)
                        } else if t.order >= target.insert_order {
                            (t.order, 1)
                        } else {
                            (t.order, 0)
                        }
                    });
                    let new_order: Vec<i64> = sorted.iter().map(|t| t.id).collect();
                    let _ = repo
                        .reorder_transforms(target.parent_type, target.parent_id, new_order)
                        .await;
                }

                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to create transform: {}", e);
                gx.toast(Toast::error("Failed to create transform"));
            }
        }
    }

    /// Create transform data for a given type.
    ///
    /// No-config transforms return data directly. Config transforms open their
    /// edit modal first — the transform is only created if the user confirms.
    /// Returns `None` if the user cancels.
    async fn create_transform_data(
        &self,
        transform_type: TransformType,
        target: &InsertTarget,
        gx: &GlobalContext,
    ) -> Option<TransformData> {
        // Get context needed by some modals
        let entity_mapping = self
            .entity_mappings
            .get()
            .iter()
            .find(|em| em.id == target.entity_mapping_id)
            .cloned();

        let source_entity = entity_mapping
            .as_ref()
            .map(|em| em.source_entity.clone())
            .unwrap_or_default();

        let variables: Vec<VariableInfo> = self
            .variables
            .get()
            .iter()
            .filter(|v| v.entity_mapping_id == target.entity_mapping_id)
            .map(|v| VariableInfo {
                name: v.name.clone(),
                declared_type: v.declared_type.clone(),
            })
            .collect();

        match transform_type {
            // =================================================================
            // No-config transforms — create directly
            // =================================================================
            TransformType::Guid => Some(TransformData::Guid),
            TransformType::ParseInt => Some(TransformData::ParseInt),
            TransformType::ParseDecimal => Some(TransformData::ParseDecimal),
            TransformType::Coalesce => Some(TransformData::Coalesce),

            // =================================================================
            // Config transforms — open edit modal first
            // =================================================================
            TransformType::Match => {
                let modal = MatchTransformModal::new_modal(false);
                gx.modal(modal)
                    .await
                    .map(|has_default| TransformData::Match { has_default })
            }
            TransformType::Copy => {
                let modal = CopyTransformModal::new_modal(
                    self.source_client.get().clone(),
                    source_entity,
                    variables,
                    String::new(),
                );
                gx.modal(modal)
                    .await
                    .map(|path| TransformData::Copy { path })
            }
            TransformType::Constant => {
                let modal = ConstantTransformModal::new_modal(Value::Null);
                gx.modal(modal)
                    .await
                    .map(|value| TransformData::Constant { value })
            }
            TransformType::StringOps => {
                let modal = StringOpsTransformModal::new_modal(StringOp::Trim);
                gx.modal(modal)
                    .await
                    .map(|op| TransformData::StringOps { op })
            }
            TransformType::Format => {
                let modal = FormatTransformModal::new_modal(
                    self.source_client.get().clone(),
                    source_entity,
                    variables,
                    String::new(),
                );
                gx.modal(modal)
                    .await
                    .map(|template| TransformData::Format { template })
            }
            TransformType::Replace => {
                let modal =
                    ReplaceTransformModal::new_modal(String::new(), String::new(), false);
                gx.modal(modal).await.map(|result| TransformData::Replace {
                    from: result.from,
                    to: result.to,
                    regex: result.regex,
                })
            }
            TransformType::Convert => {
                let modal = ConvertTransformModal::new_modal("string");
                gx.modal(modal)
                    .await
                    .map(|target_type| TransformData::Convert { target_type })
            }
            TransformType::ParseDate => {
                let modal = ParseDateTransformModal::new_modal("%Y-%m-%d".to_string());
                gx.modal(modal)
                    .await
                    .map(|format| TransformData::ParseDate { format })
            }
            TransformType::Math => {
                let modal = MathTransformModal::new_modal(MathOp::Add(0.0));
                gx.modal(modal)
                    .await
                    .map(|op| TransformData::Math { operation: op })
            }
            TransformType::Guard => {
                let default_condition = Condition::IsNull(Expr::SystemVar(SystemVar::Value));
                let modal = GuardTransformModal::new_modal(
                    self.source_client.get().clone(),
                    source_entity,
                    variables,
                    default_condition,
                );
                gx.modal(modal)
                    .await
                    .map(|condition| TransformData::Guard { condition })
            }
            TransformType::Find => {
                // TODO: FindTransformModal — for now create with default
                Some(TransformData::Find {
                    entity: String::new(),
                    fallback: FindFallback::Null,
                    mode: FindMode::Where,
                })
            }
            TransformType::ValueMap => {
                self.create_value_map_data(target, gx).await
            }
        }
    }

    // =========================================================================
    // ValueMap Creation
    // =========================================================================

    /// Create ValueMap transform data by determining source/target option set
    /// contexts from type tracking and entity metadata.
    async fn create_value_map_data(
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

        gx.modal(modal).await.map(|mappings| TransformData::ValueMap {
            source: source_ctx,
            target: target_ctx,
            mappings,
        })
    }

    /// Get the input type at the insert position.
    ///
    /// If inserting after an existing transform, returns that transform's output type
    /// by looking it up in the tree. If inserting at position 0, returns `Null`.
    fn resolve_input_type_at(&self, target: &InsertTarget) -> ValueType {
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

        let prev_transform = siblings
            .iter()
            .filter(|t| t.order < target.insert_order)
            .last();

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
    fn extract_option_set_info(vt: &ValueType) -> Option<OptionSetInfo> {
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
    fn resolve_target_option_set(&self, target: &InsertTarget) -> Option<OptionSetInfo> {
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
            ParentType::GuardFallback | ParentType::MatchDefault => {
                // parent_id is the transform_id of the guard/match
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
        }
    }

    /// Determine where to insert a new transform based on the focused node.
    fn get_transform_insert_target(&self) -> Option<InsertTarget> {
        let focused = self.focused_node()?;

        match focused {
            MigrationTreeNode::Variable(vn) => {
                // Add to end of variable's chain
                let v = &vn.variable;
                let order = self.transform_count_for_parent(ParentType::Variable, v.id);
                Some(InsertTarget {
                    entity_mapping_id: v.entity_mapping_id,
                    parent_type: ParentType::Variable,
                    parent_id: v.id,
                    insert_order: order,
                })
            }
            MigrationTreeNode::FieldMapping(fmn) => {
                // Add to end of field mapping's chain
                let fm = &fmn.field_mapping;
                let order = self.transform_count_for_parent(ParentType::FieldMapping, fm.id);
                Some(InsertTarget {
                    entity_mapping_id: fm.entity_mapping_id,
                    parent_type: ParentType::FieldMapping,
                    parent_id: fm.id,
                    insert_order: order,
                })
            }
            MigrationTreeNode::Transform(tn) => {
                // Add after this transform in the same chain
                Some(InsertTarget {
                    entity_mapping_id: tn.transform.entity_mapping_id,
                    parent_type: tn.transform.parent_type,
                    parent_id: tn.transform.parent_id,
                    insert_order: tn.transform.order + 1,
                })
            }
            MigrationTreeNode::MatchBranch(mb) => {
                // Add to end of match branch's chain
                let order = self.transform_count_for_parent(ParentType::MatchBranch, mb.id);
                let entity_mapping_id = self.entity_mapping_id_for_match_branch(&mb)?;
                Some(InsertTarget {
                    entity_mapping_id,
                    parent_type: ParentType::MatchBranch,
                    parent_id: mb.id,
                    insert_order: order,
                })
            }
            MigrationTreeNode::CoalesceChain(cc) => {
                // Add to end of coalesce chain
                let order = self.transform_count_for_parent(ParentType::CoalesceChain, cc.id);
                let entity_mapping_id = self.entity_mapping_id_for_coalesce_chain(&cc)?;
                Some(InsertTarget {
                    entity_mapping_id,
                    parent_type: ParentType::CoalesceChain,
                    parent_id: cc.id,
                    insert_order: order,
                })
            }
            MigrationTreeNode::FindCondition(fc) => {
                // Add to end of find condition's chain
                let order = self.transform_count_for_parent(ParentType::FindCondition, fc.id);
                let entity_mapping_id = self.entity_mapping_id_for_find_condition(&fc)?;
                Some(InsertTarget {
                    entity_mapping_id,
                    parent_type: ParentType::FindCondition,
                    parent_id: fc.id,
                    insert_order: order,
                })
            }
            MigrationTreeNode::MatchDefault { transform_id } => {
                // Add to end of default branch chain
                let order =
                    self.transform_count_for_parent(ParentType::MatchDefault, transform_id);
                let entity_mapping_id = self
                    .transforms
                    .get()
                    .iter()
                    .find(|t| t.id == transform_id)
                    .map(|t| t.entity_mapping_id)?;
                Some(InsertTarget {
                    entity_mapping_id,
                    parent_type: ParentType::MatchDefault,
                    parent_id: transform_id,
                    insert_order: order,
                })
            }
            MigrationTreeNode::Chain {
                parent_type,
                parent_id,
            } => {
                // Add to end of the chain
                let order = self.transform_count_for_parent(parent_type, parent_id);
                let entity_mapping_id =
                    self.entity_mapping_id_for_chain(parent_type, parent_id)?;
                Some(InsertTarget {
                    entity_mapping_id,
                    parent_type,
                    parent_id,
                    insert_order: order,
                })
            }
            // Other node types don't support adding transforms directly
            _ => None,
        }
    }

    /// Count transforms for a given parent.
    fn transform_count_for_parent(&self, parent_type: ParentType, parent_id: i64) -> i32 {
        self.transforms
            .get()
            .iter()
            .filter(|t| t.parent_type == parent_type && t.parent_id == parent_id)
            .count() as i32
    }

    /// Get entity_mapping_id for a match branch by traversing up to its transform.
    fn entity_mapping_id_for_match_branch(&self, mb: &MatchBranch) -> Option<i64> {
        self.transforms
            .get()
            .iter()
            .find(|t| t.id == mb.transform_id)
            .map(|t| t.entity_mapping_id)
    }

    /// Get entity_mapping_id for a coalesce chain by traversing up to its transform.
    fn entity_mapping_id_for_coalesce_chain(&self, cc: &CoalesceChain) -> Option<i64> {
        self.transforms
            .get()
            .iter()
            .find(|t| t.id == cc.transform_id)
            .map(|t| t.entity_mapping_id)
    }

    /// Get entity_mapping_id for a find condition by traversing up to its transform.
    fn entity_mapping_id_for_find_condition(&self, fc: &FindCondition) -> Option<i64> {
        self.transforms
            .get()
            .iter()
            .find(|t| t.id == fc.transform_id)
            .map(|t| t.entity_mapping_id)
    }

    /// Get entity_mapping_id for a chain wrapper node.
    fn entity_mapping_id_for_chain(
        &self,
        parent_type: ParentType,
        parent_id: i64,
    ) -> Option<i64> {
        match parent_type {
            ParentType::Variable => self
                .variables
                .get()
                .iter()
                .find(|v| v.id == parent_id)
                .map(|v| v.entity_mapping_id),
            ParentType::FieldMapping => self
                .field_mappings
                .get()
                .iter()
                .find(|fm| fm.id == parent_id)
                .map(|fm| fm.entity_mapping_id),
            ParentType::MatchBranch => {
                let mb = self
                    .match_branches
                    .get()
                    .iter()
                    .find(|mb| mb.id == parent_id)
                    .cloned()?;
                self.entity_mapping_id_for_match_branch(&mb)
            }
            ParentType::CoalesceChain => {
                let cc = self
                    .coalesce_chains
                    .get()
                    .iter()
                    .find(|cc| cc.id == parent_id)
                    .cloned()?;
                self.entity_mapping_id_for_coalesce_chain(&cc)
            }
            ParentType::FindCondition => {
                let fc = self
                    .find_conditions
                    .get()
                    .iter()
                    .find(|fc| fc.id == parent_id)
                    .cloned()?;
                self.entity_mapping_id_for_find_condition(&fc)
            }
            ParentType::GuardFallback | ParentType::MatchDefault => {
                // parent_id is the transform_id of the guard/match
                self.transforms
                    .get()
                    .iter()
                    .find(|t| t.id == parent_id)
                    .map(|t| t.entity_mapping_id)
            }
        }
    }

    // =========================================================================
    // Delete Transform
    // =========================================================================

    /// Delete a transform and all its nested children.
    pub(super) async fn delete_transform_impl(
        &self,
        transform: &Transform,
        cx: &AppContext,
        gx: &GlobalContext,
    ) {
        // Compute next focus before deletion
        let transforms = self.transforms.get();
        let siblings: Vec<_> = transforms
            .iter()
            .filter(|t| {
                t.parent_type == transform.parent_type && t.parent_id == transform.parent_id
            })
            .collect();
        let current_idx = siblings.iter().position(|t| t.id == transform.id);

        let next_focus = current_idx.and_then(|idx| {
            if idx > 0 {
                siblings
                    .get(idx - 1)
                    .map(|t| format!("transform-{}", t.id))
            } else if idx + 1 < siblings.len() {
                siblings
                    .get(idx + 1)
                    .map(|t| format!("transform-{}", t.id))
            } else {
                self.parent_focus_key(transform.parent_type, transform.parent_id)
            }
        });

        // Confirm deletion
        let confirmed = gx
            .modal(ConfirmModal::with_message("Delete this transform?"))
            .await;

        if !confirmed {
            return;
        }

        let repo = gx.data::<MigrationRepository>();
        let transform_id = transform.id;
        let parent_type = transform.parent_type;
        let parent_id = transform.parent_id;

        match repo.delete_transform(transform_id).await {
            Ok(()) => {
                // Reorder remaining siblings
                if let Ok(remaining) = repo.get_transforms(parent_type, parent_id).await {
                    let ordered_ids: Vec<i64> = remaining.iter().map(|t| t.id).collect();
                    let _ = repo
                        .reorder_transforms(parent_type, parent_id, ordered_ids)
                        .await;
                }

                gx.toast(Toast::info("Transform deleted"));
                self.load_db_data(gx).await;

                if let Some(key) = next_focus {
                    cx.focus(&format!("migration-tree-node-{}", key));
                }
            }
            Err(e) => {
                log::error!("Failed to delete transform: {}", e);
                gx.toast(Toast::error("Failed to delete transform"));
            }
        }
    }

    /// Get focus key for the parent of a transform.
    fn parent_focus_key(&self, parent_type: ParentType, parent_id: i64) -> Option<String> {
        match parent_type {
            ParentType::Variable => Some(format!("variable-{}", parent_id)),
            ParentType::FieldMapping => Some(format!("field-mapping-{}", parent_id)),
            ParentType::MatchBranch => Some(format!("match-branch-{}", parent_id)),
            ParentType::CoalesceChain => Some(format!("coalesce-chain-{}", parent_id)),
            ParentType::FindCondition => Some(format!("find-condition-{}", parent_id)),
            ParentType::GuardFallback => Some(format!("transform-{}", parent_id)),
            ParentType::MatchDefault => Some(format!("match-default-{}", parent_id)),
        }
    }

    // =========================================================================
    // Reorder Transform
    // =========================================================================

    /// Reorder a transform within its chain.
    pub(super) async fn reorder_transform_impl(
        &self,
        transform: &Transform,
        direction: i32,
        gx: &GlobalContext,
    ) {
        let transforms = self.transforms.get();
        let mut siblings: Vec<_> = transforms
            .iter()
            .filter(|t| {
                t.parent_type == transform.parent_type && t.parent_id == transform.parent_id
            })
            .collect();
        siblings.sort_by_key(|t| t.order);

        let Some(current_idx) = siblings.iter().position(|t| t.id == transform.id) else {
            return;
        };

        let new_idx = (current_idx as i32 + direction).max(0) as usize;
        if new_idx >= siblings.len() || new_idx == current_idx {
            return;
        }

        // Build new order
        let mut ordered_ids: Vec<i64> = siblings.iter().map(|t| t.id).collect();
        ordered_ids.remove(current_idx);
        ordered_ids.insert(new_idx, transform.id);

        let repo = gx.data::<MigrationRepository>();
        match repo
            .reorder_transforms(transform.parent_type, transform.parent_id, ordered_ids)
            .await
        {
            Ok(()) => {
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to reorder transforms: {}", e);
                gx.toast(Toast::error("Failed to reorder transforms"));
            }
        }
    }

    // =========================================================================
    // Edit Transform
    // =========================================================================

    /// Edit a transform by showing the appropriate modal based on its type.
    pub(super) async fn edit_transform_impl(&self, transform: &Transform, gx: &GlobalContext) {
        // Get the entity mapping for this transform
        let entity_mapping = self
            .entity_mappings
            .get()
            .iter()
            .find(|em| em.id == transform.entity_mapping_id)
            .cloned();

        let Some(entity_mapping) = entity_mapping else {
            log::error!("Entity mapping not found for transform");
            return;
        };

        let source_entity = entity_mapping.source_entity;

        // Get variables with type info for this entity mapping
        let variables: Vec<VariableInfo> = self
            .variables
            .get()
            .iter()
            .filter(|v| v.entity_mapping_id == entity_mapping.id)
            .map(|v| VariableInfo {
                name: v.name.clone(),
                declared_type: v.declared_type.clone(),
            })
            .collect();

        // Dispatch based on transform type
        match &transform.data {
            TransformData::Copy { path } => {
                let modal = CopyTransformModal::new_modal(
                    self.source_client.get().clone(),
                    source_entity,
                    variables,
                    path.clone(),
                );

                if let Some(new_path) = gx.modal(modal).await {
                    self.update_transform_data(
                        transform.id,
                        TransformData::Copy { path: new_path },
                        gx,
                    )
                    .await;
                }
            }
            TransformData::Constant { value } => {
                let modal = ConstantTransformModal::new_modal(value.clone());

                if let Some(new_value) = gx.modal(modal).await {
                    self.update_transform_data(
                        transform.id,
                        TransformData::Constant { value: new_value },
                        gx,
                    )
                    .await;
                }
            }
            TransformData::StringOps { op } => {
                let modal = StringOpsTransformModal::new_modal(op.clone());

                if let Some(new_op) = gx.modal(modal).await {
                    self.update_transform_data(
                        transform.id,
                        TransformData::StringOps { op: new_op },
                        gx,
                    )
                    .await;
                }
            }
            TransformData::Format { template } => {
                let modal = FormatTransformModal::new_modal(
                    self.source_client.get().clone(),
                    source_entity.clone(),
                    variables.clone(),
                    template.clone(),
                );

                if let Some(new_template) = gx.modal(modal).await {
                    self.update_transform_data(
                        transform.id,
                        TransformData::Format {
                            template: new_template,
                        },
                        gx,
                    )
                    .await;
                }
            }
            TransformData::Replace { from, to, regex } => {
                let modal = ReplaceTransformModal::new_modal(from.clone(), to.clone(), *regex);

                if let Some(result) = gx.modal(modal).await {
                    self.update_transform_data(
                        transform.id,
                        TransformData::Replace {
                            from: result.from,
                            to: result.to,
                            regex: result.regex,
                        },
                        gx,
                    )
                    .await;
                }
            }
            TransformData::Convert { target_type } => {
                let modal = ConvertTransformModal::new_modal(target_type);

                if let Some(new_type) = gx.modal(modal).await {
                    self.update_transform_data(
                        transform.id,
                        TransformData::Convert {
                            target_type: new_type,
                        },
                        gx,
                    )
                    .await;
                }
            }
            TransformData::Guid => {
                gx.toast(Toast::info(
                    "GUID generates a random UUID - no configuration needed",
                ));
            }
            TransformData::ParseInt => {
                gx.toast(Toast::info(
                    "Parse Int converts string to integer - no configuration needed",
                ));
            }
            TransformData::ParseDecimal => {
                gx.toast(Toast::info(
                    "Parse Decimal converts string to decimal - no configuration needed",
                ));
            }
            TransformData::ParseDate { format } => {
                let modal = ParseDateTransformModal::new_modal(format.clone());

                if let Some(new_format) = gx.modal(modal).await {
                    self.update_transform_data(
                        transform.id,
                        TransformData::ParseDate { format: new_format },
                        gx,
                    )
                    .await;
                }
            }
            TransformData::ValueMap {
                source,
                target,
                mappings,
            } => {
                let modal = ValueMapTransformModal::new_modal(
                    source.options.clone(),
                    target.options.clone(),
                    mappings.clone(),
                );

                if let Some(new_mappings) = gx.modal(modal).await {
                    self.update_transform_data(
                        transform.id,
                        TransformData::ValueMap {
                            source: source.clone(),
                            target: target.clone(),
                            mappings: new_mappings,
                        },
                        gx,
                    )
                    .await;
                }
            }
            TransformData::Math { operation } => {
                let modal = MathTransformModal::new_modal(operation.clone());

                if let Some(new_op) = gx.modal(modal).await {
                    self.update_transform_data(
                        transform.id,
                        TransformData::Math { operation: new_op },
                        gx,
                    )
                    .await;
                }
            }
            TransformData::Guard { condition } => {
                let modal = GuardTransformModal::new_modal(
                    self.source_client.get().clone(),
                    source_entity,
                    variables,
                    condition.clone(),
                );

                if let Some(new_condition) = gx.modal(modal).await {
                    self.update_transform_data(
                        transform.id,
                        TransformData::Guard {
                            condition: new_condition,
                        },
                        gx,
                    )
                    .await;
                }
            }
            TransformData::Match { has_default } => {
                let modal = MatchTransformModal::new_modal(*has_default);

                if let Some(new_has_default) = gx.modal(modal).await {
                    if new_has_default == *has_default {
                        return; // No change
                    }

                    // Disabling default: confirm + delete default branch transforms
                    if !new_has_default {
                        let repo = gx.data::<MigrationRepository>();
                        let default_transforms = repo
                            .get_transforms(ParentType::MatchDefault, transform.id)
                            .await
                            .unwrap_or_default();

                        if !default_transforms.is_empty() {
                            let confirmed = gx
                                .modal(ConfirmModal::with_message(format!(
                                    "Removing the default branch will delete {} transform(s). Continue?",
                                    default_transforms.len()
                                )))
                                .await;

                            if !confirmed {
                                return;
                            }

                            for t in &default_transforms {
                                if let Err(e) = repo.delete_transform(t.id).await {
                                    log::error!("Failed to delete default transform: {}", e);
                                }
                            }
                        }
                    }

                    self.update_transform_data(
                        transform.id,
                        TransformData::Match {
                            has_default: new_has_default,
                        },
                        gx,
                    )
                    .await;
                }
            }
            TransformData::Coalesce => {
                // Coalesce has no config — Enter adds a fallback chain
                self.add_coalesce_chain_impl(transform, gx).await;
            }
            // Other transform types - show toast for now
            _ => {
                gx.toast(Toast::info(
                    "Editor for this transform type not yet implemented",
                ));
            }
        }
    }

    // =========================================================================
    // Match Branch Operations
    // =========================================================================

    /// Add a new match branch to a match transform.
    pub(super) async fn add_match_branch_impl(
        &self,
        transform: &Transform,
        gx: &GlobalContext,
    ) {
        // Get entity mapping for source entity + variables
        let entity_mapping = self
            .entity_mappings
            .get()
            .iter()
            .find(|em| em.id == transform.entity_mapping_id)
            .cloned();

        let Some(entity_mapping) = entity_mapping else {
            log::error!("Entity mapping not found for transform");
            return;
        };

        let source_entity = entity_mapping.source_entity;
        let variables: Vec<VariableInfo> = self
            .variables
            .get()
            .iter()
            .filter(|v| v.entity_mapping_id == entity_mapping.id)
            .map(|v| VariableInfo {
                name: v.name.clone(),
                declared_type: v.declared_type.clone(),
            })
            .collect();

        // Open guard-style condition modal for the branch condition
        let default_condition = Condition::IsNull(Expr::SystemVar(SystemVar::Value));
        let modal = GuardTransformModal::new_modal(
            self.source_client.get().clone(),
            source_entity,
            variables,
            default_condition,
        );

        let Some(condition) = gx.modal(modal).await else {
            return;
        };

        // Determine order (append at end)
        let branches = self.match_branches.get();
        let order = branches
            .iter()
            .filter(|mb| mb.transform_id == transform.id)
            .count() as i32;

        let repo = gx.data::<MigrationRepository>();
        match repo
            .create_match_branch(NewMatchBranch {
                transform_id: transform.id,
                order,
                condition,
            })
            .await
        {
            Ok(_id) => {
                gx.toast(Toast::info("Branch added"));
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to create match branch: {}", e);
                gx.toast(Toast::error("Failed to create branch"));
            }
        }
    }

    /// Edit a match branch's condition.
    pub(super) async fn edit_match_branch_impl(
        &self,
        branch: &MatchBranch,
        gx: &GlobalContext,
    ) {
        // Get entity mapping via the parent transform
        let transform = self
            .transforms
            .get()
            .iter()
            .find(|t| t.id == branch.transform_id)
            .cloned();

        let Some(transform) = transform else {
            log::error!("Parent transform not found for match branch");
            return;
        };

        let entity_mapping = self
            .entity_mappings
            .get()
            .iter()
            .find(|em| em.id == transform.entity_mapping_id)
            .cloned();

        let Some(entity_mapping) = entity_mapping else {
            log::error!("Entity mapping not found for transform");
            return;
        };

        let source_entity = entity_mapping.source_entity;
        let variables: Vec<VariableInfo> = self
            .variables
            .get()
            .iter()
            .filter(|v| v.entity_mapping_id == entity_mapping.id)
            .map(|v| VariableInfo {
                name: v.name.clone(),
                declared_type: v.declared_type.clone(),
            })
            .collect();

        let modal = GuardTransformModal::new_modal(
            self.source_client.get().clone(),
            source_entity,
            variables,
            branch.condition.clone(),
        );

        let Some(new_condition) = gx.modal(modal).await else {
            return;
        };

        let repo = gx.data::<MigrationRepository>();
        match repo
            .update_match_branch(
                branch.id,
                UpdateMatchBranch {
                    condition: Some(new_condition),
                },
            )
            .await
        {
            Ok(()) => {
                gx.toast(Toast::info("Branch updated"));
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to update match branch: {}", e);
                gx.toast(Toast::error("Failed to update branch"));
            }
        }
    }

    /// Delete a match branch and its child transforms.
    pub(super) async fn delete_match_branch_impl(
        &self,
        branch: &MatchBranch,
        cx: &AppContext,
        gx: &GlobalContext,
    ) {
        // Compute next focus before deletion
        let branches = self.match_branches.get();
        let siblings: Vec<_> = branches
            .iter()
            .filter(|mb| mb.transform_id == branch.transform_id)
            .collect();
        let current_idx = siblings.iter().position(|mb| mb.id == branch.id);

        let next_focus = current_idx.and_then(|idx| {
            if idx > 0 {
                siblings
                    .get(idx - 1)
                    .map(|mb| format!("match-branch-{}", mb.id))
            } else if idx + 1 < siblings.len() {
                siblings
                    .get(idx + 1)
                    .map(|mb| format!("match-branch-{}", mb.id))
            } else {
                Some(format!("transform-{}", branch.transform_id))
            }
        });

        let confirmed = gx
            .modal(ConfirmModal::with_message("Delete this branch?"))
            .await;

        if !confirmed {
            return;
        }

        let repo = gx.data::<MigrationRepository>();
        match repo.delete_match_branch(branch.id).await {
            Ok(()) => {
                // Reorder remaining siblings
                if let Ok(remaining) = repo.get_match_branches(branch.transform_id).await {
                    let ordered_ids: Vec<i64> = remaining.iter().map(|mb| mb.id).collect();
                    let _ = repo
                        .reorder_match_branches(branch.transform_id, ordered_ids)
                        .await;
                }

                gx.toast(Toast::info("Branch deleted"));
                self.load_db_data(gx).await;

                if let Some(key) = next_focus {
                    cx.focus(&format!("migration-tree-node-{}", key));
                }
            }
            Err(e) => {
                log::error!("Failed to delete match branch: {}", e);
                gx.toast(Toast::error("Failed to delete branch"));
            }
        }
    }

    /// Reorder a match branch within its transform.
    pub(super) async fn reorder_match_branch_impl(
        &self,
        branch: &MatchBranch,
        direction: i32,
        gx: &GlobalContext,
    ) {
        let branches = self.match_branches.get();
        let mut siblings: Vec<_> = branches
            .iter()
            .filter(|mb| mb.transform_id == branch.transform_id)
            .collect();
        siblings.sort_by_key(|mb| mb.order);

        let Some(current_idx) = siblings.iter().position(|mb| mb.id == branch.id) else {
            return;
        };

        let new_idx = (current_idx as i32 + direction).max(0) as usize;
        if new_idx >= siblings.len() || new_idx == current_idx {
            return;
        }

        let mut ordered_ids: Vec<i64> = siblings.iter().map(|mb| mb.id).collect();
        ordered_ids.remove(current_idx);
        ordered_ids.insert(new_idx, branch.id);

        let repo = gx.data::<MigrationRepository>();
        match repo
            .reorder_match_branches(branch.transform_id, ordered_ids)
            .await
        {
            Ok(()) => {
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to reorder match branches: {}", e);
                gx.toast(Toast::error("Failed to reorder branches"));
            }
        }
    }

    // =========================================================================
    // Coalesce Chain Operations
    // =========================================================================

    /// Add a new fallback chain to a coalesce transform.
    pub(super) async fn add_coalesce_chain_impl(
        &self,
        transform: &Transform,
        gx: &GlobalContext,
    ) {
        let chains = self.coalesce_chains.get();
        let order = chains
            .iter()
            .filter(|cc| cc.transform_id == transform.id)
            .count() as i32;

        let repo = gx.data::<MigrationRepository>();
        match repo
            .create_coalesce_chain(NewCoalesceChain {
                transform_id: transform.id,
                order,
            })
            .await
        {
            Ok(_id) => {
                gx.toast(Toast::info("Fallback chain added"));
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to create coalesce chain: {}", e);
                gx.toast(Toast::error("Failed to create fallback chain"));
            }
        }
    }

    /// Delete a coalesce chain and its child transforms.
    pub(super) async fn delete_coalesce_chain_impl(
        &self,
        chain: &CoalesceChain,
        cx: &AppContext,
        gx: &GlobalContext,
    ) {
        let chains = self.coalesce_chains.get();
        let siblings: Vec<_> = chains
            .iter()
            .filter(|cc| cc.transform_id == chain.transform_id)
            .collect();
        let current_idx = siblings.iter().position(|cc| cc.id == chain.id);

        let next_focus = current_idx.and_then(|idx| {
            if idx > 0 {
                siblings
                    .get(idx - 1)
                    .map(|cc| format!("coalesce-chain-{}", cc.id))
            } else if idx + 1 < siblings.len() {
                siblings
                    .get(idx + 1)
                    .map(|cc| format!("coalesce-chain-{}", cc.id))
            } else {
                Some(format!("transform-{}", chain.transform_id))
            }
        });

        let confirmed = gx
            .modal(ConfirmModal::with_message(
                "Delete this fallback chain and its transforms?",
            ))
            .await;

        if !confirmed {
            return;
        }

        let repo = gx.data::<MigrationRepository>();
        match repo.delete_coalesce_chain(chain.id).await {
            Ok(()) => {
                if let Ok(remaining) = repo.get_coalesce_chains(chain.transform_id).await {
                    let ordered_ids: Vec<i64> = remaining.iter().map(|cc| cc.id).collect();
                    let _ = repo
                        .reorder_coalesce_chains(chain.transform_id, ordered_ids)
                        .await;
                }

                gx.toast(Toast::info("Fallback chain deleted"));
                self.load_db_data(gx).await;

                if let Some(key) = next_focus {
                    cx.focus(&format!("migration-tree-node-{}", key));
                }
            }
            Err(e) => {
                log::error!("Failed to delete coalesce chain: {}", e);
                gx.toast(Toast::error("Failed to delete fallback chain"));
            }
        }
    }

    /// Reorder a coalesce chain within its transform.
    pub(super) async fn reorder_coalesce_chain_impl(
        &self,
        chain: &CoalesceChain,
        direction: i32,
        gx: &GlobalContext,
    ) {
        let chains = self.coalesce_chains.get();
        let mut siblings: Vec<_> = chains
            .iter()
            .filter(|cc| cc.transform_id == chain.transform_id)
            .collect();
        siblings.sort_by_key(|cc| cc.order);

        let Some(current_idx) = siblings.iter().position(|cc| cc.id == chain.id) else {
            return;
        };

        let new_idx = (current_idx as i32 + direction).max(0) as usize;
        if new_idx >= siblings.len() || new_idx == current_idx {
            return;
        }

        let mut ordered_ids: Vec<i64> = siblings.iter().map(|cc| cc.id).collect();
        ordered_ids.remove(current_idx);
        ordered_ids.insert(new_idx, chain.id);

        let repo = gx.data::<MigrationRepository>();
        match repo
            .reorder_coalesce_chains(chain.transform_id, ordered_ids)
            .await
        {
            Ok(()) => {
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to reorder coalesce chains: {}", e);
                gx.toast(Toast::error("Failed to reorder fallback chains"));
            }
        }
    }

    /// Update a transform's data in the database.
    async fn update_transform_data(
        &self,
        transform_id: i64,
        data: TransformData,
        gx: &GlobalContext,
    ) {
        let repo = gx.data::<MigrationRepository>();

        match repo
            .update_transform(transform_id, UpdateTransform { data })
            .await
        {
            Ok(()) => {
                gx.toast(Toast::info("Transform updated"));
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to update transform: {}", e);
                gx.toast(Toast::error("Failed to update transform"));
            }
        }
    }
}

/// Extracted option set kind + name for ValueMap resolution.
struct OptionSetInfo {
    kind: AttributeType,
    name: String,
}

/// Fetch option set options by option set name on an entity.
///
/// Uses the full entity metadata (which expands typed picklist/state/status
/// attributes with their option sets) and searches by option set name.
async fn fetch_option_set_options(
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
