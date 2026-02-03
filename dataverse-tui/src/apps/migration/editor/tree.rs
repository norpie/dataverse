//! Tree item implementation for the migration editor.

use rafter::widgets::TreeItem;
use rafter::widgets::TreeNode;
use tuidom::Color;
use tuidom::Element;
use tuidom::Style;

use crate::apps::migration::types::EntityMapping;
use crate::apps::migration::types::Mode;
use crate::apps::migration::types::Phase;

/// A node in the migration editor tree.
#[derive(Clone, Debug)]
pub enum MigrationTreeNode {
    /// A phase (top-level node).
    Phase(Phase),
    /// An entity mapping (child of a phase).
    EntityMapping(EntityMapping),
}

impl MigrationTreeNode {
    /// Get the ID suitable for database operations.
    pub fn id(&self) -> i64 {
        match self {
            Self::Phase(p) => p.id,
            Self::EntityMapping(em) => em.id,
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
                    "{} -> {}{}",
                    em.source_entity, em.target_entity, mode_indicator
                );

                Element::row().gap(1).child(
                    Element::text(&label).style(Style::new().foreground(Color::var("muted"))),
                )
            }
        }
    }
}

/// Build tree nodes from phases and entity mappings.
pub fn build_tree_nodes(
    phases: Vec<Phase>,
    entity_mappings: Vec<EntityMapping>,
) -> Vec<TreeNode<MigrationTreeNode>> {
    phases
        .into_iter()
        .map(|phase| {
            let phase_id = phase.id;
            let children: Vec<TreeNode<MigrationTreeNode>> = entity_mappings
                .iter()
                .filter(|em| em.phase_id == phase_id)
                .cloned()
                .map(|em| TreeNode::leaf(MigrationTreeNode::EntityMapping(em)))
                .collect();

            if children.is_empty() {
                TreeNode::leaf(MigrationTreeNode::Phase(phase))
            } else {
                TreeNode::branch(MigrationTreeNode::Phase(phase), children)
            }
        })
        .collect()
}
