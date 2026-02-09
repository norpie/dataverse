//! Tree building logic for the migration editor.
//!
//! Converts flat database rows (phases, entity mappings, transforms, etc.)
//! into a hierarchical tree structure with embedded type tracking data.

use std::collections::HashMap;

use dataverse_lib::model::ValueType;
use rafter::widgets::TreeNode;

use crate::apps::migration::types::resolve_branch_union;
use crate::apps::migration::types::ChainOutputWarning;
use crate::apps::migration::types::ChainTypeResult;
use crate::apps::migration::types::CoalesceChain;
use crate::apps::migration::types::EntityMapping;
use crate::apps::migration::types::FieldMapping;
use crate::apps::migration::types::FindCondition;
use crate::apps::migration::types::FindFallback;
use crate::apps::migration::types::MatchBranch;
use crate::apps::migration::types::MatchCondition;
use crate::apps::migration::types::MatchStrategy;
use crate::apps::migration::types::Mode;
use crate::apps::migration::types::ParentType;
use crate::apps::migration::types::Phase;
use crate::apps::migration::types::Transform;
use crate::apps::migration::types::TransformData;
use crate::apps::migration::types::TypeWarning;
use crate::apps::migration::types::Variable;

use super::tree::FieldMappingNode;
use super::tree::FieldTypeCache;
use super::tree::MigrationTreeNode;
use super::tree::TransformNode;
use super::tree::VariableNode;
use super::tree_types::compute_chain_types;

// =============================================================================
// Internal type tracking state
// =============================================================================

/// Internal type tracking state accumulated during tree building.
///
/// This is NOT exposed outside the tree module. Type data is embedded
/// directly on tree nodes (`TransformNode.output_type`, `.warning`,
/// `FieldMappingNode.target_type`, `.warning`).
#[derive(Debug, Default)]
pub(super) struct TypeAccumulator {
    /// Transform ID -> output type after that transform.
    pub(super) transform_types: HashMap<i64, ValueType>,
    /// All type warnings across all chains.
    pub(super) warnings: Vec<TypeWarning>,
    /// Variable name -> resolved output type of its chain.
    pub(super) variable_types: HashMap<String, ValueType>,
}

impl TypeAccumulator {
    /// Get the output type for a transform.
    pub(super) fn type_for(&self, transform_id: i64) -> Option<&ValueType> {
        self.transform_types.get(&transform_id)
    }

    /// Get the warning for a transform, if any.
    pub(super) fn warning_for(&self, transform_id: i64) -> Option<&TypeWarning> {
        self.warnings
            .iter()
            .find(|w| w.transform_id == transform_id)
    }

    /// Merge a chain type result into this accumulator.
    pub(super) fn merge(&mut self, chain_result: &ChainTypeResult) {
        self.transform_types.extend(
            chain_result
                .transform_types
                .iter()
                .map(|(k, v)| (*k, v.clone())),
        );
        self.warnings.extend(chain_result.warnings.iter().cloned());
    }
}

// =============================================================================
// Lookup data
// =============================================================================

/// Immutable lookup data for tree building.
pub(super) struct TreeLookup<'a> {
    pub(super) transforms: &'a [Transform],
    pub(super) match_branches: &'a [MatchBranch],
    pub(super) coalesce_chains: &'a [CoalesceChain],
    pub(super) find_conditions: &'a [FindCondition],
    pub(super) match_conditions: &'a [MatchCondition],
}

impl<'a> TreeLookup<'a> {
    /// Get transforms for a specific parent, sorted by order.
    pub(super) fn get_transforms(&self, parent_type: ParentType, parent_id: i64) -> Vec<Transform> {
        let mut transforms: Vec<_> = self
            .transforms
            .iter()
            .filter(|t| t.parent_type == parent_type && t.parent_id == parent_id)
            .cloned()
            .collect();
        transforms.sort_by_key(|t| t.order);
        transforms
    }

    /// Get match branches for a transform, sorted by order.
    fn get_match_branches(&self, transform_id: i64) -> Vec<MatchBranch> {
        let mut branches: Vec<_> = self
            .match_branches
            .iter()
            .filter(|mb| mb.transform_id == transform_id)
            .cloned()
            .collect();
        branches.sort_by_key(|mb| mb.order);
        branches
    }

