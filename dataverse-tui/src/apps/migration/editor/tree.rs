//! Tree node types and rendering for the migration editor.

use std::collections::HashMap;

use dataverse_lib::model::FieldType;
use dataverse_lib::model::ValueType;
use rafter::element;
use rafter::widgets::Text;
use rafter::widgets::TreeItem;
use tuidom::Element;

use crate::apps::migration::types::ChainOutputWarning;
use crate::apps::migration::types::CoalesceChain;
use crate::apps::migration::types::CompareOp;
use crate::apps::migration::types::Condition;
use crate::apps::migration::types::EntityMapping;
use crate::apps::migration::types::Expr;
use crate::apps::migration::types::FieldMapping;
use crate::apps::migration::types::FindCondition;
use crate::apps::migration::types::MatchBranch;
use crate::apps::migration::types::MatchCondition;
use crate::apps::migration::types::Mode;
use crate::apps::migration::types::ParentType;
use crate::apps::migration::types::Phase;
use crate::apps::migration::types::SystemVar;
use crate::apps::migration::types::Transform;
use crate::apps::migration::types::TransformData;
use crate::apps::migration::types::TypeWarning;
use crate::apps::migration::types::Variable;

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
    /// A condition within a match config (find mode).
    MatchCondition(MatchCondition),
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
            Self::MatchCondition(mc) => Some(mc.entity_mapping_id),
            Self::Chain { .. } => None, // Get via parent
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
                | Self::MatchCondition(_)
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
            Self::MatchCondition(mc) => format!("match-condition-{}", mc.id),
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
            Self::MatchCondition(mc) => {
                let label = format!("Condition: {}", mc.target_field);
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

// =============================================================================
// Display text helpers
// =============================================================================

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
