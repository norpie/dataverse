//! Yank (copy) and paste for transforms and their child subtrees.

use rafter::prelude::*;

use crate::apps::migration::repository::MigrationRepository;
use crate::apps::migration::repository::NewCoalesceChain;
use crate::apps::migration::repository::NewFindCondition;
use crate::apps::migration::repository::NewMatchBranch;
use crate::apps::migration::repository::NewTransform;
use crate::apps::migration::types::CoalesceChain;
use crate::apps::migration::types::Condition;
use crate::apps::migration::types::FindCondition;
use crate::apps::migration::types::FindFallback;
use crate::apps::migration::types::FindMode;
use crate::apps::migration::types::MatchBranch;
use crate::apps::migration::types::ParentType;
use crate::apps::migration::types::Transform;
use crate::apps::migration::types::TransformData;

use super::MigrationEditor;
use super::tree::MigrationTreeNode;

/// A yanked transform with its full subtree (children recursively included).
#[derive(Clone, Debug)]
pub struct YankedTransform {
    /// The transform data (type + config).
    pub data: TransformData,
    /// Child scopes owned by this transform.
    pub children: Vec<YankedChild>,
}

/// A yanked child scope.
#[derive(Clone, Debug)]
pub enum YankedChild {
    /// A match branch with its condition and nested transform chain.
    MatchBranch {
        condition: Condition,
        transforms: Vec<YankedTransform>,
    },
    /// A coalesce fallback chain with its nested transforms.
    CoalesceChain {
        transforms: Vec<YankedTransform>,
    },
    /// A find condition with its target field and nested transform chain.
    FindCondition {
        target_field: String,
        transforms: Vec<YankedTransform>,
    },
    /// A guard's fallback chain.
    GuardFallback {
        transforms: Vec<YankedTransform>,
    },
    /// A match's default branch chain.
    MatchDefault {
        transforms: Vec<YankedTransform>,
    },
    /// A find's default chain.
    FindDefault {
        transforms: Vec<YankedTransform>,
    },
}

impl YankedTransform {
    /// Short label for toast messages.
    pub fn label(&self) -> &'static str {
        match &self.data {
            TransformData::Copy { .. } => "copy",
            TransformData::Constant { .. } => "constant",
            TransformData::Guard { .. } => "guard",
            TransformData::Match { .. } => "match",
            TransformData::Find { .. } => "find",
            TransformData::Format { .. } => "format",
            TransformData::Replace { .. } => "replace",
            TransformData::StringOps { .. } => "string_ops",
            TransformData::ValueMap { .. } => "value_map",
            TransformData::Math { .. } => "math",
            TransformData::Coalesce => "coalesce",
            TransformData::Convert { .. } => "convert",
            TransformData::ParseInt => "parse_int",
            TransformData::ParseDecimal => "parse_decimal",
            TransformData::ParseDate { .. } => "parse_date",
            TransformData::Guid => "guid",
        }
    }
}

// =============================================================================
// Build yanked tree from in-memory data
// =============================================================================