    /// Get coalesce chains for a transform, sorted by order.
    fn get_coalesce_chains(&self, transform_id: i64) -> Vec<CoalesceChain> {
        let mut chains: Vec<_> = self
            .coalesce_chains
            .iter()
            .filter(|cc| cc.transform_id == transform_id)
            .cloned()
            .collect();
        chains.sort_by_key(|cc| cc.order);
        chains
    }

    /// Get find conditions for a transform, sorted by order.
    fn get_find_conditions(&self, transform_id: i64) -> Vec<FindCondition> {
        let mut conditions: Vec<_> = self
            .find_conditions
            .iter()
            .filter(|fc| fc.transform_id == transform_id)
            .cloned()
            .collect();
        conditions.sort_by_key(|fc| fc.order);
        conditions
    }

    /// Get match conditions for an entity mapping, sorted by order.
    fn get_match_conditions(&self, entity_mapping_id: i64) -> Vec<MatchCondition> {
        let mut conditions: Vec<_> = self
            .match_conditions
            .iter()
            .filter(|mc| mc.entity_mapping_id == entity_mapping_id)
            .cloned()
            .collect();
        conditions.sort_by_key(|mc| mc.order);
        conditions
    }
}

// =============================================================================
// Build context
// =============================================================================

/// Context for building the tree, holding lookup data and mutable type accumulator.
pub(super) struct TreeBuildContext<'a> {
    pub(super) lookup: TreeLookup<'a>,
    /// Entity mappings for looking up source entity names.
    pub(super) entity_mappings: &'a [EntityMapping],
    /// Cached field types per source entity (for type tracking).
    pub(super) field_type_cache: &'a FieldTypeCache,
    /// Cached field types per target entity (for target field type checking).
    pub(super) target_field_cache: &'a FieldTypeCache,
    /// Internal type accumulator, populated during tree building.
    pub(super) types: TypeAccumulator,
}

impl<'a> TreeBuildContext<'a> {
    /// Look up the source entity name for a given entity mapping ID.
    pub(super) fn source_entity_for(&self, entity_mapping_id: i64) -> &str {
        self.entity_mappings
            .iter()
            .find(|em| em.id == entity_mapping_id)
            .map(|em| em.source_entity.as_str())
            .unwrap_or("")
    }

    /// Look up the target entity name for a given entity mapping ID.
    fn target_entity_for(&self, entity_mapping_id: i64) -> &str {
        self.entity_mappings
            .iter()
            .find(|em| em.id == entity_mapping_id)
            .map(|em| em.target_entity.as_str())
            .unwrap_or("")
    }
}

// =============================================================================
// Top-level tree building
// =============================================================================

/// Build tree nodes from all migration data.
///
/// Type tracking is performed internally — type data is embedded directly
/// on `TransformNode` and `FieldMappingNode` tree nodes.
pub fn build_tree_nodes(
    phases: Vec<Phase>,
    entity_mappings: Vec<EntityMapping>,
    variables: Vec<Variable>,
    field_mappings: Vec<FieldMapping>,
    transforms: Vec<Transform>,
    match_branches: Vec<MatchBranch>,
    coalesce_chains: Vec<CoalesceChain>,
    find_conditions: Vec<FindCondition>,
    match_conditions: Vec<MatchCondition>,
    field_type_cache: &FieldTypeCache,
    target_field_cache: &FieldTypeCache,
) -> Vec<TreeNode<MigrationTreeNode>> {
    let mut ctx = TreeBuildContext {
        lookup: TreeLookup {
            transforms: &transforms,
            match_branches: &match_branches,
            coalesce_chains: &coalesce_chains,
            find_conditions: &find_conditions,
            match_conditions: &match_conditions,
        },
        entity_mappings: &entity_mappings,
        field_type_cache,
        target_field_cache,
        types: TypeAccumulator::default(),
    };

    let nodes = phases
        .into_iter()
        .map(|phase| {
            let phase_id = phase.id;
            let is_lua_phase = phase.mode == Mode::Lua;

            // Lua phases have no entity mapping children
            if is_lua_phase {
                return TreeNode::leaf(MigrationTreeNode::Phase(phase));
            }

            let children: Vec<TreeNode<MigrationTreeNode>> = entity_mappings
                .iter()
                .filter(|em| em.phase_id == phase_id)
                .cloned()
                .map(|em| build_entity_mapping_node(em, &variables, &field_mappings, &mut ctx))
                .collect();

            if children.is_empty() {
                TreeNode::leaf(MigrationTreeNode::Phase(phase))
            } else {
                TreeNode::branch(MigrationTreeNode::Phase(phase), children)
            }
        })
        .collect();

    nodes
}

