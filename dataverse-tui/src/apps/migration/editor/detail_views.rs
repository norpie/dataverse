//! Detail panel rendering functions for the migration editor.

use tuidom::Color;
use tuidom::Element;
use tuidom::Style;

use crate::apps::migration::types::CoalesceChain;
use crate::apps::migration::types::FieldMapping;
use crate::apps::migration::types::FindCondition;
use crate::apps::migration::types::MatchBranch;
use crate::apps::migration::types::ParentType;
use crate::apps::migration::types::Transform;
use crate::apps::migration::types::Variable;

use super::tree::transform_display_text;
use super::MigrationEditor;

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

        Element::col()
            .gap(1)
            .child(
                Element::text(title).style(Style::new().bold().foreground(Color::var("interact"))),
            )
            .child(
                Element::col()
                    .child(
                        Element::row()
                            .gap(1)
                            .child(
                                Element::text("Parent")
                                    .style(Style::new().foreground(Color::var("muted"))),
                            )
                            .child(Element::text(&parent_name)),
                    )
                    .child(
                        Element::text(description)
                            .style(Style::new().foreground(Color::var("muted"))),
                    ),
            )
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

        Element::col()
            .gap(1)
            .child(
                Element::text("Variables")
                    .style(Style::new().bold().foreground(Color::var("interact"))),
            )
            .child(
                Element::col()
                    .child(
                        Element::row()
                            .gap(1)
                            .child(
                                Element::text("Parent")
                                    .style(Style::new().foreground(Color::var("muted"))),
                            )
                            .child(Element::text(&parent_name)),
                    )
                    .child(
                        Element::row()
                            .gap(1)
                            .child(
                                Element::text("Count")
                                    .style(Style::new().foreground(Color::var("muted"))),
                            )
                            .child(Element::text(format!("{}", var_count))),
                    )
                    .child(
                        Element::text("Computed values available in field mapping transforms")
                            .style(Style::new().foreground(Color::var("muted"))),
                    ),
            )
    }

    /// Render a single Variable detail view.
    pub(super) fn render_variable_detail(&self, variable: &Variable) -> Element {
        let em = self
            .entity_mappings
            .get()
            .iter()
            .find(|em| em.id == variable.entity_mapping_id)
            .cloned();

        let parent_name = em.map(|e| e.name).unwrap_or_else(|| "Unknown".to_string());

        Element::col()
            .gap(1)
            .child(
                Element::text("Variable")
                    .style(Style::new().bold().foreground(Color::var("interact"))),
            )
            .child(
                Element::col()
                    .child(
                        Element::row()
                            .gap(1)
                            .child(
                                Element::text("Name")
                                    .style(Style::new().foreground(Color::var("muted"))),
                            )
                            .child(Element::text(format!("${}", variable.name))),
                    )
                    .child(
                        Element::row()
                            .gap(1)
                            .child(
                                Element::text("Parent")
                                    .style(Style::new().foreground(Color::var("muted"))),
                            )
                            .child(Element::text(&parent_name)),
                    )
                    .child(
                        Element::text("Press Enter to edit transform chain")
                            .style(Style::new().foreground(Color::var("muted"))),
                    ),
            )
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

        Element::col()
            .gap(1)
            .child(
                Element::text("Field Mappings")
                    .style(Style::new().bold().foreground(Color::var("interact"))),
            )
            .child(
                Element::col()
                    .child(
                        Element::row()
                            .gap(1)
                            .child(
                                Element::text("Parent")
                                    .style(Style::new().foreground(Color::var("muted"))),
                            )
                            .child(Element::text(&parent_name)),
                    )
                    .child(
                        Element::row()
                            .gap(1)
                            .child(
                                Element::text("Count")
                                    .style(Style::new().foreground(Color::var("muted"))),
                            )
                            .child(Element::text(format!("{}", fm_count))),
                    )
                    .child(
                        Element::text("Mappings from source fields to target fields")
                            .style(Style::new().foreground(Color::var("muted"))),
                    ),
            )
    }

    /// Render a single Field Mapping detail view.
    pub(super) fn render_field_mapping_detail(&self, field_mapping: &FieldMapping) -> Element {
        let em = self
            .entity_mappings
            .get()
            .iter()
            .find(|em| em.id == field_mapping.entity_mapping_id)
            .cloned();

        let parent_name = em.map(|e| e.name).unwrap_or_else(|| "Unknown".to_string());

        Element::col()
            .gap(1)
            .child(
                Element::text("Field Mapping")
                    .style(Style::new().bold().foreground(Color::var("interact"))),
            )
            .child(
                Element::col()
                    .child(
                        Element::row()
                            .gap(1)
                            .child(
                                Element::text("Target Field")
                                    .style(Style::new().foreground(Color::var("muted"))),
                            )
                            .child(Element::text(&field_mapping.target_field)),
                    )
                    .child(
                        Element::row()
                            .gap(1)
                            .child(
                                Element::text("Parent")
                                    .style(Style::new().foreground(Color::var("muted"))),
                            )
                            .child(Element::text(&parent_name)),
                    )
                    .child(
                        Element::text("Press Enter to edit transform chain")
                            .style(Style::new().foreground(Color::var("muted"))),
                    ),
            )
    }

    /// Render a Transform detail view.
    pub(super) fn render_transform_detail(&self, transform: &Transform) -> Element {
        let display_text = transform_display_text(&transform.data);
        let parent_desc = match transform.parent_type {
            ParentType::FieldMapping => "Field Mapping",
            ParentType::Variable => "Variable",
            ParentType::MatchBranch => "Match Branch",
            ParentType::GuardFallback => "Guard Fallback",
            ParentType::CoalesceChain => "Coalesce Chain",
            ParentType::FindCondition => "Find Condition",
        };

        Element::col()
            .gap(1)
            .child(
                Element::text("Transform")
                    .style(Style::new().bold().foreground(Color::var("interact"))),
            )
            .child(
                Element::col()
                    .child(
                        Element::row()
                            .gap(1)
                            .child(
                                Element::text("Type")
                                    .style(Style::new().foreground(Color::var("muted"))),
                            )
                            .child(Element::text(&display_text)),
                    )
                    .child(
                        Element::row()
                            .gap(1)
                            .child(
                                Element::text("Order")
                                    .style(Style::new().foreground(Color::var("muted"))),
                            )
                            .child(Element::text(format!("{}", transform.order + 1))),
                    )
                    .child(
                        Element::row()
                            .gap(1)
                            .child(
                                Element::text("Parent")
                                    .style(Style::new().foreground(Color::var("muted"))),
                            )
                            .child(Element::text(parent_desc)),
                    )
                    .child(
                        Element::text("Press Enter to edit transform parameters")
                            .style(Style::new().foreground(Color::var("muted"))),
                    ),
            )
    }

    /// Render a Match Branch detail view.
    pub(super) fn render_match_branch_detail(&self, branch: &MatchBranch) -> Element {
        let branch_label = if branch.is_default {
            "Default".to_string()
        } else {
            format!("Branch {}", branch.order + 1)
        };

        Element::col()
            .gap(1)
            .child(
                Element::text("Match Branch")
                    .style(Style::new().bold().foreground(Color::var("interact"))),
            )
            .child(
                Element::col()
                    .child(
                        Element::row()
                            .gap(1)
                            .child(
                                Element::text("Branch")
                                    .style(Style::new().foreground(Color::var("muted"))),
                            )
                            .child(Element::text(&branch_label)),
                    )
                    .child(
                        Element::row()
                            .gap(1)
                            .child(
                                Element::text("Default")
                                    .style(Style::new().foreground(Color::var("muted"))),
                            )
                            .child(Element::text(if branch.is_default { "Yes" } else { "No" })),
                    )
                    .child(
                        Element::text("Press Enter to edit branch condition")
                            .style(Style::new().foreground(Color::var("muted"))),
                    ),
            )
    }

    /// Render a Coalesce Chain detail view.
    pub(super) fn render_coalesce_chain_detail(&self, chain: &CoalesceChain) -> Element {
        Element::col()
            .gap(1)
            .child(
                Element::text("Coalesce Fallback")
                    .style(Style::new().bold().foreground(Color::var("interact"))),
            )
            .child(
                Element::col()
                    .child(
                        Element::row()
                            .gap(1)
                            .child(
                                Element::text("Fallback")
                                    .style(Style::new().foreground(Color::var("muted"))),
                            )
                            .child(Element::text(format!("{}", chain.order + 1))),
                    )
                    .child(
                        Element::text("Add transforms to this fallback chain")
                            .style(Style::new().foreground(Color::var("muted"))),
                    ),
            )
    }

    /// Render a Find Condition detail view.
    pub(super) fn render_find_condition_detail(&self, condition: &FindCondition) -> Element {
        Element::col()
            .gap(1)
            .child(
                Element::text("Find Condition")
                    .style(Style::new().bold().foreground(Color::var("interact"))),
            )
            .child(
                Element::col()
                    .child(
                        Element::row()
                            .gap(1)
                            .child(
                                Element::text("Target Field")
                                    .style(Style::new().foreground(Color::var("muted"))),
                            )
                            .child(Element::text(&condition.target_field)),
                    )
                    .child(
                        Element::row()
                            .gap(1)
                            .child(
                                Element::text("Order")
                                    .style(Style::new().foreground(Color::var("muted"))),
                            )
                            .child(Element::text(format!("{}", condition.order + 1))),
                    )
                    .child(
                        Element::text("Press Enter to edit target field, add transforms to define the match value")
                            .style(Style::new().foreground(Color::var("muted"))),
                    ),
            )
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

        Element::col()
            .gap(1)
            .child(
                Element::text("Transform Chain")
                    .style(Style::new().bold().foreground(Color::var("interact"))),
            )
            .child(
                Element::col()
                    .child(
                        Element::row()
                            .gap(1)
                            .child(
                                Element::text("Parent")
                                    .style(Style::new().foreground(Color::var("muted"))),
                            )
                            .child(Element::text(parent_desc)),
                    )
                    .child(
                        Element::text("Multi-transform chain within a nested scope")
                            .style(Style::new().foreground(Color::var("muted"))),
                    ),
            )
    }
}