/// Build a `YankedTransform` from a transform and its in-memory siblings.
///
/// Recursively collects all child scopes (match branches, coalesce chains,
/// find conditions, guard/match/find defaults) and their nested transforms.
pub fn build_yanked_tree(
    transform: &Transform,
    all_transforms: &[Transform],
    all_match_branches: &[MatchBranch],
    all_coalesce_chains: &[CoalesceChain],
    all_find_conditions: &[FindCondition],
) -> YankedTransform {
    let children = match &transform.data {
        TransformData::Guard { .. } => {
            // Guard has one fallback chain: (GuardFallback, transform.id)
            let fallback_transforms =
                get_sorted_transforms(all_transforms, ParentType::GuardFallback, transform.id);
            let yanked = yank_chain(
                &fallback_transforms,
                all_transforms,
                all_match_branches,
                all_coalesce_chains,
                all_find_conditions,
            );
            if yanked.is_empty() {
                vec![]
            } else {
                vec![YankedChild::GuardFallback {
                    transforms: yanked,
                }]
            }
        }
        TransformData::Match { has_default } => {
            let mut children = Vec::new();

            // Branches
            let mut branches: Vec<_> = all_match_branches
                .iter()
                .filter(|mb| mb.transform_id == transform.id)
                .collect();
            branches.sort_by_key(|mb| mb.order);

            for mb in &branches {
                let branch_transforms =
                    get_sorted_transforms(all_transforms, ParentType::MatchBranch, mb.id);
                let yanked = yank_chain(
                    &branch_transforms,
                    all_transforms,
                    all_match_branches,
                    all_coalesce_chains,
                    all_find_conditions,
                );
                children.push(YankedChild::MatchBranch {
                    condition: mb.condition.clone(),
                    transforms: yanked,
                });
            }

            // Default
            if *has_default {
                let default_transforms =
                    get_sorted_transforms(all_transforms, ParentType::MatchDefault, transform.id);
                let yanked = yank_chain(
                    &default_transforms,
                    all_transforms,
                    all_match_branches,
                    all_coalesce_chains,
                    all_find_conditions,
                );
                children.push(YankedChild::MatchDefault {
                    transforms: yanked,
                });
            }

            children
        }
        TransformData::Coalesce => {
            let mut chains: Vec<_> = all_coalesce_chains
                .iter()
                .filter(|cc| cc.transform_id == transform.id)
                .collect();
            chains.sort_by_key(|cc| cc.order);

            chains
                .iter()
                .map(|cc| {
                    let chain_transforms =
                        get_sorted_transforms(all_transforms, ParentType::CoalesceChain, cc.id);
                    let yanked = yank_chain(
                        &chain_transforms,
                        all_transforms,
                        all_match_branches,
                        all_coalesce_chains,
                        all_find_conditions,
                    );
                    YankedChild::CoalesceChain {
                        transforms: yanked,
                    }
                })
                .collect()
        }
        TransformData::Find { fallback, mode, .. } => {
            let mut children = Vec::new();

            // Conditions (only for Where mode)
            if matches!(mode, FindMode::Where) {
                let mut conditions: Vec<_> = all_find_conditions
                    .iter()
                    .filter(|fc| fc.transform_id == transform.id)
                    .collect();
                conditions.sort_by_key(|fc| fc.order);

                for fc in &conditions {
                    let cond_transforms =
                        get_sorted_transforms(all_transforms, ParentType::FindCondition, fc.id);
                    let yanked = yank_chain(
                        &cond_transforms,
                        all_transforms,
                        all_match_branches,
                        all_coalesce_chains,
                        all_find_conditions,
                    );
                    children.push(YankedChild::FindCondition {
                        target_field: fc.target_field.clone(),
                        transforms: yanked,
                    });
                }
            }

            // Default
            if matches!(fallback, FindFallback::Default) {
                let default_transforms =
                    get_sorted_transforms(all_transforms, ParentType::FindDefault, transform.id);
                let yanked = yank_chain(
                    &default_transforms,
                    all_transforms,
                    all_match_branches,
                    all_coalesce_chains,
                    all_find_conditions,
                );
                children.push(YankedChild::FindDefault {
                    transforms: yanked,
                });
            }

            children
        }
        // Leaf transforms have no children
        _ => vec![],
    };

    YankedTransform {
        data: transform.data.clone(),
        children,
    }
}

/// Get transforms for a (parent_type, parent_id) pair, sorted by order.
fn get_sorted_transforms(
    all_transforms: &[Transform],
    parent_type: ParentType,
    parent_id: i64,
) -> Vec<Transform> {
    let mut transforms: Vec<_> = all_transforms
        .iter()
        .filter(|t| t.parent_type == parent_type && t.parent_id == parent_id)
        .cloned()
        .collect();
    transforms.sort_by_key(|t| t.order);
    transforms
}