// =============================================================================
// Entity mapping node
// =============================================================================

/// Build a tree node for an entity mapping with its child config nodes.
fn build_entity_mapping_node(
    em: EntityMapping,
    variables: &[Variable],
    field_mappings: &[FieldMapping],
    ctx: &mut TreeBuildContext,
) -> TreeNode<MigrationTreeNode> {
    let em_id = em.id;
    let is_lua = em.mode == Mode::Lua;

    let mut children = Vec::new();

    if is_lua {
        // Lua mode: only Test GUIDs
        children.push(TreeNode::leaf(MigrationTreeNode::TestGuids {
            entity_mapping_id: em_id,
        }));
    } else {
        // Declarative mode: all config nodes
        // MatchConfig is expandable when in Find mode (has match conditions)
        if em.match_strategy == MatchStrategy::Find {
            let mc_children: Vec<TreeNode<MigrationTreeNode>> = ctx
                .lookup
                .get_match_conditions(em_id)
                .into_iter()
                .map(|mc| build_match_condition_node(mc, ctx))
                .collect();
            if mc_children.is_empty() {
                children.push(TreeNode::leaf(MigrationTreeNode::MatchConfig {
                    entity_mapping_id: em_id,
                }));
            } else {
                children.push(TreeNode::branch(
                    MigrationTreeNode::MatchConfig {
                        entity_mapping_id: em_id,
                    },
                    mc_children,
                ));
            }
        } else {
            children.push(TreeNode::leaf(MigrationTreeNode::MatchConfig {
                entity_mapping_id: em_id,
            }));
        }
        children.push(TreeNode::leaf(MigrationTreeNode::SourceFilter {
            entity_mapping_id: em_id,
        }));
        children.push(TreeNode::leaf(MigrationTreeNode::TargetFilter {
            entity_mapping_id: em_id,
        }));
        children.push(TreeNode::leaf(MigrationTreeNode::UnmatchedHandling {
            entity_mapping_id: em_id,
        }));
        children.push(TreeNode::leaf(MigrationTreeNode::Passes {
            entity_mapping_id: em_id,
        }));
        children.push(TreeNode::leaf(MigrationTreeNode::TestGuids {
            entity_mapping_id: em_id,
        }));

        // Variables section with transforms
        // Variables are processed in order so that later variables can reference earlier ones.
        let mut em_variables: Vec<_> = variables
            .iter()
            .filter(|v| v.entity_mapping_id == em_id)
            .cloned()
            .collect();
        em_variables.sort_by_key(|v| v.order);

        let var_children: Vec<TreeNode<MigrationTreeNode>> = em_variables
            .into_iter()
            .map(|v| build_variable_node(v, ctx))
            .collect();

        if var_children.is_empty() {
            children.push(TreeNode::leaf(MigrationTreeNode::Variables {
                entity_mapping_id: em_id,
            }));
        } else {
            children.push(TreeNode::branch(
                MigrationTreeNode::Variables {
                    entity_mapping_id: em_id,
                },
                var_children,
            ));
        }

        // Field mappings section with transforms
        let fm_children: Vec<TreeNode<MigrationTreeNode>> = field_mappings
            .iter()
            .filter(|fm| fm.entity_mapping_id == em_id)
            .cloned()
            .map(|fm| build_field_mapping_node(fm, ctx))
            .collect();

        if fm_children.is_empty() {
            children.push(TreeNode::leaf(MigrationTreeNode::FieldMappings {
                entity_mapping_id: em_id,
            }));
        } else {
            children.push(TreeNode::branch(
                MigrationTreeNode::FieldMappings {
                    entity_mapping_id: em_id,
                },
                fm_children,
            ));
        }
    }

    TreeNode::branch(MigrationTreeNode::EntityMapping(em), children)
}

// =============================================================================
// Variable + field mapping nodes
// =============================================================================

