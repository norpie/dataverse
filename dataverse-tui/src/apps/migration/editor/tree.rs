//! Tree item implementation for the migration editor.

use rafter::widgets::TreeItem;
use rafter::widgets::TreeNode;
use tuidom::Color;
use tuidom::Element;
use tuidom::Style;

use crate::apps::migration::types::EntityMapping;
use crate::apps::migration::types::FieldMapping;
use crate::apps::migration::types::Mode;
use crate::apps::migration::types::Phase;
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
        )
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

                Element::row().gap(1).child(Element::text(&label))
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

                Element::row().gap(1).child(
                    Element::text(&label).style(Style::new().foreground(Color::var("muted"))),
                )
            }
            Self::MatchConfig { .. } => Element::row().gap(1).child(
                Element::text("Match Config").style(Style::new().foreground(Color::var("muted"))),
            ),
            Self::SourceFilter { .. } => Element::row().gap(1).child(
                Element::text("Source Filter").style(Style::new().foreground(Color::var("muted"))),
            ),
            Self::TargetFilter { .. } => Element::row().gap(1).child(
                Element::text("Target Filter").style(Style::new().foreground(Color::var("muted"))),
            ),
            Self::UnmatchedHandling { .. } => Element::row().gap(1).child(
                Element::text("Unmatched Handling")
                    .style(Style::new().foreground(Color::var("muted"))),
            ),
            Self::Passes { .. } => Element::row()
                .gap(1)
                .child(Element::text("Passes").style(Style::new().foreground(Color::var("muted")))),
            Self::TestGuids { .. } => Element::row().gap(1).child(
                Element::text("Test GUIDs").style(Style::new().foreground(Color::var("muted"))),
            ),
            Self::Variables { .. } => Element::row().gap(1).child(
                Element::text("Variables").style(Style::new().foreground(Color::var("muted"))),
            ),
            Self::Variable(v) => Element::row().gap(1).child(
                Element::text(format!("${}", v.name))
                    .style(Style::new().foreground(Color::var("primary"))),
            ),
            Self::FieldMappings { .. } => Element::row().gap(1).child(
                Element::text("Field Mappings").style(Style::new().foreground(Color::var("muted"))),
            ),
            Self::FieldMapping(fm) => Element::row().gap(1).child(
                Element::text(&fm.target_field)
                    .style(Style::new().foreground(Color::var("primary"))),
            ),
        }
    }
}

/// Build tree nodes from phases and entity mappings.
pub fn build_tree_nodes(
    phases: Vec<Phase>,
    entity_mappings: Vec<EntityMapping>,
    variables: Vec<Variable>,
    field_mappings: Vec<FieldMapping>,
) -> Vec<TreeNode<MigrationTreeNode>> {
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
                .map(|em| build_entity_mapping_node(em, &variables, &field_mappings))
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

        // Variables section
        let var_children: Vec<TreeNode<MigrationTreeNode>> = variables
            .iter()
            .filter(|v| v.entity_mapping_id == em_id)
            .cloned()
            .map(|v| TreeNode::leaf(MigrationTreeNode::Variable(v)))
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

        // Field mappings section
        let fm_children: Vec<TreeNode<MigrationTreeNode>> = field_mappings
            .iter()
            .filter(|fm| fm.entity_mapping_id == em_id)
            .cloned()
            .map(|fm| TreeNode::leaf(MigrationTreeNode::FieldMapping(fm)))
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
