//! Transform add/delete/reorder operations.

use dataverse_lib::model::Value;
use rafter::prelude::*;

use crate::apps::migration::modals::ConstantTransformModal;
use crate::apps::migration::modals::ConvertTransformModal;
use crate::apps::migration::modals::CopyTransformModal;
use crate::apps::migration::modals::FormatTransformModal;
use crate::apps::migration::modals::ParseDateTransformModal;
use crate::apps::migration::modals::ReplaceTransformModal;
use crate::apps::migration::modals::SelectTransformModal;
use crate::apps::migration::modals::StringOpsTransformModal;
use crate::apps::migration::modals::TransformType;
use crate::apps::migration::modals::ValueMapTransformModal;
use crate::apps::migration::repository::MigrationRepository;
use crate::apps::migration::repository::NewTransform;
use crate::apps::migration::repository::UpdateTransform;
use crate::apps::migration::types::CoalesceChain;
use crate::apps::migration::types::Condition;
use crate::apps::migration::types::Expr;
use crate::apps::migration::types::FindCondition;
use crate::apps::migration::types::FindFallback;
use crate::apps::migration::types::FindMode;
use crate::apps::migration::types::MatchBranch;
use crate::apps::migration::types::MathOp;
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

                self.refresh_data(gx).await;
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

        let variables: Vec<String> = self
            .variables
            .get()
            .iter()
            .filter(|v| v.entity_mapping_id == target.entity_mapping_id)
            .map(|v| v.name.clone())
            .collect();

        match transform_type {
            // =================================================================
            // No-config transforms — create directly
            // =================================================================
            TransformType::Guid => Some(TransformData::Guid),
            TransformType::ParseInt => Some(TransformData::ParseInt),
            TransformType::ParseDecimal => Some(TransformData::ParseDecimal),
            TransformType::Coalesce => Some(TransformData::Coalesce),
            TransformType::Match => Some(TransformData::Match),

            // =================================================================
            // Config transforms — open edit modal first
            // =================================================================
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
                // TODO: MathTransformModal — for now create with default
                Some(TransformData::Math {
                    operation: MathOp::Add(0.0),
                })
            }
            TransformType::Guard => {
                // TODO: GuardTransformModal — for now create with default
                Some(TransformData::Guard {
                    condition: Condition::IsNull(Expr::SystemVar(SystemVar::Value)),
                })
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
                // Special flow — see 1C.5
                // TODO: create_value_map_data
                gx.toast(Toast::warning(
                    "ValueMap creation flow not yet implemented",
                ));
                None
            }
        }
    }

    /// Determine where to insert a new transform based on the focused node.
    fn get_transform_insert_target(&self) -> Option<InsertTarget> {
        let focused = self.focused_node()?;

        match focused {
            MigrationTreeNode::Variable(v) => {
                // Add to end of variable's chain
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
            ParentType::GuardFallback => {
                // GuardFallback parent_id is the transform_id of the guard
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
                self.refresh_data(gx).await;

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
                self.refresh_data(gx).await;
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

        // Get variable names for this entity mapping
        let variables: Vec<String> = self
            .variables
            .get()
            .iter()
            .filter(|v| v.entity_mapping_id == entity_mapping.id)
            .map(|v| v.name.clone())
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
            // Other transform types - show toast for now
            _ => {
                gx.toast(Toast::info(
                    "Editor for this transform type not yet implemented",
                ));
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
                self.refresh_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to update transform: {}", e);
                gx.toast(Toast::error("Failed to update transform"));
            }
        }
    }
}
