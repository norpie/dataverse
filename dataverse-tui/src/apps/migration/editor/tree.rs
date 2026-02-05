//! Tree item implementation for the migration editor.

use rafter::element;
use rafter::widgets::Text;
use rafter::widgets::TreeItem;
use rafter::widgets::TreeNode;
use tuidom::Element;

use crate::apps::migration::types::CoalesceChain;
use crate::apps::migration::types::EntityMapping;
use crate::apps::migration::types::FieldMapping;
use crate::apps::migration::types::FindCondition;
use crate::apps::migration::types::MatchBranch;
use crate::apps::migration::types::Mode;
use crate::apps::migration::types::ParentType;
use crate::apps::migration::types::Phase;
use crate::apps::migration::types::Transform;
use crate::apps::migration::types::TransformData;
use crate::apps::migration::types::Variable;

/// A node in the migration editor tree.
#[derive(Clone, Debug)]
pub enum MigrationTreeNode {
    /// A phase (top-level node).
    Phase(Phase),
    /// An entity mapping (child of a phase).
    EntityMapping(EntityMapping),
    /// Match configuration (child of entity mapping, Declarative only).
    MatchConfig { entity_mapping_id: i64 },
    /// Source filter (child of entity mapping, Declarative only).
    SourceFilter { entity_mapping_id: i64 },
    /// Target filter (child of entity mapping, Declarative only).
    TargetFilter { entity_mapping_id: i64 },
    /// Unmatched handling settings (child of entity mapping, Declarative only).
    UnmatchedHandling { entity_mapping_id: i64 },
    /// Pass toggles (child of entity mapping, Declarative only).
    Passes { entity_mapping_id: i64 },
    /// Test GUIDs (child of entity mapping, both modes).
    TestGuids { entity_mapping_id: i64 },
    /// Variables section header (child of entity mapping, Declarative only).
    Variables { entity_mapping_id: i64 },
    /// An individual variable (child of Variables section).
    Variable(Variable),
    /// Field mappings section header (child of entity mapping, Declarative only).
    FieldMappings { entity_mapping_id: i64 },
    /// An individual field mapping (child of FieldMappings section).
    FieldMapping(FieldMapping),
    /// A transform operation (child of Variable, FieldMapping, or nested chain).
    Transform(Transform),
    /// A branch within a match transform.
    MatchBranch(MatchBranch),
    /// A fallback chain within a coalesce transform.
    CoalesceChain(CoalesceChain),
    /// A condition within a find transform (where-clause mode).
    FindCondition(FindCondition),
    /// A wrapper for multi-transform nested chains.
    /// Used when a nested chain (guard fallback, coalesce chain, match branch, find condition)
    /// has more than one transform.
    Chain {
        parent_type: ParentType,
        parent_id: i64,
    },
}

impl MigrationTreeNode {
    /// Get the entity mapping ID if this is a child node of an entity mapping.
    pub fn entity_mapping_id(&self) -> Option<i64> {
        match self {
            Self::Phase(_) => None,
            Self::EntityMapping(em) => Some(em.id),
            Self::MatchConfig { entity_mapping_id }
            | Self::SourceFilter { entity_mapping_id }
            | Self::TargetFilter { entity_mapping_id }
            | Self::UnmatchedHandling { entity_mapping_id }
            | Self::Passes { entity_mapping_id }
            | Self::TestGuids { entity_mapping_id }
            | Self::Variables { entity_mapping_id }
            | Self::FieldMappings { entity_mapping_id } => Some(*entity_mapping_id),
            Self::Variable(v) => Some(v.entity_mapping_id),
            Self::FieldMapping(fm) => Some(fm.entity_mapping_id),
            Self::Transform(t) => Some(t.entity_mapping_id),
            Self::MatchBranch(_) => None,   // Get via transform
            Self::CoalesceChain(_) => None, // Get via transform
            Self::FindCondition(_) => None, // Get via transform
            Self::Chain { .. } => None,     // Get via parent
        }
    }

    /// Check if this is a phase node.
    pub fn is_phase(&self) -> bool {
        matches!(self, Self::Phase(_))
    }

