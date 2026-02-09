//! Tree item implementation for the migration editor.

use std::collections::HashMap;

use dataverse_lib::model::metadata::AttributeType;
use dataverse_lib::model::FieldType;
use dataverse_lib::model::ValueType;
use rafter::element;
use rafter::widgets::Text;
use rafter::widgets::TreeItem;
use rafter::widgets::TreeNode;
use tuidom::Element;

use crate::apps::migration::types::propagate_chain_types;
use crate::apps::migration::types::resolve_branch_union;
use crate::apps::migration::types::ChainOutputWarning;
use crate::apps::migration::types::ChainTypeResult;
use crate::apps::migration::types::CoalesceChain;
use crate::apps::migration::types::CompareOp;
use crate::apps::migration::types::Condition;
use crate::apps::migration::types::EntityMapping;
use crate::apps::migration::types::Expr;
use crate::apps::migration::types::FieldMapping;
use crate::apps::migration::types::FindCondition;
use crate::apps::migration::types::FindFallback;
use crate::apps::migration::types::MatchBranch;
use crate::apps::migration::types::Mode;
use crate::apps::migration::types::ParentType;
use crate::apps::migration::types::Phase;
use crate::apps::migration::types::SystemVar;
use crate::apps::migration::types::Transform;
use crate::apps::migration::types::TransformData;
use crate::apps::migration::types::TypeWarning;
use crate::apps::migration::types::Variable;
use crate::apps::migration::validation::parse_path;
use crate::apps::migration::validation::FieldPath;
use crate::apps::migration::validation::PathExpr;

/// Cache of field types per source entity.
/// Maps `source_entity_logical_name -> (field_logical_name -> FieldType)`.
pub type FieldTypeCache = HashMap<String, HashMap<String, FieldType>>;

/// A transform node in the tree, enriched with type tracking data.
#[derive(Clone, Debug)]
pub struct TransformNode {
    /// The underlying transform data.
    pub transform: Transform,
    /// The resolved output type of this transform (from type propagation).
    pub output_type: Option<ValueType>,
    /// Type warning if this transform has an input type mismatch.
    pub warning: Option<TypeWarning>,
}

/// A field mapping node in the tree, enriched with target type checking data.
#[derive(Clone, Debug)]
pub struct FieldMappingNode {
    /// The underlying field mapping data.
    pub field_mapping: FieldMapping,
    /// The target field's type (for display in the tree).
    pub target_type: Option<ValueType>,
    /// Type warning if the chain output doesn't match the target field type.
    pub warning: Option<ChainOutputWarning>,
}