/// Recursively yank a chain of transforms.
fn yank_chain(
    transforms: &[Transform],
    all_transforms: &[Transform],
    all_match_branches: &[MatchBranch],
    all_coalesce_chains: &[CoalesceChain],
    all_find_conditions: &[FindCondition],
) -> Vec<YankedTransform> {
    transforms
        .iter()
        .map(|t| {
            build_yanked_tree(
                t,
                all_transforms,
                all_match_branches,
                all_coalesce_chains,
                all_find_conditions,
            )
        })
        .collect()
}

// =============================================================================
// Paste yanked tree into DB (iterative work queue)
// =============================================================================

/// A pending work item for the paste queue.
enum PasteWork<'a> {
    /// Insert a transform under the given parent.
    Transform {
        yanked: &'a YankedTransform,
        parent_type: ParentType,
        parent_id: i64,
        order: i32,
    },
    /// Process the children of a just-inserted transform.
    Children {
        yanked: &'a YankedTransform,
        transform_id: i64,
    },
}

/// Insert the full yanked tree into the DB using an iterative work queue.
///
/// Returns `Ok(root_transform_id)` on success.
pub async fn paste_yanked_tree(
    yanked: &YankedTransform,
    entity_mapping_id: i64,
    parent_type: ParentType,
    parent_id: i64,
    order: i32,
    repo: &MigrationRepository,
) -> Result<i64, String> {
    let mut root_id: Option<i64> = None;
    let mut queue: Vec<PasteWork<'_>> = vec![PasteWork::Transform {
        yanked,
        parent_type,
        parent_id,
        order,
    }];

    while let Some(work) = queue.pop() {
        match work {
            PasteWork::Transform {
                yanked: yt,
                parent_type: pt,
                parent_id: pid,
                order: ord,
            } => {
                let transform_id = repo
                    .create_transform(NewTransform {
                        entity_mapping_id,
                        parent_type: pt,
                        parent_id: pid,
                        order: ord,
                        data: yt.data.clone(),
                    })
                    .await
                    .map_err(|e| format!("Failed to create transform: {}", e))?;

                if root_id.is_none() {
                    root_id = Some(transform_id);
                }

                // Schedule children processing (push to front by inserting at end,
                // since we pop from end — this processes children before next sibling)
                if !yt.children.is_empty() {
                    queue.push(PasteWork::Children {
                        yanked: yt,
                        transform_id,
                    });
                }
            }
            PasteWork::Children {
                yanked: yt,
                transform_id: tid,
            } => {
                // Process children in reverse so they end up in correct order
                // when popped from the stack.
                let mut child_work = Vec::new();

                for child in &yt.children {
                    match child {
                        YankedChild::GuardFallback { transforms } => {
                            enqueue_chain(transforms, ParentType::GuardFallback, tid, &mut child_work);
                        }
                        YankedChild::MatchBranch {
                            condition,
                            transforms,
                        } => {
                            let branch_count = repo
                                .get_match_branches(tid)
                                .await
                                .map(|b| b.len())
                                .unwrap_or(0) as i32;

                            let branch_id = repo
                                .create_match_branch(NewMatchBranch {
                                    transform_id: tid,
                                    order: branch_count,
                                    condition: condition.clone(),
                                })
                                .await
                                .map_err(|e| format!("Failed to create match branch: {}", e))?;

                            enqueue_chain(transforms, ParentType::MatchBranch, branch_id, &mut child_work);
                        }
                        YankedChild::MatchDefault { transforms } => {
                            enqueue_chain(transforms, ParentType::MatchDefault, tid, &mut child_work);
                        }
                        YankedChild::CoalesceChain { transforms } => {
                            let chain_count = repo
                                .get_coalesce_chains(tid)
                                .await
                                .map(|c| c.len())
                                .unwrap_or(0) as i32;

                            let chain_id = repo
                                .create_coalesce_chain(NewCoalesceChain {
                                    transform_id: tid,
                                    order: chain_count,
                                })
                                .await
                                .map_err(|e| format!("Failed to create coalesce chain: {}", e))?;

                            enqueue_chain(transforms, ParentType::CoalesceChain, chain_id, &mut child_work);
                        }
                        YankedChild::FindCondition {
                            target_field,
                            transforms,
                        } => {
                            let cond_count = repo
                                .get_find_conditions(tid)
                                .await
                                .map(|c| c.len())
                                .unwrap_or(0) as i32;

                            let cond_id = repo
                                .create_find_condition(NewFindCondition {
                                    transform_id: tid,
                                    target_field: target_field.clone(),
                                    order: cond_count,
                                })
                                .await
                                .map_err(|e| format!("Failed to create find condition: {}", e))?;

                            enqueue_chain(transforms, ParentType::FindCondition, cond_id, &mut child_work);
                        }
                        YankedChild::FindDefault { transforms } => {
                            enqueue_chain(transforms, ParentType::FindDefault, tid, &mut child_work);
                        }
                    }
                }

                // Reverse so first child is processed first (stack is LIFO)
                child_work.reverse();
                queue.extend(child_work);
            }
        }
    }

    root_id.ok_or_else(|| "No transforms were inserted".to_string())
}