    /// Check if this is an entity mapping node.
    pub fn is_entity_mapping(&self) -> bool {
        matches!(self, Self::EntityMapping(_))
    }

    /// Check if this is a child config node (not phase or entity mapping).
    pub fn is_config_node(&self) -> bool {
        matches!(
            self,
            Self::MatchConfig { .. }
                | Self::SourceFilter { .. }
                | Self::TargetFilter { .. }
                | Self::UnmatchedHandling { .. }
                | Self::Passes { .. }
                | Self::TestGuids { .. }
                | Self::Variables { .. }
                | Self::Variable(_)
                | Self::FieldMappings { .. }
                | Self::FieldMapping(_)
                | Self::Transform(_)
                | Self::MatchBranch(_)
                | Self::CoalesceChain(_)
                | Self::FindCondition(_)
                | Self::Chain { .. }
        )
    }

    /// Check if this is a transform node.
    pub fn is_transform(&self) -> bool {
        matches!(self, Self::Transform(_))
    }

    /// Get the transform if this is a transform node.
    pub fn as_transform(&self) -> Option<&Transform> {
        match self {
            Self::Transform(t) => Some(t),
            _ => None,
        }
    }

    /// Get the variable if this is a variable node.
    pub fn as_variable(&self) -> Option<&Variable> {
        match self {
            Self::Variable(v) => Some(v),
            _ => None,
        }
    }

    /// Get the field mapping if this is a field mapping node.
    pub fn as_field_mapping(&self) -> Option<&FieldMapping> {
        match self {
            Self::FieldMapping(fm) => Some(fm),
            _ => None,
        }
    }

    /// Get the match branch if this is a match branch node.
    pub fn as_match_branch(&self) -> Option<&MatchBranch> {
        match self {
            Self::MatchBranch(mb) => Some(mb),
            _ => None,
        }
    }

    /// Get the coalesce chain if this is a coalesce chain node.
    pub fn as_coalesce_chain(&self) -> Option<&CoalesceChain> {
        match self {
            Self::CoalesceChain(cc) => Some(cc),
            _ => None,
        }
    }

    /// Get the find condition if this is a find condition node.
    pub fn as_find_condition(&self) -> Option<&FindCondition> {
        match self {
            Self::FindCondition(fc) => Some(fc),
            _ => None,
        }
    }

    /// Get the phase if this is a phase node.
    pub fn as_phase(&self) -> Option<&Phase> {
        match self {
            Self::Phase(p) => Some(p),
            _ => None,
        }
    }

    /// Get the entity mapping if this is an entity mapping node.
    pub fn as_entity_mapping(&self) -> Option<&EntityMapping> {
        match self {
            Self::EntityMapping(em) => Some(em),
            _ => None,
        }
    }
}

impl TreeItem for MigrationTreeNode {
    type Key = String;

    fn key(&self) -> String {
        match self {
            Self::Phase(p) => format!("phase-{}", p.id),
            Self::EntityMapping(em) => format!("entity-{}", em.id),
            Self::MatchConfig { entity_mapping_id } => {
                format!("match-config-{}", entity_mapping_id)
            }
            Self::SourceFilter { entity_mapping_id } => {
                format!("source-filter-{}", entity_mapping_id)
            }
            Self::TargetFilter { entity_mapping_id } => {
                format!("target-filter-{}", entity_mapping_id)
            }
            Self::UnmatchedHandling { entity_mapping_id } => {
                format!("unmatched-{}", entity_mapping_id)
            }
            Self::Passes { entity_mapping_id } => format!("passes-{}", entity_mapping_id),
            Self::TestGuids { entity_mapping_id } => format!("test-guids-{}", entity_mapping_id),
            Self::Variables { entity_mapping_id } => format!("variables-{}", entity_mapping_id),
            Self::Variable(v) => format!("variable-{}", v.id),
            Self::FieldMappings { entity_mapping_id } => {
                format!("field-mappings-{}", entity_mapping_id)
            }
            Self::FieldMapping(fm) => format!("field-mapping-{}", fm.id),
            Self::Transform(t) => format!("transform-{}", t.id),
            Self::MatchBranch(mb) => format!("match-branch-{}", mb.id),
            Self::CoalesceChain(cc) => format!("coalesce-chain-{}", cc.id),
            Self::FindCondition(fc) => format!("find-condition-{}", fc.id),
            Self::Chain {
                parent_type,
                parent_id,
            } => format!("chain-{}-{}", parent_type.as_str(), parent_id),
        }
    }