/// Build a tree node for a variable with its transforms.
fn build_variable_node(v: Variable, ctx: &mut TreeBuildContext) -> TreeNode<MigrationTreeNode> {
    let transforms = ctx.lookup.get_transforms(ParentType::Variable, v.id);
    let var_name = v.name.clone();

    // Record variable declared type for use in later chains (copy($var))
    log::debug!(
        "type_tracking: variable ${} declared as {:?}",
        var_name,
        v.declared_type,
    );
    ctx.types
        .variable_types
        .insert(var_name, v.declared_type.clone());

    if transforms.is_empty() {
        let vn = VariableNode {
            variable: v,
            warning: None,
        };
        TreeNode::leaf(MigrationTreeNode::Variable(vn))
    } else {
        // Compute types for this chain
        let source_entity = ctx.source_entity_for(v.entity_mapping_id);
        let chain_result = compute_chain_types(&transforms, source_entity, ctx);
        ctx.types.merge(&chain_result);

        // Check chain output against declared type
        let warning = check_chain_output(&v.declared_type, &chain_result.output_type);
        if let Some(ref w) = warning {
            log::debug!(
                "type_tracking: variable ${} chain output {:?} incompatible with declared type {:?}",
                v.name,
                w.chain_output,
                w.target_type,
            );
        }

        let vn = VariableNode {
            variable: v,
            warning,
        };
        let transform_nodes: Vec<TreeNode<MigrationTreeNode>> = transforms
            .into_iter()
            .map(|t| build_transform_node(t, ctx))
            .collect();
        TreeNode::branch(MigrationTreeNode::Variable(vn), transform_nodes)
    }
}

/// Build a tree node for a field mapping with its transforms.
fn build_field_mapping_node(
    fm: FieldMapping,
    ctx: &mut TreeBuildContext,
) -> TreeNode<MigrationTreeNode> {
    let transforms = ctx.lookup.get_transforms(ParentType::FieldMapping, fm.id);
    let target_entity = ctx.target_entity_for(fm.entity_mapping_id).to_owned();

    // Resolve the target field type for display
    let target_type = resolve_target_field_type(&fm.target_field, &target_entity, ctx);

    if transforms.is_empty() {
        let fmn = FieldMappingNode {
            field_mapping: fm,
            target_type,
            warning: None,
        };
        TreeNode::leaf(MigrationTreeNode::FieldMapping(fmn))
    } else {
        // Compute types for this chain
        let source_entity = ctx.source_entity_for(fm.entity_mapping_id).to_owned();
        log::debug!(
            "type_tracking: computing chain for field mapping {} (target_field={}, source={}, target={})",
            fm.id,
            fm.target_field,
            source_entity,
            target_entity,
        );
        let chain_result = compute_chain_types(&transforms, &source_entity, ctx);
        ctx.types.merge(&chain_result);

        // Check chain output against target field type
        let warning = check_field_mapping_output(
            &fm.target_field,
            &target_entity,
            &chain_result.output_type,
            ctx,
        );
        // Warning is embedded directly on FieldMappingNode below

        let fmn = FieldMappingNode {
            field_mapping: fm,
            target_type,
            warning,
        };
        let transform_nodes: Vec<TreeNode<MigrationTreeNode>> = transforms
            .into_iter()
            .map(|t| build_transform_node(t, ctx))
            .collect();
        TreeNode::branch(MigrationTreeNode::FieldMapping(fmn), transform_nodes)
    }
}

// =============================================================================
// Type checking helpers
// =============================================================================

/// Look up the target field type from the target field cache.
fn resolve_target_field_type(
    target_field: &str,
    target_entity: &str,
    ctx: &TreeBuildContext,
) -> Option<ValueType> {
    let target_fields = ctx.target_field_cache.get(target_entity)?;
    let field_type = target_fields.get(target_field)?;
    Some(ValueType::Known(field_type.clone()))
}

/// Check if a chain output type is compatible with an expected type.
/// Returns a `ChainOutputWarning` if incompatible.
fn check_chain_output(
    expected_type: &ValueType,
    chain_output: &ValueType,
) -> Option<ChainOutputWarning> {
    // Skip check if chain output is Null (always compatible) or Any
    if matches!(chain_output, ValueType::Null | ValueType::Any) {
        return None;
    }

    if !chain_output.is_compatible_with(expected_type) {
        Some(ChainOutputWarning {
            chain_output: chain_output.clone(),
            target_type: expected_type.clone(),
        })
    } else {
        None
    }
}

