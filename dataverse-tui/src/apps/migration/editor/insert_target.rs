//! Insert target resolution for transform operations.
//!
//! Determines where to insert a new transform based on the focused tree node,
//! and resolves entity_mapping_id by walking up the tree hierarchy.

use crate::apps::migration::types::CoalesceChain;
use crate::apps::migration::types::FindCondition;
use crate::apps::migration::types::MatchBranch;
use crate::apps::migration::types::ParentType;

use super::MigrationEditor;
use super::tree::MigrationTreeNode;

/// Information about where to insert a new transform.
pub(super) struct InsertTarget {
    /// The entity mapping this transform belongs to.
    pub entity_mapping_id: i64,
    /// The parent type for the transform.
    pub parent_type: ParentType,
    /// The parent id for the transform.
    pub parent_id: i64,
    /// The order at which to insert (existing transforms at this order and after will be shifted).
    pub insert_order: i32,
}

impl MigrationEditor {
    /// Determine where to insert a new transform based on the focused node.
    pub(super) fn get_transform_insert_target(&self) -> Option<InsertTarget> {
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
            MigrationTreeNode::MatchCondition(mc) => {
                // Add to end of match condition's chain
                let order = self.transform_count_for_parent(ParentType::MatchCondition, mc.id);
                Some(InsertTarget {
                    entity_mapping_id: mc.entity_mapping_id,
                    parent_type: ParentType::MatchCondition,
                    parent_id: mc.id,
                    insert_order: order,
                })
            }
            MigrationTreeNode::MatchDefault { transform_id } => {
                // Add to end of default branch chain
                let order = self.transform_count_for_parent(ParentType::MatchDefault, transform_id);
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
            MigrationTreeNode::FindDefault { transform_id } => {
                // Add to end of find default chain
                let order = self.transform_count_for_parent(ParentType::FindDefault, transform_id);
                let entity_mapping_id = self
                    .transforms
                    .get()
                    .iter()
                    .find(|t| t.id == transform_id)
                    .map(|t| t.entity_mapping_id)?;
                Some(InsertTarget {
                    entity_mapping_id,
                    parent_type: ParentType::FindDefault,
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
    pub(super) fn transform_count_for_parent(
        &self,
        parent_type: ParentType,
        parent_id: i64,
    ) -> i32 {
        self.transforms
            .get()
            .iter()
            .filter(|t| t.parent_type == parent_type && t.parent_id == parent_id)
            .count() as i32
    }

    /// Get entity_mapping_id for a match branch by traversing up to its transform.
    pub(super) fn entity_mapping_id_for_match_branch(&self, mb: &MatchBranch) -> Option<i64> {
        self.transforms
            .get()
            .iter()
            .find(|t| t.id == mb.transform_id)
            .map(|t| t.entity_mapping_id)
    }

    /// Get entity_mapping_id for a coalesce chain by traversing up to its transform.
    pub(super) fn entity_mapping_id_for_coalesce_chain(&self, cc: &CoalesceChain) -> Option<i64> {
        self.transforms
            .get()
            .iter()
            .find(|t| t.id == cc.transform_id)
            .map(|t| t.entity_mapping_id)
    }

    /// Get entity_mapping_id for a find condition by traversing up to its transform.
    pub(super) fn entity_mapping_id_for_find_condition(&self, fc: &FindCondition) -> Option<i64> {
        self.transforms
            .get()
            .iter()
            .find(|t| t.id == fc.transform_id)
            .map(|t| t.entity_mapping_id)
    }

    /// Get entity_mapping_id for a chain wrapper node.
    pub(super) fn entity_mapping_id_for_chain(
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
            ParentType::GuardFallback | ParentType::MatchDefault | ParentType::FindDefault => {
                // parent_id is the transform_id of the guard/match/find
                self.transforms
                    .get()
                    .iter()
                    .find(|t| t.id == parent_id)
                    .map(|t| t.entity_mapping_id)
            }
            ParentType::MatchCondition => {
                // parent_id is the match_condition id
                self.match_conditions
                    .get()
                    .iter()
                    .find(|mc| mc.id == parent_id)
                    .map(|mc| mc.entity_mapping_id)
            }
        }
    }
}