    fn render(&self) -> Element {
        match self {
            Self::Phase(phase) => {
                let mode_indicator = match phase.mode {
                    Mode::Declarative => "",
                    Mode::Lua => " [lua]",
                };
                let label = format!("{}{}", phase.name, mode_indicator);

                element! {
                    text (content: {label})
                }
            }
            Self::EntityMapping(em) => {
                let mode_indicator = match em.mode {
                    Mode::Declarative => "",
                    Mode::Lua => " [lua]",
                };
                let label = format!(
                    "{} ({} → {}){}",
                    em.name, em.source_entity, em.target_entity, mode_indicator
                );

                element! {
                    text (content: {label}) style (fg: muted)
                }
            }
            Self::MatchConfig { .. } => element! {
                text (content: "Match Config") style (fg: muted)
            },
            Self::SourceFilter { .. } => element! {
                text (content: "Source Filter") style (fg: muted)
            },
            Self::TargetFilter { .. } => element! {
                text (content: "Target Filter") style (fg: muted)
            },
            Self::UnmatchedHandling { .. } => element! {
                text (content: "Unmatched Handling") style (fg: muted)
            },
            Self::Passes { .. } => element! {
                text (content: "Passes") style (fg: muted)
            },
            Self::TestGuids { .. } => element! {
                text (content: "Test GUIDs") style (fg: muted)
            },
            Self::Variables { .. } => element! {
                text (content: "Variables") style (fg: muted)
            },
            Self::Variable(v) => {
                let label = format!("${}", v.name);
                element! {
                    text (content: {label}) style (fg: primary)
                }
            }
            Self::FieldMappings { .. } => element! {
                text (content: "Field Mappings") style (fg: muted)
            },
            Self::FieldMapping(fm) => {
                let label = fm.target_field.clone();
                element! {
                    text (content: {label}) style (fg: primary)
                }
            }
            Self::Transform(t) => {
                let label = transform_display_text(&t.data);
                element! {
                    text (content: {label}) style (fg: primary)
                }
            }
            Self::MatchBranch(mb) => {
                let label = if mb.is_default {
                    "Default".to_string()
                } else {
                    // TODO: Show condition summary when condition display is implemented
                    format!("Branch {}", mb.order + 1)
                };
                element! {
                    text (content: {label}) style (fg: muted)
                }
            }
            Self::CoalesceChain(cc) => {
                let label = format!("Fallback {}", cc.order + 1);
                element! {
                    text (content: {label}) style (fg: muted)
                }
            }
            Self::FindCondition(fc) => {
                let label = format!("Condition: {}", fc.target_field);
                element! {
                    text (content: {label}) style (fg: muted)
                }
            }
            Self::Chain { .. } => element! {
                text (content: "Chain") style (fg: muted)
            },
        }
    }
}

