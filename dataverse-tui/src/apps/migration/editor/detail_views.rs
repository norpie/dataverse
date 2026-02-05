//! Detail panel rendering functions for the migration editor.

use tuidom::Color;
use tuidom::Element;
use tuidom::Style;

use crate::apps::migration::types::FieldMapping;
use crate::apps::migration::types::Variable;

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
}
