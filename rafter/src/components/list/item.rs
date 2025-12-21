//! ListItem trait for items that can be displayed in a List.

use crate::node::Node;

/// Trait for items that can be displayed in a List.
///
/// Implement this trait to define how your items render in the list.
///
/// # Default Styling Helpers
///
/// The trait provides composable helpers for common styling patterns:
///
/// - [`apply_default_style`](ListItem::apply_default_style) - Wrap any Node with standard focus/selection colors
/// - [`render_default`](ListItem::render_default) - Convenience function for simple text items
/// - [`selection_indicator`](ListItem::selection_indicator) - Get checkbox string "[x]" or "[ ]"
///
/// # Examples
///
/// ## Simple text item
/// ```ignore
/// impl ListItem for Task {
///     fn render(&self, focused: bool, selected: bool) -> Node {
///         Self::render_default(&self.name, focused, selected)
///     }
/// }
/// ```
///
/// ## Item with checkbox
/// ```ignore
/// impl ListItem for Task {
///     fn render(&self, focused: bool, selected: bool) -> Node {
///         let checkbox = Self::selection_indicator(selected);
///         let content = view! {
///             text { format!("{}{}", checkbox, self.name) }
///         };
///         Self::apply_default_style(content, focused, selected)
///     }
/// }
/// ```
///
/// ## Custom layout with default colors
/// ```ignore
/// impl ListItem for FileEntry {
///     fn render(&self, focused: bool, selected: bool) -> Node {
///         let content = view! {
///             row (justify: space_between) {
///                 text { self.name.clone() }
///                 text (fg: muted) { self.size }
///             }
///         };
///         Self::apply_default_style(content, focused, selected)
///     }
/// }
/// ```
pub trait ListItem: Send + Sync + Clone + 'static {
    /// Height of each item in rows (constant for all items of this type).
    const HEIGHT: u16 = 1;

    /// Unique identifier for this item.
    ///
    /// Used for stable selection across item mutations. By default, uses the
    /// item's index as a string, which means selection shifts when items are
    /// added/removed. Override this to provide stable IDs.
    ///
    /// # Arguments
    /// * `index` - The item's current index in the list
    fn id(&self, index: usize) -> String {
        index.to_string()
    }

    /// Render this item.
    ///
    /// # Arguments
    /// * `focused` - Whether this item has the cursor (keyboard focus)
    /// * `selected` - Whether this item is selected (for multi-select)
    fn render(&self, focused: bool, selected: bool) -> Node;

    /// Helper: Apply default focus/selection colors to any Node.
    ///
    /// This wraps your custom content with standard colors based on state:
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
    ///         text { "🔥 " }
    ///         text { self.name.clone() }
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

    /// Helper: Render text with default focus/selection styling.
    ///
    /// Convenience wrapper for simple text-only items.
    ///
    /// # Example
    /// ```ignore
    /// impl ListItem for Task {
    ///     fn render(&self, focused: bool, selected: bool) -> Node {
    ///         Self::render_default(&self.name, focused, selected)
    ///     }
    /// }
    /// ```
    fn render_default(content: impl Into<String>, focused: bool, selected: bool) -> Node {
        use crate::style::Style;

        let text_node = Node::Text {
            content: content.into(),
            style: Style::new(),
        };
        Self::apply_default_style(text_node, focused, selected)
    }

    /// Helper: Get the selection indicator (checkbox).
    ///
    /// Returns `"■ "` for selected, `"□ "` for unselected.
    ///
    /// # Example
    /// ```ignore
    /// let checkbox = Self::selection_indicator(selected);
    /// let content = view! {
    ///     text { format!("{}{}", checkbox, self.name) }
    /// };
    /// Self::apply_default_style(content, focused, selected)
    /// ```
    fn selection_indicator(selected: bool) -> &'static str {
        if selected { "■ " } else { "□ " }
    }
}