/// Generate display text for a transform.
/// Format: `type (summary)` where summary is type-specific.
pub fn transform_display_text(data: &TransformData) -> String {
    match data {
        TransformData::Copy { path } => {
            format!("copy ({})", path)
        }
        TransformData::Constant { value } => {
            let summary = match value {
                dataverse_lib::model::Value::Null => "null".to_string(),
                dataverse_lib::model::Value::Bool(b) => b.to_string(),
                dataverse_lib::model::Value::Int(i) => i.to_string(),
                dataverse_lib::model::Value::Float(f) => f.to_string(),
                dataverse_lib::model::Value::Decimal(d) => d.to_string(),
                dataverse_lib::model::Value::String(s) => {
                    if s.len() > 20 {
                        format!("\"{}...\"", &s[..17])
                    } else {
                        format!("\"{}\"", s)
                    }
                }
                dataverse_lib::model::Value::DateTime(_) => "[datetime]".to_string(),
                dataverse_lib::model::Value::Guid(_) => "[guid]".to_string(),
                _ => "[value]".to_string(),
            };
            format!("constant ({})", summary)
        }
        TransformData::Guid => "guid".to_string(),
        TransformData::Format { template } => {
            let summary = if template.len() > 20 {
                format!("\"{}...\"", &template[..17])
            } else {
                format!("\"{}\"", template)
            };
            format!("format ({})", summary)
        }
        TransformData::Replace { from, to, regex } => {
            let prefix = if *regex { "r" } else { "" };
            format!("replace ({}\"{}\" → \"{}\")", prefix, from, to)
        }
        TransformData::StringOps { ops } => {
            let op_names: Vec<&str> = ops
                .iter()
                .map(|op| match op {
                    crate::apps::migration::types::StringOp::Uppercase => "uppercase",
                    crate::apps::migration::types::StringOp::Lowercase => "lowercase",
                    crate::apps::migration::types::StringOp::Trim => "trim",
                    crate::apps::migration::types::StringOp::TrimStart => "trim_start",
                    crate::apps::migration::types::StringOp::TrimEnd => "trim_end",
                })
                .collect();
            format!("string_ops ({})", op_names.join(", "))
        }
        TransformData::Convert { target_type } => {
            format!("convert ({})", target_type)
        }
        TransformData::ParseInt => "parse_int".to_string(),
        TransformData::ParseDecimal => "parse_decimal".to_string(),
        TransformData::ParseDate { format } => {
            format!("parse_date (\"{}\")", format)
        }
        TransformData::ValueMap { mappings } => {
            format!("value_map ({} mappings)", mappings.len())
        }
        TransformData::Math { operation } => {
            let op_str = match operation {
                crate::apps::migration::types::MathOp::Add(n) => format!("add {}", n),
                crate::apps::migration::types::MathOp::Subtract(n) => format!("subtract {}", n),
                crate::apps::migration::types::MathOp::Multiply(n) => format!("multiply {}", n),
                crate::apps::migration::types::MathOp::Divide(n) => format!("divide {}", n),
                crate::apps::migration::types::MathOp::Round(places) => {
                    format!("round {}", places)
                }
            };
            format!("math ({})", op_str)
        }
        TransformData::Guard { condition: _ } => {
            // TODO: Show condition summary when condition display is implemented
            "guard (...)".to_string()
        }
        TransformData::Match => "match".to_string(),
        TransformData::Coalesce => "coalesce".to_string(),
        TransformData::Find {
            entity,
            fallback: _,
            mode: _,
        } => {
            format!("find ({})", entity)
        }
    }
}

/// Context for building the tree, holding all lookup data.
struct TreeBuildContext<'a> {
    transforms: &'a [Transform],
    match_branches: &'a [MatchBranch],
    coalesce_chains: &'a [CoalesceChain],
    find_conditions: &'a [FindCondition],
}

impl<'a> TreeBuildContext<'a> {
    /// Get transforms for a specific parent, sorted by order.
    fn get_transforms(&self, parent_type: ParentType, parent_id: i64) -> Vec<&Transform> {
        let mut transforms: Vec<_> = self
            .transforms
            .iter()
            .filter(|t| t.parent_type == parent_type && t.parent_id == parent_id)
            .collect();
        transforms.sort_by_key(|t| t.order);
        transforms
    }

    /// Get match branches for a transform, sorted by order.
    fn get_match_branches(&self, transform_id: i64) -> Vec<&MatchBranch> {
        let mut branches: Vec<_> = self
            .match_branches
            .iter()
            .filter(|mb| mb.transform_id == transform_id)
            .collect();
        branches.sort_by_key(|mb| mb.order);
        branches
    }

    /// Get coalesce chains for a transform, sorted by order.
    fn get_coalesce_chains(&self, transform_id: i64) -> Vec<&CoalesceChain> {
        let mut chains: Vec<_> = self
            .coalesce_chains
            .iter()
            .filter(|cc| cc.transform_id == transform_id)
            .collect();
        chains.sort_by_key(|cc| cc.order);
        chains
    }