/// A variable node in the tree, enriched with type checking data.
#[derive(Clone, Debug)]
pub struct VariableNode {
    /// The underlying variable data.
    pub variable: Variable,
    /// Type warning if the chain output doesn't match the declared type.
    pub warning: Option<ChainOutputWarning>,
}

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
    Variable(VariableNode),
    /// Field mappings section header (child of entity mapping, Declarative only).
    FieldMappings { entity_mapping_id: i64 },
    /// An individual field mapping (child of FieldMappings section).
    FieldMapping(FieldMappingNode),
    /// A transform operation (child of Variable, FieldMapping, or nested chain).
    Transform(TransformNode),
    /// A branch within a match transform.
    MatchBranch(MatchBranch),
    /// The default branch of a match transform (transforms use ParentType::MatchDefault).
    MatchDefault { transform_id: i64 },
    /// A fallback chain within a coalesce transform.
    CoalesceChain(CoalesceChain),
    /// A condition within a find transform (where-clause mode).
    FindCondition(FindCondition),
    /// The default chain of a find transform (transforms use ParentType::FindDefault).
    FindDefault { transform_id: i64 },
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
            Self::Variable(vn) => Some(vn.variable.entity_mapping_id),
            Self::FieldMapping(fmn) => Some(fmn.field_mapping.entity_mapping_id),
            Self::Transform(tn) => Some(tn.transform.entity_mapping_id),
            Self::MatchBranch(_) => None,      // Get via transform
            Self::MatchDefault { .. } => None, // Get via transform
            Self::CoalesceChain(_) => None,    // Get via transform
            Self::FindCondition(_) => None,    // Get via transform
            Self::FindDefault { .. } => None,  // Get via transform
            Self::Chain { .. } => None,        // Get via parent
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
                | Self::FindDefault { .. }
                | Self::Chain { .. }
        )
    }

    /// Check if this is a transform node.
    pub fn is_transform(&self) -> bool {
        matches!(self, Self::Transform(_))
    }

    /// Get the transform node if this is a transform node.
    pub fn as_transform_node(&self) -> Option<&TransformNode> {
        match self {
            Self::Transform(tn) => Some(tn),
            _ => None,
        }
    }

    /// Get the transform if this is a transform node.
    pub fn as_transform(&self) -> Option<&Transform> {
        match self {
            Self::Transform(tn) => Some(&tn.transform),
            _ => None,
        }
    }

    /// Get the variable node if this is a variable node.
    pub fn as_variable_node(&self) -> Option<&VariableNode> {
        match self {
            Self::Variable(vn) => Some(vn),
            _ => None,
        }
    }

    /// Get the variable if this is a variable node.
    pub fn as_variable(&self) -> Option<&Variable> {
        match self {
            Self::Variable(vn) => Some(&vn.variable),
            _ => None,
        }
    }

    /// Get the field mapping node if this is a field mapping node.
    pub fn as_field_mapping_node(&self) -> Option<&FieldMappingNode> {
        match self {
            Self::FieldMapping(fmn) => Some(fmn),
            _ => None,
        }
    }

    /// Get the field mapping if this is a field mapping node.
    pub fn as_field_mapping(&self) -> Option<&FieldMapping> {
        match self {
            Self::FieldMapping(fmn) => Some(&fmn.field_mapping),
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
            Self::Variable(vn) => format!("variable-{}", vn.variable.id),
            Self::FieldMappings { entity_mapping_id } => {
                format!("field-mappings-{}", entity_mapping_id)
            }
            Self::FieldMapping(fmn) => format!("field-mapping-{}", fmn.field_mapping.id),
            Self::Transform(tn) => format!("transform-{}", tn.transform.id),
            Self::MatchBranch(mb) => format!("match-branch-{}", mb.id),
            Self::MatchDefault { transform_id } => format!("match-default-{}", transform_id),
            Self::CoalesceChain(cc) => format!("coalesce-chain-{}", cc.id),
            Self::FindCondition(fc) => format!("find-condition-{}", fc.id),
            Self::FindDefault { transform_id } => format!("find-default-{}", transform_id),
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
            Self::Variable(vn) => {
                let label = format!("${}", vn.variable.name);
                let type_label = format!(" ({})", vn.variable.declared_type.display());
                let has_warning = vn.warning.is_some();
                element! {
                    row {
                        text (content: {label}) style (fg: primary)
                        text (content: {type_label}) style (fg: muted)
                        if has_warning {
                            text (content: " !") style (fg: warning)
                        }
                    }
                }
            }
            Self::FieldMappings { .. } => element! {
                text (content: "Field Mappings") style (fg: muted)
            },
            Self::FieldMapping(fmn) => {
                let label = fmn.field_mapping.target_field.clone();
                let has_warning = fmn.warning.is_some();
                let type_label = fmn
                    .target_type
                    .as_ref()
                    .map(|t| format!(" ({})", t.display()))
                    .unwrap_or_default();
                let has_type = fmn.target_type.is_some();
                element! {
                    row {
                        text (content: {label}) style (fg: primary)
                        if has_type {
                            text (content: {type_label}) style (fg: muted)
                        }
                        if has_warning {
                            text (content: " !") style (fg: warning)
                        }
                    }
                }
            }
            Self::Transform(tn) => {
                let label = transform_display_text(&tn.transform.data);
                let has_type = tn.output_type.is_some();
                let is_null = matches!(&tn.output_type, Some(ValueType::Null));
                let type_label = tn
                    .output_type
                    .as_ref()
                    .map(|t| format!(" -> {}", t.display()))
                    .unwrap_or_default();
                let has_warning = tn.warning.is_some();

                element! {
                    row {
                        text (content: {label}) style (fg: primary)
                        if has_type {
                            text (content: {type_label}) style (fg: muted)
                        }
                        if is_null {
                            text (content: " !") style (fg: error)
                        }
                        if has_warning {
                            text (content: " !") style (fg: warning)
                        }
                    }
                }
            }
            Self::MatchBranch(mb) => {
                let label = format!("Branch: {}", condition_summary(&mb.condition));
                element! {
                    text (content: {label}) style (fg: primary)
                }
            }
            Self::MatchDefault { .. } => element! {
                text (content: "Default") style (fg: primary)
            },
            Self::FindDefault { .. } => element! {
                text (content: "Default") style (fg: primary)
            },
            Self::CoalesceChain(cc) => {
                let label = format!("Fallback {}", cc.order + 1);
                element! {
                    text (content: {label}) style (fg: primary)
                }
            }
            Self::FindCondition(fc) => {
                let label = format!("Condition: {}", fc.target_field);
                element! {
                    text (content: {label}) style (fg: primary)
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
        TransformData::StringOps { op } => {
            let op_name = match op {
                crate::apps::migration::types::StringOp::Uppercase => "uppercase",
                crate::apps::migration::types::StringOp::Lowercase => "lowercase",
                crate::apps::migration::types::StringOp::Trim => "trim",
                crate::apps::migration::types::StringOp::TrimStart => "trim_start",
                crate::apps::migration::types::StringOp::TrimEnd => "trim_end",
            };
            format!("string_ops ({})", op_name)
        }
        TransformData::Convert { target_type } => {
            format!("convert ({})", target_type)
        }
        TransformData::ParseInt => "parse_int".to_string(),
        TransformData::ParseDecimal => "parse_decimal".to_string(),
        TransformData::ParseDate { format } => {
            format!("parse_date (\"{}\")", format)
        }
        TransformData::ValueMap { mappings, .. } => {
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
        TransformData::Guard { condition } => {
            format!("guard ({})", condition_summary(condition))
        }
        TransformData::Match { .. } => "match".to_string(),
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

/// Produce a short summary of a condition for display in tree nodes.
pub fn condition_summary(condition: &Condition) -> String {
    match condition {
        Condition::IsNull(expr) => format!("{} is null", expr_short(expr)),
        Condition::IsNotNull(expr) => format!("{} is not null", expr_short(expr)),
        Condition::Compare { left, op, right } => {
            let op_str = match op {
                CompareOp::Equal => "==",
                CompareOp::NotEqual => "!=",
                CompareOp::LessThan => "<",
                CompareOp::LessThanOrEqual => "<=",
                CompareOp::GreaterThan => ">",
                CompareOp::GreaterThanOrEqual => ">=",
            };
            format!("{} {} {}", expr_short(left), op_str, expr_short(right))
        }
        Condition::Contains { value, substring } => {
            format!("{} contains {}", expr_short(value), expr_short(substring))
        }
        Condition::StartsWith { value, prefix } => {
            format!("{} starts with {}", expr_short(value), expr_short(prefix))
        }
        Condition::EndsWith { value, suffix } => {
            format!("{} ends with {}", expr_short(value), expr_short(suffix))
        }
        Condition::And(conditions) => {
            format!("({} conditions)", conditions.len())
        }
        Condition::Or(conditions) => {
            format!("({} conditions)", conditions.len())
        }
        Condition::Not(inner) => {
            format!("not ({})", condition_summary(inner))
        }
    }
}

/// Short display of an expression.
fn expr_short(expr: &Expr) -> String {
    match expr {
        Expr::Path(p) => p.clone(),
        Expr::Variable(v) => format!("${}", v),
        Expr::SystemVar(sv) => match sv {
            SystemVar::Value => "#value".to_string(),
            SystemVar::Type => "#type".to_string(),
            SystemVar::Index => "#index".to_string(),
            SystemVar::SourceEntity => "#source_entity".to_string(),
            SystemVar::TargetEntity => "#target_entity".to_string(),
        },
        Expr::Literal(v) => match v {
            dataverse_lib::model::Value::String(s) => format!("\"{}\"", s),
            dataverse_lib::model::Value::Int(n) => n.to_string(),
            dataverse_lib::model::Value::Float(n) => n.to_string(),
            dataverse_lib::model::Value::Bool(b) => b.to_string(),
            dataverse_lib::model::Value::Null => "null".to_string(),
            other => format!("{:?}", other),
        },
    }
}

/// Internal type tracking state accumulated during tree building.
///
/// This is NOT exposed outside the tree module. Type data is embedded
/// directly on tree nodes (`TransformNode.output_type`, `.warning`,
/// `FieldMappingNode.target_type`, `.warning`).
#[derive(Debug, Default)]
struct TypeAccumulator {
    /// Transform ID -> output type after that transform.
    transform_types: HashMap<i64, ValueType>,
    /// All type warnings across all chains.
    warnings: Vec<TypeWarning>,
    /// Variable name -> resolved output type of its chain.
    variable_types: HashMap<String, ValueType>,
}

impl TypeAccumulator {
    /// Get the output type for a transform.
    fn type_for(&self, transform_id: i64) -> Option<&ValueType> {
        self.transform_types.get(&transform_id)
    }

    /// Get the warning for a transform, if any.
    fn warning_for(&self, transform_id: i64) -> Option<&TypeWarning> {
        self.warnings
            .iter()
            .find(|w| w.transform_id == transform_id)
    }

    /// Merge a chain type result into this accumulator.
    fn merge(&mut self, chain_result: &ChainTypeResult) {
        self.transform_types.extend(
            chain_result
                .transform_types
                .iter()
                .map(|(k, v)| (*k, v.clone())),
        );
        self.warnings.extend(chain_result.warnings.iter().cloned());
    }
}

/// Immutable lookup data for tree building.
struct TreeLookup<'a> {
    transforms: &'a [Transform],
    match_branches: &'a [MatchBranch],
    coalesce_chains: &'a [CoalesceChain],
    find_conditions: &'a [FindCondition],
}

impl<'a> TreeLookup<'a> {
    /// Get transforms for a specific parent, sorted by order.
    fn get_transforms(&self, parent_type: ParentType, parent_id: i64) -> Vec<Transform> {
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
}

/// Context for building the tree, holding lookup data and mutable type accumulator.
struct TreeBuildContext<'a> {
    lookup: TreeLookup<'a>,
    /// Entity mappings for looking up source entity names.
    entity_mappings: &'a [EntityMapping],
    /// Cached field types per source entity (for type tracking).
    field_type_cache: &'a FieldTypeCache,
    /// Cached field types per target entity (for target field type checking).
    target_field_cache: &'a FieldTypeCache,
    /// Internal type accumulator, populated during tree building.
    types: TypeAccumulator,
}

impl<'a> TreeBuildContext<'a> {
    /// Look up the source entity name for a given entity mapping ID.
    fn source_entity_for(&self, entity_mapping_id: i64) -> &str {
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
    field_type_cache: &FieldTypeCache,
    target_field_cache: &FieldTypeCache,
) -> Vec<TreeNode<MigrationTreeNode>> {
    let mut ctx = TreeBuildContext {
        lookup: TreeLookup {
            transforms: &transforms,
            match_branches: &match_branches,
            coalesce_chains: &coalesce_chains,
            find_conditions: &find_conditions,
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

// =============================================================================
// Type computation helper
// =============================================================================

/// Compute chain types using the current type tracking context.
///
/// Resolves `copy($var)` references using already-computed variable types,
/// and `copy(field)` paths using the field type cache from entity metadata.
fn compute_chain_types(
    transforms: &[Transform],
    source_entity: &str,
    ctx: &TreeBuildContext,
) -> ChainTypeResult {
    let variable_types = &ctx.types.variable_types;
    let field_types = ctx.field_type_cache.get(source_entity);

    propagate_chain_types(transforms, |data, current_type| {
        match data {
            TransformData::Copy { path } => {
                // Use the structured parser for all path resolution
                match parse_path(path) {
                    Ok(PathExpr::Variable(var_name)) => Some(
                        variable_types
                            .get(&var_name)
                            .cloned()
                            .unwrap_or(ValueType::Null),
                    ),
                    Ok(PathExpr::VariableNavigation {
                        name,
                        target,
                        path: field_path,
                    }) => resolve_variable_navigation(
                        &name,
                        target.as_deref(),
                        &field_path,
                        variable_types,
                        ctx,
                    ),
                    Ok(PathExpr::SystemVar(sys_var)) => match sys_var {
                        SystemVar::Value => Some(current_type.clone()),
                        SystemVar::Index => Some(ValueType::simple(AttributeType::Integer)),
                        SystemVar::Type | SystemVar::SourceEntity | SystemVar::TargetEntity => {
                            Some(ValueType::simple(AttributeType::String))
                        }
                    },
                    Ok(PathExpr::Field(_)) => {
                        // Field path - resolve from metadata cache.
                        resolve_field_path(path, source_entity, ctx)
                    }
                    Err(_) => None,
                }
            }
            _ => None, // Passthrough for all other dynamic cases
        }
    })
}

/// Resolve a variable navigation path (`$var.field` or `$var[target].field`).
///
/// Looks up the variable's declared type, determines the target entity from
/// its Lookup type, then resolves the remaining field path against that entity.
fn resolve_variable_navigation(
    var_name: &str,
    target: Option<&str>,
    field_path: &FieldPath,
    variable_types: &HashMap<String, ValueType>,
    ctx: &TreeBuildContext,
) -> Option<ValueType> {
    let var_type = variable_types.get(var_name)?;

    // The variable must be a lookup to navigate into it
    let targets = match var_type {
        ValueType::Known(FieldType::Lookup { targets, .. }) => targets,
        ValueType::Union(types) => {
            // Find the first lookup in the union
            let lookup = types.iter().find_map(|ft| match ft {
                FieldType::Lookup { targets, .. } => Some(targets),
                _ => None,
            })?;
            lookup
        }
        _ => {
            log::debug!(
                "type_tracking: variable ${} is not a lookup, cannot navigate into it",
                var_name,
            );
            return None;
        }
    };

    // Determine target entity
    let target_entity = if let Some(specified) = target {
        // Explicit target: $var[account].name
        if !targets.is_empty() && !targets.contains(&specified.to_string()) {
            log::debug!(
                "type_tracking: specified target '{}' not in targets {:?} for variable ${}",
                specified,
                targets,
                var_name,
            );
            return None;
        }
        specified.to_string()
    } else if targets.len() == 1 {
        targets[0].clone()
    } else if targets.is_empty() {
        log::debug!(
            "type_tracking: variable ${} lookup has no known targets, cannot navigate",
            var_name,
        );
        return None;
    } else {
        log::debug!(
            "type_tracking: variable ${} is polymorphic (targets: {:?}), use ${}[target].field syntax",
            var_name,
            targets,
            var_name,
        );
        return None;
    };

    log::debug!(
        "type_tracking: resolving ${}.{} via entity '{}'",
        var_name,
        field_path
            .segments
            .iter()
            .map(|s| s.field.as_str())
            .collect::<Vec<_>>()
            .join("."),
        target_entity,
    );

    // Resolve the field path starting from the target entity
    if field_path.segments.len() == 1 {
        let segment = &field_path.segments[0];
        let fields = ctx.field_type_cache.get(&target_entity)?;
        let field_type = fields.get(&segment.field)?;
        Some(ValueType::Known(field_type.clone()))
    } else {
        resolve_dotted_field_path(
            field_path,
            &target_entity,
            ctx,
            &format!(
                "${}.{}",
                var_name,
                field_path
                    .segments
                    .iter()
                    .map(|s| s.field.as_str())
                    .collect::<Vec<_>>()
                    .join(".")
            ),
        )
    }
}

/// Resolve a field path (possibly dotted) to its `ValueType` using the field type cache.
///
/// For simple paths like `name`, looks up the field directly on the source entity.
/// For dotted paths like `parentaccountid.name`, walks segment-by-segment:
/// each navigation segment must be a lookup, and its target entity is used to
/// resolve the next segment.
fn resolve_field_path(
    path: &str,
    source_entity: &str,
    ctx: &TreeBuildContext,
) -> Option<ValueType> {
    // Parse the path to get structured segments
    let field_path = match parse_path(path) {
        Ok(PathExpr::Field(fp)) => fp,
        _ => {
            // Not a field path (variable/system var handled elsewhere), or parse error
            log::debug!("type_tracking: failed to parse field path '{}'", path,);
            return None;
        }
    };

    if field_path.segments.is_empty() {
        return None;
    }

    // Simple (non-dotted) path: single segment lookup
    if field_path.segments.len() == 1 {
        let segment = &field_path.segments[0];
        let fields = ctx.field_type_cache.get(source_entity)?;
        let field_type = fields.get(&segment.field)?;
        log::debug!(
            "type_tracking: resolved field '{}' -> {:?}",
            path,
            field_type,
        );
        return Some(ValueType::Known(field_type.clone()));
    }

    // Dotted path: walk segment-by-segment
    resolve_dotted_field_path(&field_path, source_entity, ctx, path)
}

/// Walk a dotted field path segment-by-segment through the field type cache.
fn resolve_dotted_field_path(
    field_path: &FieldPath,
    source_entity: &str,
    ctx: &TreeBuildContext,
    original_path: &str,
) -> Option<ValueType> {
    let mut current_entity = source_entity.to_string();

    for (i, segment) in field_path.segments.iter().enumerate() {
        let is_last = i == field_path.segments.len() - 1;

        let fields = match ctx.field_type_cache.get(&current_entity) {
            Some(f) => f,
            None => {
                log::debug!(
                    "type_tracking: no metadata cached for entity '{}' while resolving '{}'",
                    current_entity,
                    original_path,
                );
                return None;
            }
        };

        let field_type = match fields.get(&segment.field) {
            Some(ft) => ft,
            None => {
                log::debug!(
                    "type_tracking: field '{}' not found on '{}' while resolving '{}'",
                    segment.field,
                    current_entity,
                    original_path,
                );
                return None;
            }
        };

        if is_last {
            // Last segment — this is the leaf field, return its type
            log::debug!(
                "type_tracking: resolved dotted path '{}' -> {:?}",
                original_path,
                field_type,
            );
            return Some(ValueType::Known(field_type.clone()));
        }

        // Navigation segment — must be a lookup
        let targets = match field_type {
            FieldType::Lookup { targets, .. } => targets,
            FieldType::Simple(_) | FieldType::OptionSet { .. } => {
                log::debug!(
                    "type_tracking: field '{}' on '{}' is not a lookup, cannot navigate in '{}'",
                    segment.field,
                    current_entity,
                    original_path,
                );
                return None;
            }
        };

        // Determine the target entity to navigate to
        let next_entity = if let Some(specified) = &segment.target {
            // Polymorphic lookup with explicit target: ownerid[systemuser]
            if !targets.is_empty() && !targets.contains(specified) {
                log::debug!(
                    "type_tracking: specified target '{}' not in targets {:?} for '{}' on '{}'",
                    specified,
                    targets,
                    segment.field,
                    current_entity,
                );
                return None;
            }
            specified.clone()
        } else if targets.len() == 1 {
            // Single-target lookup
            targets[0].clone()
        } else if targets.is_empty() {
            // Unknown targets — can't navigate
            log::debug!(
                "type_tracking: lookup '{}' on '{}' has no known targets, cannot navigate in '{}'",
                segment.field,
                current_entity,
                original_path,
            );
            return None;
        } else {
            // Polymorphic lookup without explicit target — ambiguous
            log::debug!(
                "type_tracking: polymorphic lookup '{}' on '{}' requires target specifier (targets: {:?}) in '{}'",
                segment.field,
                current_entity,
                targets,
                original_path,
            );
            return None;
        };

        current_entity = next_entity;
    }

    None
}