/// Push transform work items for a chain onto the work list.
fn enqueue_chain<'a>(
    transforms: &'a [YankedTransform],
    parent_type: ParentType,
    parent_id: i64,
    work: &mut Vec<PasteWork<'a>>,
) {
    for (i, yt) in transforms.iter().enumerate() {
        work.push(PasteWork::Transform {
            yanked: yt,
            parent_type,
            parent_id,
            order: i as i32,
        });
    }
}

// =============================================================================
// Handler implementations
// =============================================================================

impl MigrationEditor {
    /// Yank the focused transform and its subtree.
    pub(super) async fn yank_item_impl(&self, gx: &GlobalContext) {
        let focused = self.focused_node();

        let Some(MigrationTreeNode::Transform(tn)) = focused else {
            gx.toast(Toast::warning("Nothing to yank"));
            return;
        };

        let transforms = self.transforms.get();
        let match_branches = self.match_branches.get();
        let coalesce_chains = self.coalesce_chains.get();
        let find_conditions = self.find_conditions.get();

        let yanked = build_yanked_tree(
            &tn.transform,
            &transforms,
            &match_branches,
            &coalesce_chains,
            &find_conditions,
        );

        let label = yanked.label();
        self.yanked.set(Some(yanked));
        gx.toast(Toast::info(format!("Yanked: {}", label)));
    }

    /// Paste the yanked transform at the current focus position.
    pub(super) async fn paste_item_impl(&self, gx: &GlobalContext) {
        let yanked = self.yanked.get();
        let Some(yanked) = yanked else {
            gx.toast(Toast::warning("Nothing to paste"));
            return;
        };

        let Some(target) = self.get_transform_insert_target() else {
            gx.toast(Toast::warning("Cannot paste here"));
            return;
        };

        let repo = gx.data::<MigrationRepository>();

        match paste_yanked_tree(
            &yanked,
            target.entity_mapping_id,
            target.parent_type,
            target.parent_id,
            target.insert_order,
            &repo,
        )
        .await
        {
            Ok(_id) => {
                // Reorder siblings to fix gaps
                if let Ok(remaining) = repo
                    .get_transforms(target.parent_type, target.parent_id)
                    .await
                {
                    let ordered_ids: Vec<i64> = remaining.iter().map(|t| t.id).collect();
                    let _ = repo
                        .reorder_transforms(target.parent_type, target.parent_id, ordered_ids)
                        .await;
                }

                gx.toast(Toast::info(format!("Pasted: {}", yanked.label())));
                self.load_db_data(gx).await;
            }
            Err(e) => {
                log::error!("Failed to paste transform: {}", e);
                gx.toast(Toast::error("Failed to paste transform"));
            }
        }
    }
}