    /// Get find conditions for a transform, sorted by order.
    fn get_find_conditions(&self, transform_id: i64) -> Vec<&FindCondition> {
        let mut conditions: Vec<_> = self
            .find_conditions
            .iter()
            .filter(|fc| fc.transform_id == transform_id)
            .collect();
        conditions.sort_by_key(|fc| fc.order);
        conditions
    }
}

/// Build tree nodes from all migration data.
pub fn build_tree_nodes(
    phases: Vec<Phase>,
    entity_mappings: Vec<EntityMapping>,
    variables: Vec<Variable>,
    field_mappings: Vec<FieldMapping>,
    transforms: Vec<Transform>,
    match_branches: Vec<MatchBranch>,
    coalesce_chains: Vec<CoalesceChain>,
    find_conditions: Vec<FindCondition>,
) -> Vec<TreeNode<MigrationTreeNode>> {
    let ctx = TreeBuildContext {
        transforms: &transforms,
        match_branches: &match_branches,
        coalesce_chains: &coalesce_chains,
        find_conditions: &find_conditions,
    };

    phases
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
                .map(|em| build_entity_mapping_node(em, &variables, &field_mappings, &ctx))
                .collect();

            if children.is_empty() {
                TreeNode::leaf(MigrationTreeNode::Phase(phase))
            } else {
                TreeNode::branch(MigrationTreeNode::Phase(phase), children)
            }
        })
        .collect()
}

