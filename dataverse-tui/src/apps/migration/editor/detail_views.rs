//! Detail panel rendering functions for the migration editor.

use rafter::element;
use rafter::widgets::Text;
use tuidom::Element;

use super::tree::transform_display_text;
use super::tree::FieldMappingNode;
use super::tree::TransformNode;
use super::tree::VariableNode;
use super::MigrationEditor;
use crate::apps::migration::types::CoalesceChain;
use crate::apps::migration::types::FindCondition;
use crate::apps::migration::types::MatchBranch;
use crate::apps::migration::types::ParentType;

impl MigrationEditor {
    /// Render a generic config detail view.
    pub(super) fn render_config_detail(
        &self,
        title: &str,
        entity_mapping_id: i64,
        description: &str,
    ) -> Element {
        let em = self
            .entity_mappings
            .get()
            .iter()
            .find(|em| em.id == entity_mapping_id)
            .cloned();

        let parent_name = em.map(|e| e.name).unwrap_or_else(|| "Unknown".to_string());
        let title = title.to_string();
        let description = description.to_string();

        element! {
            column (gap: 1) {
                text (content: {title}) style (bold, fg: interact)
                column {
                    row (gap: 1) {
                        text (content: "Parent") style (fg: muted)
                        text (content: {parent_name})
                    }
                    text (content: {description}) style (fg: muted)
                }
            }
        }
    }

    /// Render the Variables section detail view.
    pub(super) fn render_variables_detail(&self, entity_mapping_id: i64) -> Element {
        let em = self
            .entity_mappings
            .get()
            .iter()
            .find(|em| em.id == entity_mapping_id)
            .cloned();

        let parent_name = em.map(|e| e.name).unwrap_or_else(|| "Unknown".to_string());

        let var_count = self
            .variables
            .get()
            .iter()
            .filter(|v| v.entity_mapping_id == entity_mapping_id)
            .count();
        let var_count_str = format!("{}", var_count);

        element! {
            column (gap: 1) {
                text (content: "Variables") style (bold, fg: interact)
                column {
                    row (gap: 1) {
                        text (content: "Parent") style (fg: muted)
                        text (content: {parent_name})
                    }
                    row (gap: 1) {
                        text (content: "Count") style (fg: muted)
                        text (content: {var_count_str})
                    }
                    text (content: "Computed values available in field mapping transforms") style (fg: muted)
                }
            }
        }
    }

    /// Render a single Variable detail view.
    pub(super) fn render_variable_detail(&self, vn: &VariableNode) -> Element {
        let variable = &vn.variable;
        let em = self
            .entity_mappings
            .get()
            .iter()
            .find(|em| em.id == variable.entity_mapping_id)
            .cloned();

        let parent_name = em.map(|e| e.name).unwrap_or_else(|| "Unknown".to_string());
        let var_name = format!("${}", variable.name);
        let declared_type_str = variable.declared_type.display();
        let has_warning = vn.warning.is_some();
        let chain_output_str = vn
            .warning
            .as_ref()
            .map(|w| w.chain_output.display())
            .unwrap_or_default();
        let target_type_str = vn
            .warning
            .as_ref()
            .map(|w| w.target_type.display())
            .unwrap_or_default();

        element! {
            column (gap: 1) {
                text (content: "Variable") style (bold, fg: interact)
                column {
                    row (gap: 1) {
                        text (content: "Name") style (fg: muted)
                        text (content: {var_name})
                    }
                    row (gap: 1) {
                        text (content: "Type") style (fg: muted)
                        text (content: {declared_type_str})
                    }
                    row (gap: 1) {
                        text (content: "Parent") style (fg: muted)
                        text (content: {parent_name})
                    }
                    text (content: "Press Enter to edit transform chain") style (fg: muted)
                }
                if has_warning {
                    column {
                        text (content: "Type Warning") style (bold, fg: warning)
                        row (gap: 1) {
                            text (content: "Chain Output") style (fg: muted)
                            text (content: {chain_output_str}) style (fg: warning)
                        }
                        row (gap: 1) {
                            text (content: "Declared Type") style (fg: muted)
                            text (content: {target_type_str})
                        }
                        text (content: "Chain output type is incompatible with declared type") style (fg: muted)
                    }
                }
            }
        }
    }

