//! Materializer — converts flat DB rows into `ChainItem` trees.
//!
//! Takes all pre-loaded rows for one entity mapping (transforms, match branches,
//! coalesce chains, find conditions) and builds the nested `ChainItem` structures
//! needed for execution.

use std::collections::HashMap;

use crate::apps::migration::types::CoalesceChain;
use crate::apps::migration::types::FindCondition;
use crate::apps::migration::types::FindFallback;
use crate::apps::migration::types::FindMode;
use crate::apps::migration::types::MatchBranch;
use crate::apps::migration::types::ParentType;
use crate::apps::migration::types::Transform;
use crate::apps::migration::types::TransformData;

use super::executor::BranchItem;
use super::executor::ChainItem;
use super::executor::FindConditionItem;

// =============================================================================
// Input Data
// =============================================================================

/// All flat DB rows for one entity mapping, pre-loaded and ready for materialization.
pub struct MaterializeData {
    /// All transforms, indexed by (parent_type, parent_id).
    transforms: HashMap<(ParentType, i64), Vec<Transform>>,
    /// Match branches, indexed by transform_id.
    match_branches: HashMap<i64, Vec<MatchBranch>>,
    /// Coalesce chains, indexed by transform_id.
    coalesce_chains: HashMap<i64, Vec<CoalesceChain>>,
    /// Find conditions, indexed by transform_id.
    find_conditions: HashMap<i64, Vec<FindCondition>>,
}

impl MaterializeData {
    /// Build indexed data from flat row lists.
    ///
    /// All vectors should be pre-sorted by `order`, as they come from the DB.
    pub fn new(
        transforms: Vec<Transform>,
        match_branches: Vec<MatchBranch>,
        coalesce_chains: Vec<CoalesceChain>,
        find_conditions: Vec<FindCondition>,
    ) -> Self {
        let mut transform_index: HashMap<(ParentType, i64), Vec<Transform>> = HashMap::new();
        for t in transforms {
            transform_index
                .entry((t.parent_type, t.parent_id))
                .or_default()
                .push(t);
        }

        let mut branch_index: HashMap<i64, Vec<MatchBranch>> = HashMap::new();
        for b in match_branches {
            branch_index.entry(b.transform_id).or_default().push(b);
        }

        let mut coalesce_index: HashMap<i64, Vec<CoalesceChain>> = HashMap::new();
        for c in coalesce_chains {
            coalesce_index.entry(c.transform_id).or_default().push(c);
        }

        let mut find_index: HashMap<i64, Vec<FindCondition>> = HashMap::new();
        for f in find_conditions {
            find_index.entry(f.transform_id).or_default().push(f);
        }

        Self {
            transforms: transform_index,
            match_branches: branch_index,
            coalesce_chains: coalesce_index,
            find_conditions: find_index,
        }
    }
}

// =============================================================================
// Materialization
// =============================================================================

/// Materialize a transform chain for the given parent.
///
/// Returns the list of `ChainItem`s in execution order.
pub fn materialize_chain(
    parent_type: ParentType,
    parent_id: i64,
    data: &MaterializeData,
) -> Vec<ChainItem> {
    let transforms = match data.transforms.get(&(parent_type, parent_id)) {
        Some(ts) => ts,
        None => return Vec::new(),
    };

    transforms
        .iter()
        .map(|t| materialize_item(t, data))
        .collect()
}