/// Build a tree node for an entity mapping with its child config nodes.
fn build_entity_mapping_node(
    em: EntityMapping,
    variables: &[Variable],
    field_mappings: &[FieldMapping],
    ctx: &TreeBuildContext,
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
        children.push(TreeNode::leaf(MigrationTreeNode::MatchConfig {
            entity_mapping_id: em_id,
        }));
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
        let var_children: Vec<TreeNode<MigrationTreeNode>> = variables
            .iter()
            .filter(|v| v.entity_mapping_id == em_id)
            .cloned()
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

/// Build a tree node for a variable with its transforms.
fn build_variable_node(v: Variable, ctx: &TreeBuildContext) -> TreeNode<MigrationTreeNode> {
    let transforms = ctx.get_transforms(ParentType::Variable, v.id);

    if transforms.is_empty() {
        TreeNode::leaf(MigrationTreeNode::Variable(v))
    } else {
        let transform_nodes: Vec<TreeNode<MigrationTreeNode>> = transforms
            .into_iter()
            .map(|t| build_transform_node(t.clone(), ctx))
            .collect();
        TreeNode::branch(MigrationTreeNode::Variable(v), transform_nodes)
    }
}

/// Build a tree node for a field mapping with its transforms.
fn build_field_mapping_node(
    fm: FieldMapping,
    ctx: &TreeBuildContext,
) -> TreeNode<MigrationTreeNode> {
    let transforms = ctx.get_transforms(ParentType::FieldMapping, fm.id);

    if transforms.is_empty() {
        TreeNode::leaf(MigrationTreeNode::FieldMapping(fm))
    } else {
        let transform_nodes: Vec<TreeNode<MigrationTreeNode>> = transforms
            .into_iter()
            .map(|t| build_transform_node(t.clone(), ctx))
            .collect();
        TreeNode::branch(MigrationTreeNode::FieldMapping(fm), transform_nodes)
    }
}

/// Build a tree node for a transform, including nested structures.
fn build_transform_node(t: Transform, ctx: &TreeBuildContext) -> TreeNode<MigrationTreeNode> {
    match &t.data {
        TransformData::Guard { .. } => {
            // Guard: child transforms appear directly under the guard node
            let fallback_transforms = ctx.get_transforms(ParentType::GuardFallback, t.id);
            if fallback_transforms.is_empty() {
                TreeNode::leaf(MigrationTreeNode::Transform(t))
            } else {
                let children: Vec<TreeNode<MigrationTreeNode>> = fallback_transforms
                    .into_iter()
                    .map(|ft| build_transform_node(ft.clone(), ctx))
                    .collect();
                TreeNode::branch(MigrationTreeNode::Transform(t), children)
            }
        }
        TransformData::Match => {
            // Match: branches as children
            let branches = ctx.get_match_branches(t.id);
            if branches.is_empty() {
                TreeNode::leaf(MigrationTreeNode::Transform(t))
            } else {
                let children: Vec<TreeNode<MigrationTreeNode>> = branches
                    .into_iter()
                    .map(|mb| build_match_branch_node(mb.clone(), ctx))
                    .collect();
                TreeNode::branch(MigrationTreeNode::Transform(t), children)
            }
        }
        TransformData::Coalesce => {
            // Coalesce: chains as children
            let chains = ctx.get_coalesce_chains(t.id);
            if chains.is_empty() {
                TreeNode::leaf(MigrationTreeNode::Transform(t))
            } else {
                let children: Vec<TreeNode<MigrationTreeNode>> = chains
                    .into_iter()
                    .map(|cc| build_coalesce_chain_node(cc.clone(), ctx))
                    .collect();
                TreeNode::branch(MigrationTreeNode::Transform(t), children)
            }
        }
        TransformData::Find { mode, .. } => {
            // Find in Where mode: conditions as children
            if matches!(mode, crate::apps::migration::types::FindMode::Where) {
                let conditions = ctx.get_find_conditions(t.id);
                if conditions.is_empty() {
                    TreeNode::leaf(MigrationTreeNode::Transform(t))
                } else {
                    let children: Vec<TreeNode<MigrationTreeNode>> = conditions
                        .into_iter()
                        .map(|fc| build_find_condition_node(fc.clone(), ctx))
                        .collect();
                    TreeNode::branch(MigrationTreeNode::Transform(t), children)
                }
            } else {
                // Lua mode - no children
                TreeNode::leaf(MigrationTreeNode::Transform(t))
            }
        }
        _ => {
            // Simple transforms - no children
            TreeNode::leaf(MigrationTreeNode::Transform(t))
        }
    }
}

/// Build a tree node for a match branch with its transforms.
fn build_match_branch_node(mb: MatchBranch, ctx: &TreeBuildContext) -> TreeNode<MigrationTreeNode> {
    let transforms = ctx.get_transforms(ParentType::MatchBranch, mb.id);
    build_nested_chain_node(
        MigrationTreeNode::MatchBranch(mb),
        ParentType::MatchBranch,
        transforms,
        ctx,
    )
}

/// Build a tree node for a coalesce chain with its transforms.
fn build_coalesce_chain_node(
    cc: CoalesceChain,
    ctx: &TreeBuildContext,
) -> TreeNode<MigrationTreeNode> {
    let transforms = ctx.get_transforms(ParentType::CoalesceChain, cc.id);
    build_nested_chain_node(
        MigrationTreeNode::CoalesceChain(cc),
        ParentType::CoalesceChain,
        transforms,
        ctx,
    )
}

/// Build a tree node for a find condition with its transforms.
fn build_find_condition_node(
    fc: FindCondition,
    ctx: &TreeBuildContext,
) -> TreeNode<MigrationTreeNode> {
    let transforms = ctx.get_transforms(ParentType::FindCondition, fc.id);
    build_nested_chain_node(
        MigrationTreeNode::FindCondition(fc),
        ParentType::FindCondition,
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
    transforms: Vec<&Transform>,
    ctx: &TreeBuildContext,
) -> TreeNode<MigrationTreeNode> {
    let parent_id = match &parent_node {
        MigrationTreeNode::MatchBranch(mb) => mb.id,
        MigrationTreeNode::CoalesceChain(cc) => cc.id,
        MigrationTreeNode::FindCondition(fc) => fc.id,
        _ => return TreeNode::leaf(parent_node),
    };

    if transforms.is_empty() {
        TreeNode::leaf(parent_node)
    } else if transforms.len() == 1 {
        // Single transform: show directly under parent
        let transform_nodes: Vec<TreeNode<MigrationTreeNode>> = transforms
            .into_iter()
            .map(|t| build_transform_node(t.clone(), ctx))
            .collect();
        TreeNode::branch(parent_node, transform_nodes)
    } else {
        // Multiple transforms: wrap in a Chain node
        let transform_nodes: Vec<TreeNode<MigrationTreeNode>> = transforms
            .into_iter()
            .map(|t| build_transform_node(t.clone(), ctx))
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