/// Check if a chain output type is compatible with the target field type.
/// Returns a `ChainOutputWarning` if incompatible.
fn check_field_mapping_output(
    target_field: &str,
    target_entity: &str,
    chain_output: &ValueType,
    ctx: &TreeBuildContext,
) -> Option<ChainOutputWarning> {
    let target_fields = ctx.target_field_cache.get(target_entity)?;
    let target_field_type = target_fields.get(target_field)?;
    let target_type = ValueType::Known(target_field_type.clone());
    check_chain_output(&target_type, chain_output)
}

// =============================================================================
// Transform node building
// =============================================================================

/// Create a `MigrationTreeNode::Transform` with embedded type tracking data.
fn make_transform_node(t: Transform, ctx: &TreeBuildContext) -> MigrationTreeNode {
    let output_type = ctx.types.type_for(t.id).cloned();
    let warning = ctx.types.warning_for(t.id).cloned();
    log::debug!(
        "type_tracking: tree node for transform {} -> type={:?}, warning={}",
        t.id,
        output_type,
        warning.is_some(),
    );
    MigrationTreeNode::Transform(TransformNode {
        transform: t,
        output_type,
        warning,
    })
}

/// Build a tree node for a transform, including nested structures.
fn build_transform_node(t: Transform, ctx: &mut TreeBuildContext) -> TreeNode<MigrationTreeNode> {
    let source_entity = ctx.source_entity_for(t.entity_mapping_id).to_owned();
    match &t.data {
        TransformData::Guard { .. } => {
            // Guard: child transforms appear directly under the guard node
            let fallback_transforms = ctx.lookup.get_transforms(ParentType::GuardFallback, t.id);
            if fallback_transforms.is_empty() {
                TreeNode::leaf(make_transform_node(t, ctx))
            } else {
                let node = make_transform_node(t, ctx);
                let children: Vec<TreeNode<MigrationTreeNode>> = fallback_transforms
                    .into_iter()
                    .map(|ft| build_transform_node(ft, ctx))
                    .collect();
                TreeNode::branch(node, children)
            }
        }
        TransformData::Match { has_default } => {
            // Match: branches as children, plus optional default
            let has_default = *has_default;
            let branches = ctx.lookup.get_match_branches(t.id);
            let has_branches = !branches.is_empty() || has_default;

            if !has_branches {
                TreeNode::leaf(make_transform_node(t, ctx))
            } else {
                // Compute types for each branch, then union them
                let mut branch_output_types = Vec::new();
                for mb in &branches {
                    let branch_transforms =
                        ctx.lookup.get_transforms(ParentType::MatchBranch, mb.id);
                    if !branch_transforms.is_empty() {
                        let branch_result =
                            compute_chain_types(&branch_transforms, &source_entity, ctx);
                        branch_output_types.push(branch_result.output_type.clone());
                        ctx.types.merge(&branch_result);
                    }
                }

                // Also include default branch type if present
                if has_default {
                    let default_transforms =
                        ctx.lookup.get_transforms(ParentType::MatchDefault, t.id);
                    if !default_transforms.is_empty() {
                        let default_result =
                            compute_chain_types(&default_transforms, &source_entity, ctx);
                        branch_output_types.push(default_result.output_type.clone());
                        ctx.types.merge(&default_result);
                    }
                }

                // Store the union type for the match transform itself
                if !branch_output_types.is_empty() {
                    let union_type = resolve_branch_union(&branch_output_types);
                    ctx.types.transform_types.insert(t.id, union_type);
                }

                let transform_id = t.id;
                let node = make_transform_node(t, ctx);
                let mut children: Vec<TreeNode<MigrationTreeNode>> = branches
                    .into_iter()
                    .map(|mb| build_match_branch_node(mb, ctx))
                    .collect();

                // Add default branch node if has_default
                if has_default {
                    children.push(build_match_default_node(transform_id, ctx));
                }

                TreeNode::branch(node, children)
            }
        }
        TransformData::Coalesce => {
            // Coalesce: chains as children
            let chains = ctx.lookup.get_coalesce_chains(t.id);
            if chains.is_empty() {
                TreeNode::leaf(make_transform_node(t, ctx))
            } else {
                // Compute types for each fallback chain, then union them
                let mut chain_output_types = Vec::new();
                for cc in &chains {
                    let chain_transforms =
                        ctx.lookup.get_transforms(ParentType::CoalesceChain, cc.id);
                    if !chain_transforms.is_empty() {
                        let chain_result =
                            compute_chain_types(&chain_transforms, &source_entity, ctx);
                        chain_output_types.push(chain_result.output_type.clone());
                        ctx.types.merge(&chain_result);
                    }
                }
                // Store the union type for the coalesce transform itself
                if !chain_output_types.is_empty() {
                    let union_type = resolve_branch_union(&chain_output_types);
                    ctx.types.transform_types.insert(t.id, union_type);
                }

                let node = make_transform_node(t, ctx);
                let children: Vec<TreeNode<MigrationTreeNode>> = chains
                    .into_iter()
                    .map(|cc| build_coalesce_chain_node(cc, ctx))
                    .collect();
                TreeNode::branch(node, children)
            }
        }
        TransformData::Find { mode, fallback, .. } => {
            let has_default = matches!(fallback, FindFallback::Default);
            // Find in Where mode: conditions as children
            if matches!(mode, crate::apps::migration::types::FindMode::Where) {
                let conditions = ctx.lookup.get_find_conditions(t.id);
                let has_children = !conditions.is_empty() || has_default;

                if !has_children {
                    TreeNode::leaf(make_transform_node(t, ctx))
                } else {
                    // Compute types for each condition's chain
                    for fc in &conditions {
                        let cond_transforms =
                            ctx.lookup.get_transforms(ParentType::FindCondition, fc.id);
                        if !cond_transforms.is_empty() {
                            let cond_result =
                                compute_chain_types(&cond_transforms, &source_entity, ctx);
                            ctx.types.merge(&cond_result);
                        }
                    }

                    // Also compute types for default chain if present
                    if has_default {
                        let default_transforms =
                            ctx.lookup.get_transforms(ParentType::FindDefault, t.id);
                        if !default_transforms.is_empty() {
                            let default_result =
                                compute_chain_types(&default_transforms, &source_entity, ctx);
                            ctx.types.merge(&default_result);
                        }
                    }

                    let transform_id = t.id;
                    let node = make_transform_node(t, ctx);
                    let mut children: Vec<TreeNode<MigrationTreeNode>> = conditions
                        .into_iter()
                        .map(|fc| build_find_condition_node(fc, ctx))
                        .collect();

                    // Add default chain node if fallback is Default
                    if has_default {
                        children.push(build_find_default_node(transform_id, ctx));
                    }

                    TreeNode::branch(node, children)
                }
            } else {
                // Lua mode - only default chain if present
                if has_default {
                    let transform_id = t.id;
                    let node = make_transform_node(t, ctx);
                    let children = vec![build_find_default_node(transform_id, ctx)];
                    TreeNode::branch(node, children)
                } else {
                    TreeNode::leaf(make_transform_node(t, ctx))
                }
            }
        }
        _ => {
            // Simple transforms - no children
            TreeNode::leaf(make_transform_node(t, ctx))
        }
    }
}

