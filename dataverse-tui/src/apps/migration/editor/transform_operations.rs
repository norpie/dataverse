//! Transform add/edit/delete/reorder operations.

use dataverse_lib::model::Value;
use dataverse_lib::model::ValueType;
use rafter::prelude::*;

use crate::apps::migration::modals::ConstantTransformModal;
use crate::apps::migration::modals::ConvertTransformModal;
use crate::apps::migration::modals::CopyTransformModal;
use crate::apps::migration::modals::FindTransformModal;
use crate::apps::migration::modals::FormatTransformModal;
use crate::apps::migration::modals::GuardTransformModal;
use crate::apps::migration::modals::MatchTransformModal;
use crate::apps::migration::modals::MathTransformModal;
use crate::apps::migration::modals::ParseDateTransformModal;
use crate::apps::migration::modals::ReplaceTransformModal;
use crate::apps::migration::modals::SelectTransformModal;
use crate::apps::migration::modals::StringOpsTransformModal;
use crate::apps::migration::modals::TransformType;
use crate::apps::migration::modals::ValueMapTransformModal;
use crate::apps::migration::modals::VariableInfo;
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
use crate::apps::migration::types::MatchCondition;
use crate::apps::migration::types::MathOp;
use crate::apps::migration::types::ParentType;
use crate::apps::migration::types::StringOp;
use crate::apps::migration::types::SystemVar;
use crate::apps::migration::types::Transform;
use crate::apps::migration::types::TransformData;
use crate::modals::ConfirmModal;

use super::MigrationEditor;
use super::insert_target::InsertTarget;
use super::tree::TransformNode;

// =============================================================================
// Scope-aware reorder helpers
// =============================================================================

/// A target slot for moving a transform.
#[derive(Debug, Clone)]
struct MoveTarget {
    parent_type: ParentType,
    parent_id: i64,
    order: i32,
}

/// Get the ordered child scopes of a scope-owning transform.
///
/// Returns a list of `(ParentType, parent_id)` in visual order (top to bottom).
/// Returns empty for transforms that don't own child scopes.
fn child_scopes(
    transform: &Transform,
    match_branches: &[MatchBranch],
    coalesce_chains: &[CoalesceChain],
    find_conditions: &[FindCondition],
    _match_conditions: &[MatchCondition],
) -> Vec<(ParentType, i64)> {
    match &transform.data {
        TransformData::Guard { .. } => {
            vec![(ParentType::GuardFallback, transform.id)]
        }
        TransformData::Match { has_default } => {
            let mut branches: Vec<_> = match_branches
                .iter()
                .filter(|mb| mb.transform_id == transform.id)
                .collect();
            branches.sort_by_key(|mb| mb.order);
            let mut scopes: Vec<(ParentType, i64)> = branches
                .iter()
                .map(|mb| (ParentType::MatchBranch, mb.id))
                .collect();
            if *has_default {
                scopes.push((ParentType::MatchDefault, transform.id));
            }
            scopes
        }
        TransformData::Coalesce => {
            let mut chains: Vec<_> = coalesce_chains
                .iter()
                .filter(|cc| cc.transform_id == transform.id)
                .collect();
            chains.sort_by_key(|cc| cc.order);
            chains
                .iter()
                .map(|cc| (ParentType::CoalesceChain, cc.id))
                .collect()
        }
        TransformData::Find { fallback, mode, .. } => {
            let has_default = matches!(fallback, FindFallback::Default);
            if matches!(mode, FindMode::Where) {
                let mut conditions: Vec<_> = find_conditions
                    .iter()
                    .filter(|fc| fc.transform_id == transform.id)
                    .collect();
                conditions.sort_by_key(|fc| fc.order);
                let mut scopes: Vec<(ParentType, i64)> = conditions
                    .iter()
                    .map(|fc| (ParentType::FindCondition, fc.id))
                    .collect();
                if has_default {
                    scopes.push((ParentType::FindDefault, transform.id));
                }
                scopes
            } else {
                // Lua mode: only default
                if has_default {
                    vec![(ParentType::FindDefault, transform.id)]
                } else {
                    vec![]
                }
            }
        }
        _ => vec![],
    }
}

/// Get transforms in a scope, sorted by order.
fn transforms_in_scope(all_transforms: &[Transform], pt: ParentType, pid: i64) -> Vec<&Transform> {
    let mut ts: Vec<_> = all_transforms
        .iter()
        .filter(|t| t.parent_type == pt && t.parent_id == pid)
        .collect();
    ts.sort_by_key(|t| t.order);
    ts
}

