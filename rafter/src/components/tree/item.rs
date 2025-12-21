//! TreeItem trait for items that can be displayed in a Tree.

use crate::node::Node;

/// Trait for items that can be displayed in a Tree.
///
/// Implement this trait to define hierarchical data for the tree.
///
/// # Default Styling Helpers
///
/// The trait provides composable helpers for common tree styling patterns:
///
/// - [`apply_default_style`](TreeItem::apply_default_style) - Wrap any Node with standard focus/selection colors
/// - [`render_default`](TreeItem::render_default) - Convenience function combining all defaults
/// - [`indentation`](TreeItem::indentation) - Get indentation string for a depth level
/// - [`expansion_indicator`](TreeItem::expansion_indicator) - Get "▶", "▼", or "  " icon
/// - [`selection_indicator`](TreeItem::selection_indicator) - Get checkbox string "[x]" or "[ ]"
///
/// # Examples
///
/// ## Simple tree with defaults
/// ```ignore
/// impl TreeItem for FileNode {
///     fn id(&self) -> String { self.name.clone() }
///     fn children(&self) -> Vec<Self> { self.children.clone() }
///     
///     fn render(&self, focused: bool, selected: bool, depth: u16, expanded: bool) -> Node {
///         Self::render_default(&self.name, focused, selected, depth, self.is_expandable(), expanded)
///     }
/// }
/// ```
///
/// ## Tree with checkbox
/// ```ignore
/// impl TreeItem for FileNode {
///     // ... id, children ...
///     
///     fn render(&self, focused: bool, selected: bool, depth: u16, expanded: bool) -> Node {
///         let indent = Self::indentation(depth);
///         let icon = Self::expansion_indicator(self.is_expandable(), expanded);
///         let checkbox = Self::selection_indicator(selected);
///         
///         let content = view! {
///             text { format!("{}{}{}{}", indent, icon, checkbox, self.name) }
///         };
///         Self::apply_default_style(content, focused, selected)
///     }
/// }
/// ```
///
/// ## Custom layout with default colors
/// ```ignore
/// impl TreeItem for FileNode {
///     // ... id, children ...
///     
///     fn render(&self, focused: bool, selected: bool, depth: u16, expanded: bool) -> Node {
///         let indent = Self::indentation(depth);
///         let icon = Self::expansion_indicator(self.is_expandable(), expanded);
///         
///         let content = view! {
///             row (justify: space_between) {
///                 text { format!("{}{} 📁 {}", indent, icon, self.name) }
///                 text (fg: muted) { self.size }
///             }
///         };
///         Self::apply_default_style(content, focused, selected)
///     }
/// }
/// ```
pub trait TreeItem: Send + Sync + Clone + 'static {
    /// Height of each node in rows (constant for virtualization).
    const HEIGHT: u16 = 1;

    /// Unique, stable identifier for this node.
    ///
    /// This ID must be unique across the entire tree and stable across
    /// updates to maintain expand/collapse and selection state.
    fn id(&self) -> String;

    /// Get child items. Return an empty vec for leaf nodes.
    fn children(&self) -> Vec<Self>;

    /// Whether this node can be expanded.
    ///
    /// Default implementation returns true if the node has children.
    fn is_expandable(&self) -> bool {
        !self.children().is_empty()
    }

    /// Render this node.
    ///
    /// # Arguments
    /// * `focused` - Whether this node has the cursor (keyboard focus)
    /// * `selected` - Whether this node is selected
    /// * `depth` - Indentation level (0 = root level)
    /// * `expanded` - Whether children are currently visible
    fn render(&self, focused: bool, selected: bool, depth: u16, expanded: bool) -> Node;

    /// Helper: Apply default focus/selection colors to any Node.
    ///
    /// This wraps your custom content with standard colors based on state.
    /// Same color scheme as [`ListItem::apply_default_style`](crate::components::ListItem::apply_default_style):
    /// - **Focused (cursor)**: Purple background (`#A277FF`), inverted foreground
    /// - **Selected**: Dimmer purple background (`#6E5494`), inverted foreground
    /// - **Neither**: default colors
    ///
    /// Note: "Focused" means the item has the keyboard cursor, not window focus.
    /// Both focused and selected use the same style (purple bg), just different when
    /// both states are true (focused takes priority).
    /// Use this to build complex layouts while maintaining consistent styling.
    ///
    /// # Example
    /// ```ignore
    /// let content = view! {
    ///     row {
    ///         text { format!("{}{}", Self::indentation(depth), self.name) }
    ///         text (fg: muted) { self.metadata }
    ///     }
    /// };
    /// Self::apply_default_style(content, focused, selected)
    /// ```
    fn apply_default_style(child: Node, focused: bool, selected: bool) -> Node {
        use crate::color::{Color, StyleColor};
        use crate::node::{Layout, Size};
        use crate::style::Style;

        let style = if focused || selected {
            // Focused gets brighter purple, selected gets dimmer purple
            let bg_color = if focused {
                Color::hex(0xA277FF) // Bright purple for cursor
            } else {
                Color::hex(0x6E5494) // Dimmer purple for selected
            };
            Style::new()
                .bg(StyleColor::Concrete(bg_color))
                .fg(StyleColor::Named("background".into())) // Inverted: use bg color as fg
        } else {
            Style::new()
        };

        let layout = Layout {
            width: Size::Flex(1),
            ..Default::default()
        };

        Node::Row {
            children: vec![child],
            style,
            layout,
        }
    }

    /// Helper: Get the indentation string for a given depth.
    ///
    /// Default: 2 spaces per level.
    ///
    /// # Example
    /// ```ignore
    /// let indent = Self::indentation(depth);
    /// let line = format!("{}{}", indent, self.name);
    /// ```
    fn indentation(depth: u16) -> String {
        "  ".repeat(depth as usize)
    }

    /// Helper: Get the expansion indicator icon.
    ///
    /// Returns:
    /// - `"▼ "` for expandable + expanded
    /// - `"▶ "` for expandable + collapsed
    /// - `"  "` for non-expandable (leaf node)
    ///
    /// # Example
    /// ```ignore
    /// let icon = Self::expansion_indicator(self.is_expandable(), expanded);
    /// let line = format!("{}{}", icon, self.name);
    /// ```
    fn expansion_indicator(is_expandable: bool, expanded: bool) -> &'static str {
        if is_expandable {
            if expanded { "▼ " } else { "▶ " }
        } else {
            "  "
        }
    }

    /// Helper: Get the selection indicator (checkbox).
    ///
    /// Returns `"■ "` for selected, `"□ "` for unselected.
    ///
    /// # Example
    /// ```ignore
    /// let checkbox = Self::selection_indicator(selected);
    /// let line = format!("{}{}", checkbox, self.name);
    /// ```
    fn selection_indicator(selected: bool) -> &'static str {
        if selected {
            "■ "
        } else {
            "□ "
        }
    }

    /// Helper: Render tree node with default styling.
    ///
    /// Convenience wrapper that combines indentation, expansion indicator,
    /// and default colors. Use this for simple text-only tree nodes.
    ///
    /// # Example
    /// ```ignore
    /// impl TreeItem for FileNode {
    ///     fn render(&self, focused: bool, selected: bool, depth: u16, expanded: bool) -> Node {
    ///         Self::render_default(
    ///             &self.name,
    ///             focused,
    ///             selected,
    ///             depth,
    ///             !self.children.is_empty(),
    ///             expanded,
    ///         )
    ///     }
    /// }
    /// ```
    fn render_default(
        content: impl Into<String>,
        focused: bool,
        selected: bool,
        depth: u16,
        is_expandable: bool,
        expanded: bool,
    ) -> Node {
        use crate::style::Style;

        let indent = Self::indentation(depth);
        let icon = Self::expansion_indicator(is_expandable, expanded);
        let line = format!("{}{}{}", indent, icon, content.into());

        let text_node = Node::Text {
            content: line,
            style: Style::new(),
        };

        Self::apply_default_style(text_node, focused, selected)
    }
}