// =============================================================================
// Nested chain node builders
// =============================================================================

/// Build a tree node for a match branch with its transforms.
fn build_match_branch_node(
    mb: MatchBranch,
    ctx: &mut TreeBuildContext,
) -> TreeNode<MigrationTreeNode> {
    let transforms = ctx.lookup.get_transforms(ParentType::MatchBranch, mb.id);
    build_nested_chain_node(
        MigrationTreeNode::MatchBranch(mb),
        ParentType::MatchBranch,
        transforms,
        ctx,
    )
}

/// Build a tree node for the match default branch.
/// Default branch transforms use ParentType::MatchDefault with parent_id = match transform id.
fn build_match_default_node(
    match_transform_id: i64,
    ctx: &mut TreeBuildContext,
) -> TreeNode<MigrationTreeNode> {
    let transforms = ctx
        .lookup
        .get_transforms(ParentType::MatchDefault, match_transform_id);
    let node = MigrationTreeNode::MatchDefault {
        transform_id: match_transform_id,
    };
    if transforms.is_empty() {
        TreeNode::leaf(node)
    } else {
        let children: Vec<TreeNode<MigrationTreeNode>> = transforms
            .into_iter()
            .map(|t| build_transform_node(t, ctx))
            .collect();
        TreeNode::branch(node, children)
    }
}