/// Determine the parent scope of a given scope.
///
/// Returns `Some((parent_transform, its_parent_type, its_parent_id))` for nested scopes,
/// or `None` for top-level scopes (FieldMapping, Variable, MatchCondition).
fn parent_scope(
    pt: ParentType,
    pid: i64,
    all_transforms: &[Transform],
    match_branches: &[MatchBranch],
    coalesce_chains: &[CoalesceChain],
    find_conditions: &[FindCondition],
) -> Option<(i64, ParentType, i64)> {
    match pt {
        // Top-level scopes — no parent transform
        ParentType::FieldMapping | ParentType::Variable | ParentType::MatchCondition => None,

        // parent_id IS the parent transform's ID
        ParentType::GuardFallback | ParentType::MatchDefault | ParentType::FindDefault => {
            let parent_transform = all_transforms.iter().find(|t| t.id == pid)?;
            Some((
                parent_transform.id,
                parent_transform.parent_type,
                parent_transform.parent_id,
            ))
        }

        // parent_id is a child table row ID — look up the parent transform
        ParentType::MatchBranch => {
            let branch = match_branches.iter().find(|mb| mb.id == pid)?;
            let parent_transform = all_transforms
                .iter()
                .find(|t| t.id == branch.transform_id)?;
            Some((
                parent_transform.id,
                parent_transform.parent_type,
                parent_transform.parent_id,
            ))
        }
        ParentType::CoalesceChain => {
            let chain = coalesce_chains.iter().find(|cc| cc.id == pid)?;
            let parent_transform = all_transforms.iter().find(|t| t.id == chain.transform_id)?;
            Some((
                parent_transform.id,
                parent_transform.parent_type,
                parent_transform.parent_id,
            ))
        }
        ParentType::FindCondition => {
            let condition = find_conditions.iter().find(|fc| fc.id == pid)?;
            let parent_transform = all_transforms
                .iter()
                .find(|t| t.id == condition.transform_id)?;
            Some((
                parent_transform.id,
                parent_transform.parent_type,
                parent_transform.parent_id,
            ))
        }
    }
}

/// Find the index of a scope `(pt, pid)` within the parent transform's child scopes.
fn scope_index_in_parent(
    pt: ParentType,
    pid: i64,
    parent_transform: &Transform,
    match_branches: &[MatchBranch],
    coalesce_chains: &[CoalesceChain],
    find_conditions: &[FindCondition],
    match_conditions: &[MatchCondition],
) -> Option<usize> {
    let scopes = child_scopes(
        parent_transform,
        match_branches,
        coalesce_chains,
        find_conditions,
        match_conditions,
    );
    scopes
        .iter()
        .position(|(s_pt, s_pid)| *s_pt == pt && *s_pid == pid)
}