    /// Render the Field Mappings section detail view.
    pub(super) fn render_field_mappings_detail(&self, entity_mapping_id: i64) -> Element {
        let em = self
            .entity_mappings
            .get()
            .iter()
            .find(|em| em.id == entity_mapping_id)
            .cloned();

        let parent_name = em.map(|e| e.name).unwrap_or_else(|| "Unknown".to_string());

        let fm_count = self
            .field_mappings
            .get()
            .iter()
            .filter(|fm| fm.entity_mapping_id == entity_mapping_id)
            .count();
        let fm_count_str = format!("{}", fm_count);

        element! {
            column (gap: 1) {
                text (content: "Field Mappings") style (bold, fg: interact)
                column {
                    row (gap: 1) {
                        text (content: "Parent") style (fg: muted)
                        text (content: {parent_name})
                    }
                    row (gap: 1) {
                        text (content: "Count") style (fg: muted)
                        text (content: {fm_count_str})
                    }
                    text (content: "Mappings from source fields to target fields") style (fg: muted)
                }
            }
        }
    }

    /// Render a single Field Mapping detail view.
    pub(super) fn render_field_mapping_detail(&self, fmn: &FieldMappingNode) -> Element {
        let field_mapping = &fmn.field_mapping;
        let em = self
            .entity_mappings
            .get()
            .iter()
            .find(|em| em.id == field_mapping.entity_mapping_id)
            .cloned();

        let parent_name = em.map(|e| e.name).unwrap_or_else(|| "Unknown".to_string());
        let target_field = field_mapping.target_field.clone();
        let has_warning = fmn.warning.is_some();
        let chain_output_str = fmn
            .warning
            .as_ref()
            .map(|w| w.chain_output.display())
            .unwrap_or_default();
        let target_type_str = fmn
            .warning
            .as_ref()
            .map(|w| w.target_type.display())
            .unwrap_or_default();

        element! {
            column (gap: 1) {
                text (content: "Field Mapping") style (bold, fg: interact)
                column {
                    row (gap: 1) {
                        text (content: "Target Field") style (fg: muted)
                        text (content: {target_field})
                    }
                    row (gap: 1) {
                        text (content: "Parent") style (fg: muted)
                        text (content: {parent_name})
                    }
                    text (content: "Press Enter to edit transform chain") style (fg: muted)
                }
                if has_warning {
                    column {
                        text (content: "Type Warning") style (bold, fg: warning)
                        row (gap: 1) {
                            text (content: "Chain Output") style (fg: muted)
                            text (content: {chain_output_str}) style (fg: warning)
                        }
                        row (gap: 1) {
                            text (content: "Target Expects") style (fg: muted)
                            text (content: {target_type_str})
                        }
                        text (content: "Chain output type is incompatible with target field") style (fg: muted)
                    }
                }
            }
        }
    }