/// Build a tree node for a coalesce chain with its transforms.
fn build_coalesce_chain_node(
    cc: CoalesceChain,
    ctx: &mut TreeBuildContext,
) -> TreeNode<MigrationTreeNode> {
    let transforms = ctx.lookup.get_transforms(ParentType::CoalesceChain, cc.id);
    if transforms.is_empty() {
        TreeNode::leaf(MigrationTreeNode::CoalesceChain(cc))
    } else {
        // Always show transforms directly under the chain node (no Chain wrapper)
        let children: Vec<TreeNode<MigrationTreeNode>> = transforms
            .into_iter()
            .map(|t| build_transform_node(t, ctx))
            .collect();
        TreeNode::branch(MigrationTreeNode::CoalesceChain(cc), children)
    }
}

/// Build a tree node for a find condition with its transforms.
fn build_find_condition_node(
    fc: FindCondition,
    ctx: &mut TreeBuildContext,
) -> TreeNode<MigrationTreeNode> {
    let transforms = ctx.lookup.get_transforms(ParentType::FindCondition, fc.id);
    build_nested_chain_node(
        MigrationTreeNode::FindCondition(fc),
        ParentType::FindCondition,
        transforms,
        ctx,
    )
}

/// Build a tree node for the find default chain.
/// Default chain transforms use ParentType::FindDefault with parent_id = find transform id.
fn build_find_default_node(
    find_transform_id: i64,
    ctx: &mut TreeBuildContext,
) -> TreeNode<MigrationTreeNode> {
    let transforms = ctx
        .lookup
        .get_transforms(ParentType::FindDefault, find_transform_id);
    let node = MigrationTreeNode::FindDefault {
        transform_id: find_transform_id,
    };
    if transforms.is_empty() {
        TreeNode::leaf(node)
    } else {
        let children: Vec<TreeNode<MigrationTreeNode>> = transforms
            .into_iter()
            .map(|t| build_transform_node(t, ctx))
            .collect();
        TreeNode::branch(node, children)
    }
}

/// Build a tree node for a match condition with its transforms.
fn build_match_condition_node(
    mc: MatchCondition,
    ctx: &mut TreeBuildContext,
) -> TreeNode<MigrationTreeNode> {
    let transforms = ctx.lookup.get_transforms(ParentType::MatchCondition, mc.id);

    // Compute types for the condition's chain
    if !transforms.is_empty() {
        let source_entity = ctx.source_entity_for(mc.entity_mapping_id).to_string();
        let chain_result = compute_chain_types(&transforms, &source_entity, ctx);
        ctx.types.merge(&chain_result);
    }

    build_nested_chain_node(
        MigrationTreeNode::MatchCondition(mc),
        ParentType::MatchCondition,
        transforms,
        ctx,
    )
}

/// Build a nested chain node following the display rules:
/// - Single transform: show directly as child
/// - Multiple transforms: wrap in a Chain node
fn build_nested_chain_node(
    parent_node: MigrationTreeNode,
    parent_type: ParentType,
    transforms: Vec<Transform>,
    ctx: &mut TreeBuildContext,
) -> TreeNode<MigrationTreeNode> {
    let parent_id = match &parent_node {
        MigrationTreeNode::MatchBranch(mb) => mb.id,
        MigrationTreeNode::CoalesceChain(cc) => cc.id,
        MigrationTreeNode::FindCondition(fc) => fc.id,
        MigrationTreeNode::MatchCondition(mc) => mc.id,
        _ => return TreeNode::leaf(parent_node),
    };

    if transforms.is_empty() {
        TreeNode::leaf(parent_node)
    } else if transforms.len() == 1 {
        // Single transform: show directly under parent
        let transform_nodes: Vec<TreeNode<MigrationTreeNode>> = transforms
            .into_iter()
            .map(|t| build_transform_node(t, ctx))
            .collect();
        TreeNode::branch(parent_node, transform_nodes)
    } else {
        // Multiple transforms: wrap in a Chain node
        let transform_nodes: Vec<TreeNode<MigrationTreeNode>> = transforms
            .into_iter()
            .map(|t| build_transform_node(t, ctx))
            .collect();
        let chain_node = TreeNode::branch(
            MigrationTreeNode::Chain {
                parent_type,
                parent_id,
            },
            transform_nodes,
        );
        TreeNode::branch(parent_node, vec![chain_node])
    }
}
