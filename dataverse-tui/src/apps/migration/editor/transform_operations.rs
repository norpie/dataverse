//! Transform add/delete/reorder operations.

use dataverse_lib::model::Value;
use rafter::prelude::*;

use crate::apps::migration::modals::ConstantTransformModal;
use crate::apps::migration::modals::CopyTransformModal;
use crate::apps::migration::modals::SelectTransformModal;
use crate::apps::migration::modals::StringOpsTransformModal;
use crate::apps::migration::modals::TransformType;
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
    ///
    /// Returns the target info if the focused node supports adding transforms.
    pub(super) async fn add_transform_impl(&self, gx: &GlobalContext) {
        let Some(target) = self.get_transform_insert_target() else {
            log::debug!("add_transform_impl: focused node doesn't support adding transforms");
            return;
        };

        // Show transform type picker
        let Some(transform_type) = gx.modal(SelectTransformModal::new_modal()).await else {
            return;
        };

        // Create default transform data for the selected type
        let data = default_transform_data(transform_type);

        let repo = gx.data::<MigrationRepository>();

        // Shift existing transforms at insert_order and after
        let transforms = self.transforms.get();
        let siblings: Vec<_> = transforms
            .iter()
            .filter(|t| t.parent_type == target.parent_type && t.parent_id == target.parent_id)
            .collect();

        // Build new order for siblings that need shifting
        let mut needs_reorder = false;
        let mut ordered_ids: Vec<i64> = Vec::new();
        for t in &siblings {
            if t.order >= target.insert_order {
                needs_reorder = true;
            }
            ordered_ids.push(t.id);
        }

        // If we need to shift, update orders first
        if needs_reorder && !ordered_ids.is_empty() {
            // Increment order for all transforms at or after insert position
            // We'll just reorder the whole list after inserting
        }

        let new_transform = NewTransform {
            entity_mapping_id: target.entity_mapping_id,
            parent_type: target.parent_type,
            parent_id: target.parent_id,
            order: target.insert_order,
            data,
        };

        match repo.create_transform(new_transform).await {
            Ok(new_id) => {
                // Now reorder to ensure proper sequence
                // Get fresh list and reorder
                if let Ok(all_transforms) = repo
                    .get_transforms(target.parent_type, target.parent_id)
                    .await
                {
                    let mut sorted: Vec<_> = all_transforms.iter().collect();
                    sorted.sort_by_key(|t| {
                        if t.id == new_id {
                            // New transform goes at insert position
                            (target.insert_order, 0)
                        } else if t.order >= target.insert_order {
                            // Existing transforms at or after insert position shift down
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

                gx.toast(Toast::info("Transform added"));
                self.refresh_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to create transform: {}", e);
                gx.toast(Toast::error("Failed to create transform"));
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
            MigrationTreeNode::FieldMapping(fm) => {
                // Add to end of field mapping's chain
                let order = self.transform_count_for_parent(ParentType::FieldMapping, fm.id);
                Some(InsertTarget {
                    entity_mapping_id: fm.entity_mapping_id,
                    parent_type: ParentType::FieldMapping,
                    parent_id: fm.id,
                    insert_order: order,
                })
            }
            MigrationTreeNode::Transform(t) => {
                // Add after this transform in the same chain
                Some(InsertTarget {
                    entity_mapping_id: t.entity_mapping_id,
                    parent_type: t.parent_type,
                    parent_id: t.parent_id,
                    insert_order: t.order + 1,
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
            MigrationTreeNode::Chain { parent_type, parent_id } => {
                // Add to end of the chain
                let order = self.transform_count_for_parent(parent_type, parent_id);
                let entity_mapping_id = self.entity_mapping_id_for_chain(parent_type, parent_id)?;
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
    fn entity_mapping_id_for_chain(&self, parent_type: ParentType, parent_id: i64) -> Option<i64> {
        match parent_type {
            ParentType::Variable => {
                self.variables
                    .get()
                    .iter()
                    .find(|v| v.id == parent_id)
                    .map(|v| v.entity_mapping_id)
            }
            ParentType::FieldMapping => {
                self.field_mappings
                    .get()
                    .iter()
                    .find(|fm| fm.id == parent_id)
                    .map(|fm| fm.entity_mapping_id)
            }
            ParentType::MatchBranch => {
                let mb = self.match_branches.get().iter().find(|mb| mb.id == parent_id).cloned()?;
                self.entity_mapping_id_for_match_branch(&mb)
            }
            ParentType::CoalesceChain => {
                let cc = self.coalesce_chains.get().iter().find(|cc| cc.id == parent_id).cloned()?;
                self.entity_mapping_id_for_coalesce_chain(&cc)
            }
            ParentType::FindCondition => {
                let fc = self.find_conditions.get().iter().find(|fc| fc.id == parent_id).cloned()?;
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
            .filter(|t| t.parent_type == transform.parent_type && t.parent_id == transform.parent_id)
            .collect();
        let current_idx = siblings.iter().position(|t| t.id == transform.id);

        let next_focus = current_idx.and_then(|idx| {
            if idx > 0 {
                // Focus previous sibling
                siblings.get(idx - 1).map(|t| format!("transform-{}", t.id))
            } else if idx + 1 < siblings.len() {
                // Focus next sibling
                siblings.get(idx + 1).map(|t| format!("transform-{}", t.id))
            } else {
                // No siblings left, focus parent
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
                    let _ = repo.reorder_transforms(parent_type, parent_id, ordered_ids).await;
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
            .filter(|t| t.parent_type == transform.parent_type && t.parent_id == transform.parent_id)
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
            TransformData::Guid => {
                // GUID has no configuration - it just generates a random UUID
                gx.toast(Toast::info("GUID generates a random UUID - no configuration needed"));
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
            // Other transform types - show toast for now
            _ => {
                gx.toast(Toast::info("Editor for this transform type not yet implemented"));
            }
        }
    }

    /// Update a transform's data in the database.
    async fn update_transform_data(&self, transform_id: i64, data: TransformData, gx: &GlobalContext) {
        let repo = gx.data::<MigrationRepository>();
        
        match repo.update_transform(transform_id, UpdateTransform { data }).await {
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

/// Create default TransformData for a given transform type.
fn default_transform_data(transform_type: TransformType) -> TransformData {
    match transform_type {
        TransformType::Copy => TransformData::Copy {
            path: String::new(),
        },
        TransformType::Constant => TransformData::Constant { value: Value::Null },
        TransformType::Guid => TransformData::Guid,
        TransformType::StringOps => TransformData::StringOps { op: StringOp::Trim },
        TransformType::Format => TransformData::Format {
            template: String::new(),
        },
        TransformType::Replace => TransformData::Replace {
            from: String::new(),
            to: String::new(),
            regex: false,
        },
        TransformType::Convert => TransformData::Convert {
            target_type: "string".to_string(),
        },
        TransformType::ParseInt => TransformData::ParseInt,
        TransformType::ParseDecimal => TransformData::ParseDecimal,
        TransformType::ParseDate => TransformData::ParseDate {
            format: "%Y-%m-%d".to_string(),
        },
        TransformType::ValueMap => TransformData::ValueMap {
            mappings: Vec::new(),
        },
        TransformType::Math => TransformData::Math {
            operation: MathOp::Add(0.0),
        },
        TransformType::Guard => TransformData::Guard {
            condition: Condition::IsNull(Expr::SystemVar(SystemVar::Value)),
        },
        TransformType::Coalesce => TransformData::Coalesce,
        TransformType::Match => TransformData::Match,
        TransformType::Find => TransformData::Find {
            entity: String::new(),
            fallback: FindFallback::Null,
            mode: FindMode::Where,
        },
    }
}