    /// Render a Transform detail view.
    pub(super) fn render_transform_detail(&self, tn: &TransformNode) -> Element {
        let transform = &tn.transform;
        let display_text = transform_display_text(&transform.data);
        let parent_desc = match transform.parent_type {
            ParentType::FieldMapping => "Field Mapping",
            ParentType::Variable => "Variable",
            ParentType::MatchBranch => "Match Branch",
            ParentType::GuardFallback => "Guard Fallback",
            ParentType::CoalesceChain => "Coalesce Chain",
            ParentType::FindCondition => "Find Condition",
        };
        let order_str = format!("{}", transform.order + 1);
        let has_type = tn.output_type.is_some();
        let type_str = tn
            .output_type
            .as_ref()
            .map(|t| t.display())
            .unwrap_or_default();
        let has_warning = tn.warning.is_some();
        let expected_str = tn
            .warning
            .as_ref()
            .map(|w| w.expected.display())
            .unwrap_or_default();
        let actual_str = tn
            .warning
            .as_ref()
            .map(|w| w.actual.display())
            .unwrap_or_default();

        element! {
            column (gap: 1) {
                text (content: "Transform") style (bold, fg: interact)
                column {
                    row (gap: 1) {
                        text (content: "Type") style (fg: muted)
                        text (content: {display_text})
                    }
                    row (gap: 1) {
                        text (content: "Order") style (fg: muted)
                        text (content: {order_str})
                    }
                    row (gap: 1) {
                        text (content: "Parent") style (fg: muted)
                        text (content: {parent_desc})
                    }
                    if has_type {
                        row (gap: 1) {
                            text (content: "Output") style (fg: muted)
                            text (content: {type_str})
                        }
                    }
                    text (content: "Press Enter to edit transform parameters") style (fg: muted)
                }
                if has_warning {
                    column {
                        text (content: "Type Warning") style (bold, fg: warning)
                        row (gap: 1) {
                            text (content: "Expected") style (fg: muted)
                            text (content: {expected_str})
                        }
                        row (gap: 1) {
                            text (content: "Actual") style (fg: muted)
                            text (content: {actual_str}) style (fg: warning)
                        }
                        text (content: "Consider adding a convert transform before this one") style (fg: muted)
                    }
                }
            }
        }
    }

    /// Render a Match Branch detail view.
    pub(super) fn render_match_branch_detail(&self, branch: &MatchBranch) -> Element {
        let branch_label = if branch.is_default {
            "Default".to_string()
        } else {
            format!("Branch {}", branch.order + 1)
        };
        let is_default_str = if branch.is_default { "Yes" } else { "No" };

        element! {
            column (gap: 1) {
                text (content: "Match Branch") style (bold, fg: interact)
                column {
                    row (gap: 1) {
                        text (content: "Branch") style (fg: muted)
                        text (content: {branch_label})
                    }
                    row (gap: 1) {
                        text (content: "Default") style (fg: muted)
                        text (content: {is_default_str})
                    }
                    text (content: "Press Enter to edit branch condition") style (fg: muted)
                }
            }
        }
    }

    /// Render a Coalesce Chain detail view.
    pub(super) fn render_coalesce_chain_detail(&self, chain: &CoalesceChain) -> Element {
        let fallback_str = format!("{}", chain.order + 1);

        element! {
            column (gap: 1) {
                text (content: "Coalesce Fallback") style (bold, fg: interact)
                column {
                    row (gap: 1) {
                        text (content: "Fallback") style (fg: muted)
                        text (content: {fallback_str})
                    }
                    text (content: "Add transforms to this fallback chain") style (fg: muted)
                }
            }
        }
    }

    /// Render a Find Condition detail view.
    pub(super) fn render_find_condition_detail(&self, condition: &FindCondition) -> Element {
        let target_field = condition.target_field.clone();
        let order_str = format!("{}", condition.order + 1);

        element! {
            column (gap: 1) {
                text (content: "Find Condition") style (bold, fg: interact)
                column {
                    row (gap: 1) {
                        text (content: "Target Field") style (fg: muted)
                        text (content: {target_field})
                    }
                    row (gap: 1) {
                        text (content: "Order") style (fg: muted)
                        text (content: {order_str})
                    }
                    text (content: "Press Enter to edit target field, add transforms to define the match value") style (fg: muted)
                }
            }
        }
    }

    /// Render a Chain wrapper detail view.
    pub(super) fn render_chain_detail(&self, parent_type: ParentType, _parent_id: i64) -> Element {
        let parent_desc = match parent_type {
            ParentType::MatchBranch => "Match Branch",
            ParentType::GuardFallback => "Guard Fallback",
            ParentType::CoalesceChain => "Coalesce Fallback",
            ParentType::FindCondition => "Find Condition",
            _ => "Parent",
        };

        element! {
            column (gap: 1) {
                text (content: "Transform Chain") style (bold, fg: interact)
                column {
                    row (gap: 1) {
                        text (content: "Parent") style (fg: muted)
                        text (content: {parent_desc})
                    }
                    text (content: "Multi-transform chain within a nested scope") style (fg: muted)
                }
            }
        }
    }
}