/// Materialize a single transform into a `ChainItem` with resolved children.
fn materialize_item(transform: &Transform, data: &MaterializeData) -> ChainItem {
    match &transform.data {
        TransformData::Guard { .. } => {
            let fallback = materialize_chain(ParentType::GuardFallback, transform.id, data);
            ChainItem::with_fallback(transform.data.clone(), fallback)
        }

        TransformData::Match { has_default } => {
            let branches = data
                .match_branches
                .get(&transform.id)
                .map(|bs| {
                    bs.iter()
                        .map(|b| BranchItem {
                            condition: b.condition.clone(),
                            chain: materialize_chain(ParentType::MatchBranch, b.id, data),
                        })
                        .collect()
                })
                .unwrap_or_default();

            let default_chain = if *has_default {
                let chain = materialize_chain(ParentType::MatchDefault, transform.id, data);
                if chain.is_empty() { None } else { Some(chain) }
            } else {
                None
            };

            ChainItem::with_branches(transform.data.clone(), branches, default_chain)
        }

        TransformData::Coalesce => {
            let alternatives = data
                .coalesce_chains
                .get(&transform.id)
                .map(|cs| {
                    cs.iter()
                        .map(|c| materialize_chain(ParentType::CoalesceChain, c.id, data))
                        .collect()
                })
                .unwrap_or_default();

            ChainItem::with_alternatives(transform.data.clone(), alternatives)
        }

        TransformData::Find { fallback, mode, .. } => {
            let conditions = match mode {
                FindMode::Where => data
                    .find_conditions
                    .get(&transform.id)
                    .map(|fs| {
                        fs.iter()
                            .map(|f| FindConditionItem {
                                target_field: f.target_field.clone(),
                                source_chain: materialize_chain(
                                    ParentType::FindCondition,
                                    f.id,
                                    data,
                                ),
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                FindMode::Lua { .. } => Vec::new(),
            };

            let default_chain = if *fallback == FindFallback::Default {
                let chain = materialize_chain(ParentType::FindDefault, transform.id, data);
                if chain.is_empty() { None } else { Some(chain) }
            } else {
                None
            };

            ChainItem::with_find_conditions(transform.data.clone(), conditions, default_chain)
        }

        // All other transforms have no children
        _ => ChainItem::new(transform.data.clone()),
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use crate::apps::migration::types::CompareOp;
    use crate::apps::migration::types::Condition;
    use crate::apps::migration::types::Expr;

    use super::*;

    fn make_transform(
        id: i64,
        parent_type: ParentType,
        parent_id: i64,
        order: i32,
        data: TransformData,
    ) -> Transform {
        Transform {
            id,
            entity_mapping_id: 1,
            parent_type,
            parent_id,
            order,
            data,
        }
    }

    #[test]
    fn simple_chain() {
        let transforms = vec![
            make_transform(
                1,
                ParentType::FieldMapping,
                10,
                0,
                TransformData::Copy {
                    path: "name".to_string(),
                },
            ),
            make_transform(
                2,
                ParentType::FieldMapping,
                10,
                1,
                TransformData::StringOps {
                    op: crate::apps::migration::types::StringOp::Uppercase,
                },
            ),
        ];

        let data = MaterializeData::new(transforms, vec![], vec![], vec![]);
        let chain = materialize_chain(ParentType::FieldMapping, 10, &data);

        assert_eq!(chain.len(), 2);
        assert!(matches!(&chain[0].data, TransformData::Copy { path } if path == "name"));
        assert!(matches!(&chain[0].children, ChainChildren::None));
        assert!(matches!(&chain[1].data, TransformData::StringOps { .. }));
    }

    #[test]
    fn empty_parent_returns_empty() {
        let data = MaterializeData::new(vec![], vec![], vec![], vec![]);
        let chain = materialize_chain(ParentType::FieldMapping, 99, &data);
        assert!(chain.is_empty());
    }

    #[test]
    fn guard_with_fallback() {
        let condition = Condition::IsNotNull(Expr::Path("name".to_string()));
        let transforms = vec![
            // Guard in main chain
            make_transform(
                1,
                ParentType::FieldMapping,
                10,
                0,
                TransformData::Guard {
                    condition: condition.clone(),
                },
            ),
            // Fallback chain for the guard
            make_transform(
                2,
                ParentType::GuardFallback,
                1,
                0,
                TransformData::Constant {
                    value: dataverse_lib::model::Value::String("fallback".to_string()),
                },
            ),
        ];

        let data = MaterializeData::new(transforms, vec![], vec![], vec![]);
        let chain = materialize_chain(ParentType::FieldMapping, 10, &data);

        assert_eq!(chain.len(), 1);
        match &chain[0].children {
            ChainChildren::Fallback(fb) => {
                assert_eq!(fb.len(), 1);
                assert!(matches!(&fb[0].data, TransformData::Constant { .. }));
            }
            other => panic!("Expected Fallback, got {:?}", other),
        }
    }

    #[test]
    fn match_with_branches_and_default() {
        let cond_a = Condition::Compare {
            left: Expr::Path("status".to_string()),
            op: CompareOp::Equal,
            right: Expr::Literal(dataverse_lib::model::Value::Int(1)),
        };

        let transforms = vec![
            // Match in main chain
            make_transform(
                1,
                ParentType::FieldMapping,
                10,
                0,
                TransformData::Match { has_default: true },
            ),
            // Branch A chain
            make_transform(
                10,
                ParentType::MatchBranch,
                100,
                0,
                TransformData::Constant {
                    value: dataverse_lib::model::Value::String("active".to_string()),
                },
            ),
            // Default chain
            make_transform(
                20,
                ParentType::MatchDefault,
                1,
                0,
                TransformData::Constant {
                    value: dataverse_lib::model::Value::String("unknown".to_string()),
                },
            ),
        ];

        let branches = vec![MatchBranch {
            id: 100,
            transform_id: 1,
            order: 0,
            condition: cond_a,
        }];

        let data = MaterializeData::new(transforms, branches, vec![], vec![]);
        let chain = materialize_chain(ParentType::FieldMapping, 10, &data);

        assert_eq!(chain.len(), 1);
        match &chain[0].children {
            ChainChildren::Branches(bs, default) => {
                assert_eq!(bs.len(), 1);
                assert_eq!(bs[0].chain.len(), 1);
                assert!(default.is_some());
                assert_eq!(default.as_ref().unwrap().len(), 1);
            }
            other => panic!("Expected Branches, got {:?}", other),
        }
    }

    #[test]
    fn coalesce_with_alternatives() {
        let transforms = vec![
            make_transform(1, ParentType::FieldMapping, 10, 0, TransformData::Coalesce),
            // Alt 1
            make_transform(
                10,
                ParentType::CoalesceChain,
                100,
                0,
                TransformData::Copy {
                    path: "primary_email".to_string(),
                },
            ),
            // Alt 2
            make_transform(
                20,
                ParentType::CoalesceChain,
                200,
                0,
                TransformData::Copy {
                    path: "secondary_email".to_string(),
                },
            ),
        ];

        let coalesce_chains = vec![
            CoalesceChain {
                id: 100,
                transform_id: 1,
                order: 0,
            },
            CoalesceChain {
                id: 200,
                transform_id: 1,
                order: 1,
            },
        ];

        let data = MaterializeData::new(transforms, vec![], coalesce_chains, vec![]);
        let chain = materialize_chain(ParentType::FieldMapping, 10, &data);

        assert_eq!(chain.len(), 1);
        match &chain[0].children {
            ChainChildren::Alternatives(alts) => {
                assert_eq!(alts.len(), 2);
                assert_eq!(alts[0].len(), 1);
                assert_eq!(alts[1].len(), 1);
            }
            other => panic!("Expected Alternatives, got {:?}", other),
        }
    }

    #[test]
    fn find_where_with_conditions() {
        let transforms = vec![
            make_transform(
                1,
                ParentType::FieldMapping,
                10,
                0,
                TransformData::Find {
                    entity: "systemuser".to_string(),
                    fallback: FindFallback::Error,
                    mode: FindMode::Where,
                },
            ),
            // Condition source chain
            make_transform(
                10,
                ParentType::FindCondition,
                100,
                0,
                TransformData::Copy {
                    path: "emailaddress1".to_string(),
                },
            ),
        ];

        let find_conditions = vec![FindCondition {
            id: 100,
            transform_id: 1,
            target_field: "internalemailaddress".to_string(),
            order: 0,
        }];

        let data = MaterializeData::new(transforms, vec![], vec![], find_conditions);
        let chain = materialize_chain(ParentType::FieldMapping, 10, &data);

        assert_eq!(chain.len(), 1);
        match &chain[0].children {
            ChainChildren::FindConditions(conds, default) => {
                assert_eq!(conds.len(), 1);
                assert_eq!(conds[0].target_field, "internalemailaddress");
                assert_eq!(conds[0].source_chain.len(), 1);
                assert!(default.is_none());
            }
            other => panic!("Expected FindConditions, got {:?}", other),
        }
    }
}