/// Enter a scope-owning transform from the given direction.
///
/// - `entering_first` = true: enter via the first child scope (moving DOWN into it)
/// - `entering_first` = false: enter via the last child scope (moving UP into it)
///
/// Recursively descends if the edge transform in the entered scope is itself a scope-owner.
fn enter_scope(
    transform: &Transform,
    entering_first: bool,
    all_transforms: &[Transform],
    match_branches: &[MatchBranch],
    coalesce_chains: &[CoalesceChain],
    find_conditions: &[FindCondition],
    match_conditions: &[MatchCondition],
) -> Option<MoveTarget> {
    let scopes = child_scopes(
        transform,
        match_branches,
        coalesce_chains,
        find_conditions,
        match_conditions,
    );
    if scopes.is_empty() {
        return None;
    }

    let (scope_pt, scope_pid) = if entering_first {
        scopes[0]
    } else {
        scopes[scopes.len() - 1]
    };

    let scope_transforms = transforms_in_scope(all_transforms, scope_pt, scope_pid);

    if entering_first {
        // Insert at position 0. But check if the first transform is a scope-owner we should
        // recurse into — NO: when entering from above, we land at position 0 (before everything).
        // Only recurse when the scope is empty and we need to go deeper? No — empty scope is fine.
        //
        // Actually: the user wants visual DFS order. Moving DOWN past a scope-owner means the next
        // visual position is the first transform inside it. If the scope is empty, the next visual
        // position would be in the next sibling scope or after the parent. But an empty scope IS a
        // valid place to land (the transform will be the first one there).
        Some(MoveTarget {
            parent_type: scope_pt,
            parent_id: scope_pid,
            order: 0,
        })
    } else {
        // Entering from below (moving UP): land at the end. But if the last transform in this
        // scope is itself a scope-owner with children, recurse into it.
        if let Some(last) = scope_transforms.last() {
            let last_scopes = child_scopes(
                last,
                match_branches,
                coalesce_chains,
                find_conditions,
                match_conditions,
            );
            if !last_scopes.is_empty() {
                // Recurse into the last transform's scope
                if let Some(deeper) = enter_scope(
                    last,
                    false, // entering last
                    all_transforms,
                    match_branches,
                    coalesce_chains,
                    find_conditions,
                    match_conditions,
                ) {
                    return Some(deeper);
                }
            }
            // Land after the last transform
            Some(MoveTarget {
                parent_type: scope_pt,
                parent_id: scope_pid,
                order: last.order + 1,
            })
        } else {
            // Empty scope — land at position 0
            Some(MoveTarget {
                parent_type: scope_pt,
                parent_id: scope_pid,
                order: 0,
            })
        }
    }
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

        let mut variables: Vec<VariableInfo> = self
            .variables
            .get()
            .iter()
            .filter(|v| v.entity_mapping_id == target.entity_mapping_id)
            .map(|v| VariableInfo {
                name: v.name.clone(),
                declared_type: v.declared_type.clone(),
            })
            .collect();

        // Add #value so autocomplete can resolve e.g. #value.lookup_field
        let input_type = self.resolve_input_type_at(target);
        if !matches!(input_type, ValueType::Null) {
            variables.push(VariableInfo {
                name: "#value".to_string(),
                declared_type: input_type,
            });
        }

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
                    self.target_client.get().clone(),
                    source_entity,
                    variables,
                    String::new(),
                );
                gx.modal(modal)
                    .await
                    .map(|path| TransformData::Copy { path })
            }
            TransformType::Constant => {
                let modal =
                    ConstantTransformModal::new_modal(self.target_client.get(), Value::Null);
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
                    self.target_client.get().clone(),
                    source_entity,
                    variables,
                    String::new(),
                );
                gx.modal(modal)
                    .await
                    .map(|template| TransformData::Format { template })
            }
            TransformType::Replace => {
                let modal = ReplaceTransformModal::new_modal(String::new(), String::new(), false);
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
                    self.target_client.get().clone(),
                    source_entity,
                    variables,
                    default_condition,
                );
                gx.modal(modal)
                    .await
                    .map(|condition| TransformData::Guard { condition })
            }
            TransformType::Find => {
                let target_entities = self.fetch_target_entities(gx).await?;
                let modal = FindTransformModal::new_modal(target_entities);
                gx.modal(modal).await.map(|r| TransformData::Find {
                    entity: r.entity,
                    fallback: r.fallback,
                    mode: r.mode,
                })
            }
            TransformType::ValueMap => self.create_value_map_data(target, gx).await,
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
                siblings.get(idx - 1).map(|t| format!("transform-{}", t.id))
            } else if idx + 1 < siblings.len() {
                siblings.get(idx + 1).map(|t| format!("transform-{}", t.id))
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
                    cx.focus(format!("migration-tree-node-{}", key));
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
            ParentType::FindDefault => Some(format!("find-default-{}", parent_id)),
            ParentType::MatchCondition => Some(format!("match-condition-{}", parent_id)),
        }
    }

    // =========================================================================
    // Reorder Transform
    // =========================================================================

    /// Reorder a transform, supporting cross-scope movement.
    ///
    /// When at a scope boundary (first/last in chain), the transform moves into
    /// adjacent scopes following visual DFS order rather than being stuck.
    pub(super) async fn reorder_transform_impl(
        &self,
        transform: &Transform,
        direction: i32,
        gx: &GlobalContext,
    ) {
        let all_transforms = self.transforms.get();
        let match_branches = self.match_branches.get();
        let coalesce_chains = self.coalesce_chains.get();
        let find_conditions = self.find_conditions.get();
        let match_conditions = self.match_conditions.get();

        let siblings =
            transforms_in_scope(&all_transforms, transform.parent_type, transform.parent_id);

        let Some(current_idx) = siblings.iter().position(|t| t.id == transform.id) else {
            return;
        };

        let moving_down = direction > 0;

        // Try to find a cross-scope target
        let target = if moving_down {
            self.find_move_target_down(
                transform,
                current_idx,
                &siblings,
                &all_transforms,
                &match_branches,
                &coalesce_chains,
                &find_conditions,
                &match_conditions,
            )
        } else {
            self.find_move_target_up(
                transform,
                current_idx,
                &siblings,
                &all_transforms,
                &match_branches,
                &coalesce_chains,
                &find_conditions,
                &match_conditions,
            )
        };

        let repo = gx.data::<MigrationRepository>();

        match target {
            None => {
                // No movement possible
            }
            Some(t)
                if t.parent_type == transform.parent_type && t.parent_id == transform.parent_id =>
            {
                // Same scope — use simple reorder
                let new_idx = t.order as usize;
                let mut ordered_ids: Vec<i64> = siblings.iter().map(|t| t.id).collect();
                ordered_ids.remove(current_idx);
                ordered_ids.insert(new_idx.min(ordered_ids.len()), transform.id);

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
            Some(t) => {
                // Cross-scope move
                match repo
                    .move_transform_to_scope(transform.id, t.parent_type, t.parent_id, t.order)
                    .await
                {
                    Ok(()) => {
                        self.load_db_data(gx).await;
                    }
                    Err(e) => {
                        log::error!("Failed to move transform to new scope: {}", e);
                        gx.toast(Toast::error("Failed to move transform"));
                    }
                }
            }
        }
    }

    /// Find the target slot when moving a transform DOWN.
    fn find_move_target_down(
        &self,
        transform: &Transform,
        current_idx: usize,
        siblings: &[&Transform],
        all_transforms: &[Transform],
        match_branches: &[MatchBranch],
        coalesce_chains: &[CoalesceChain],
        find_conditions: &[FindCondition],
        match_conditions: &[MatchCondition],
    ) -> Option<MoveTarget> {
        if current_idx + 1 < siblings.len() {
            // There's a next sibling
            let next_sibling = siblings[current_idx + 1];
            let next_child_scopes = child_scopes(
                next_sibling,
                match_branches,
                coalesce_chains,
                find_conditions,
                match_conditions,
            );

            if !next_child_scopes.is_empty() {
                // Next sibling is a scope-owner — enter its first child scope
                if let Some(entered) = enter_scope(
                    next_sibling,
                    true, // entering first child
                    all_transforms,
                    match_branches,
                    coalesce_chains,
                    find_conditions,
                    match_conditions,
                ) {
                    return Some(entered);
                }
            }

            // Normal same-scope swap
            Some(MoveTarget {
                parent_type: transform.parent_type,
                parent_id: transform.parent_id,
                order: (current_idx + 1) as i32,
            })
        } else {
            // At end of scope — exit to parent scope (insert after parent transform)
            self.exit_scope_down(
                transform.parent_type,
                transform.parent_id,
                all_transforms,
                match_branches,
                coalesce_chains,
                find_conditions,
                match_conditions,
            )
        }
    }

    /// Find the target slot when moving a transform UP.
    fn find_move_target_up(
        &self,
        transform: &Transform,
        current_idx: usize,
        siblings: &[&Transform],
        all_transforms: &[Transform],
        match_branches: &[MatchBranch],
        coalesce_chains: &[CoalesceChain],
        find_conditions: &[FindCondition],
        match_conditions: &[MatchCondition],
    ) -> Option<MoveTarget> {
        if current_idx > 0 {
            // There's a previous sibling
            let prev_sibling = siblings[current_idx - 1];
            let prev_child_scopes = child_scopes(
                prev_sibling,
                match_branches,
                coalesce_chains,
                find_conditions,
                match_conditions,
            );

            if !prev_child_scopes.is_empty() {
                // Previous sibling is a scope-owner — enter its last child scope (recursively)
                if let Some(entered) = enter_scope(
                    prev_sibling,
                    false, // entering last child
                    all_transforms,
                    match_branches,
                    coalesce_chains,
                    find_conditions,
                    match_conditions,
                ) {
                    return Some(entered);
                }
            }

            // Normal same-scope swap
            Some(MoveTarget {
                parent_type: transform.parent_type,
                parent_id: transform.parent_id,
                order: (current_idx - 1) as i32,
            })
        } else {
            // At start of scope — exit to parent scope (insert at parent transform's position)
            self.exit_scope_up(
                transform.parent_type,
                transform.parent_id,
                all_transforms,
                match_branches,
                coalesce_chains,
                find_conditions,
                match_conditions,
            )
        }
    }

    /// Exit a scope downward: move after the parent transform in its scope.
    ///
    /// If this is the last sibling scope, the transform exits fully.
    /// If there's a next sibling scope, it enters that scope at position 0.
    fn exit_scope_down(
        &self,
        pt: ParentType,
        pid: i64,
        all_transforms: &[Transform],
        match_branches: &[MatchBranch],
        coalesce_chains: &[CoalesceChain],
        find_conditions: &[FindCondition],
        match_conditions: &[MatchCondition],
    ) -> Option<MoveTarget> {
        let (parent_id, parent_pt, parent_pid) = parent_scope(
            pt,
            pid,
            all_transforms,
            match_branches,
            coalesce_chains,
            find_conditions,
        )?;

        let parent_transform = all_transforms.iter().find(|t| t.id == parent_id)?;

        // Check if there's a next sibling scope
        let scope_idx = scope_index_in_parent(
            pt,
            pid,
            parent_transform,
            match_branches,
            coalesce_chains,
            find_conditions,
            match_conditions,
        )?;

        let all_scopes = child_scopes(
            parent_transform,
            match_branches,
            coalesce_chains,
            find_conditions,
            match_conditions,
        );

        if scope_idx + 1 < all_scopes.len() {
            // There's a next sibling scope — enter it at position 0
            let (next_pt, next_pid) = all_scopes[scope_idx + 1];
            let next_transforms = transforms_in_scope(all_transforms, next_pt, next_pid);

            // Check if the first transform in the next scope is a scope-owner to recurse into
            if let Some(first) = next_transforms.first() {
                let first_child_scopes = child_scopes(
                    first,
                    match_branches,
                    coalesce_chains,
                    find_conditions,
                    match_conditions,
                );
                if !first_child_scopes.is_empty() {
                    if let Some(entered) = enter_scope(
                        first,
                        true,
                        all_transforms,
                        match_branches,
                        coalesce_chains,
                        find_conditions,
                        match_conditions,
                    ) {
                        return Some(entered);
                    }
                }
            }

            Some(MoveTarget {
                parent_type: next_pt,
                parent_id: next_pid,
                order: 0,
            })
        } else {
            // Last sibling scope — exit fully, insert after parent transform
            Some(MoveTarget {
                parent_type: parent_pt,
                parent_id: parent_pid,
                order: parent_transform.order + 1,
            })
        }
    }

    /// Exit a scope upward: move before the parent transform in its scope.
    ///
    /// If this is the first sibling scope, the transform exits fully.
    /// If there's a previous sibling scope, it enters that scope at the end (recursively).
    fn exit_scope_up(
        &self,
        pt: ParentType,
        pid: i64,
        all_transforms: &[Transform],
        match_branches: &[MatchBranch],
        coalesce_chains: &[CoalesceChain],
        find_conditions: &[FindCondition],
        match_conditions: &[MatchCondition],
    ) -> Option<MoveTarget> {
        let (parent_id, parent_pt, parent_pid) = parent_scope(
            pt,
            pid,
            all_transforms,
            match_branches,
            coalesce_chains,
            find_conditions,
        )?;

        let parent_transform = all_transforms.iter().find(|t| t.id == parent_id)?;

        // Check if there's a previous sibling scope
        let scope_idx = scope_index_in_parent(
            pt,
            pid,
            parent_transform,
            match_branches,
            coalesce_chains,
            find_conditions,
            match_conditions,
        )?;

        let all_scopes = child_scopes(
            parent_transform,
            match_branches,
            coalesce_chains,
            find_conditions,
            match_conditions,
        );

        if scope_idx > 0 {
            // There's a previous sibling scope — enter it at the end (recursively)
            let (prev_pt, prev_pid) = all_scopes[scope_idx - 1];
            let prev_transforms = transforms_in_scope(all_transforms, prev_pt, prev_pid);

            // Check if the last transform in the previous scope is a scope-owner to recurse into
            if let Some(last) = prev_transforms.last() {
                let last_child_scopes = child_scopes(
                    last,
                    match_branches,
                    coalesce_chains,
                    find_conditions,
                    match_conditions,
                );
                if !last_child_scopes.is_empty() {
                    if let Some(entered) = enter_scope(
                        last,
                        false,
                        all_transforms,
                        match_branches,
                        coalesce_chains,
                        find_conditions,
                        match_conditions,
                    ) {
                        return Some(entered);
                    }
                }
            }

            // Land at end of previous scope
            let end_order = prev_transforms.last().map(|t| t.order + 1).unwrap_or(0);
            Some(MoveTarget {
                parent_type: prev_pt,
                parent_id: prev_pid,
                order: end_order,
            })
        } else {
            // First sibling scope — exit fully, insert at parent transform's position
            Some(MoveTarget {
                parent_type: parent_pt,
                parent_id: parent_pid,
                order: parent_transform.order,
            })
        }
    }

    // =========================================================================
    // Edit Transform
    // =========================================================================

    /// Edit a transform by showing the appropriate modal based on its type.
    pub(super) async fn edit_transform_impl(&self, tn: &TransformNode, gx: &GlobalContext) {
        let transform = &tn.transform;

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
        let mut variables: Vec<VariableInfo> = self
            .variables
            .get()
            .iter()
            .filter(|v| v.entity_mapping_id == entity_mapping.id)
            .map(|v| VariableInfo {
                name: v.name.clone(),
                declared_type: v.declared_type.clone(),
            })
            .collect();

        // Add #value as a variable so autocomplete can navigate into it
        if let Some(input_type) = &tn.input_type {
            variables.push(VariableInfo {
                name: "#value".to_string(),
                declared_type: input_type.clone(),
            });
        }

        // Dispatch based on transform type
        match &transform.data {
            TransformData::Copy { path } => {
                let modal = CopyTransformModal::new_modal(
                    self.source_client.get().clone(),
                    self.target_client.get().clone(),
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
                let modal =
                    ConstantTransformModal::new_modal(self.target_client.get(), value.clone());

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
                let modal = StringOpsTransformModal::new_modal(*op);

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
                    self.target_client.get().clone(),
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
                    self.target_client.get().clone(),
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
            TransformData::Find {
                entity,
                fallback,
                mode,
            } => {
                let Some(target_entities) = self.fetch_target_entities(gx).await else {
                    return;
                };
                let modal = FindTransformModal::edit_modal(target_entities, entity, fallback, mode);

                if let Some(result) = gx.modal(modal).await {
                    let old_had_default = *fallback == FindFallback::Default;
                    let new_has_default = result.fallback == FindFallback::Default;

                    // Disabling default: confirm + delete FindDefault chain transforms
                    if old_had_default && !new_has_default {
                        let repo = gx.data::<MigrationRepository>();
                        let default_transforms = repo
                            .get_transforms(ParentType::FindDefault, transform.id)
                            .await
                            .unwrap_or_default();

                        if !default_transforms.is_empty() {
                            let confirmed = gx
                                .modal(ConfirmModal::with_message(format!(
                                    "Removing the default fallback will delete {} transform(s). Continue?",
                                    default_transforms.len()
                                )))
                                .await;

                            if !confirmed {
                                return;
                            }

                            for t in &default_transforms {
                                if let Err(e) = repo.delete_transform(t.id).await {
                                    log::error!("Failed to delete find default transform: {}", e);
                                }
                            }
                        }
                    }

                    // Mode change from Where to Lua: delete find conditions
                    let old_is_where = matches!(mode, FindMode::Where);
                    let new_is_lua = matches!(result.mode, FindMode::Lua { .. });
                    if old_is_where && new_is_lua {
                        let repo = gx.data::<MigrationRepository>();
                        let conditions = self
                            .find_conditions
                            .get()
                            .iter()
                            .filter(|fc| fc.transform_id == transform.id)
                            .cloned()
                            .collect::<Vec<_>>();

                        if !conditions.is_empty() {
                            let confirmed = gx
                                .modal(ConfirmModal::with_message(format!(
                                    "Switching to Lua will delete {} condition(s). Continue?",
                                    conditions.len()
                                )))
                                .await;

                            if !confirmed {
                                return;
                            }

                            for fc in &conditions {
                                // Delete condition's child transforms first
                                let child_transforms = repo
                                    .get_transforms(ParentType::FindCondition, fc.id)
                                    .await
                                    .unwrap_or_default();
                                for ct in &child_transforms {
                                    if let Err(e) = repo.delete_transform(ct.id).await {
                                        log::error!("Failed to delete condition transform: {}", e);
                                    }
                                }
                                if let Err(e) = repo.delete_find_condition(fc.id).await {
                                    log::error!("Failed to delete find condition: {}", e);
                                }
                            }
                        }
                    }

                    self.update_transform_data(
                        transform.id,
                        TransformData::Find {
                            entity: result.entity,
                            fallback: result.fallback,
                            mode: result.mode,
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
