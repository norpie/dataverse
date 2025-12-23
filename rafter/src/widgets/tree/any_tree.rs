//! Type-erased tree trait for use in Node.

use std::any::Any;
use std::ops::Range;

use crate::context::AppContext;
use crate::input::events::{Modifiers, ScrollDirection};
use crate::input::keybinds::KeyCombo;
use crate::node::Node;
use crate::widgets::events::{EventResult, WidgetEvents};
use crate::widgets::scrollbar::{
    ScrollbarConfig, ScrollbarDrag, ScrollbarGeometry, ScrollbarState,
};
use crate::widgets::traits::AnySelectable;

use super::item::TreeItem;
use super::state::Tree;

/// Type-erased tree operations for use in Node.
pub trait AnyTree: Send + Sync + std::fmt::Debug {
    /// Get the tree ID as a string.
    fn id_string(&self) -> String;

    /// Get the item height.
    fn item_height(&self) -> u16;

    /// Get the number of visible nodes.
    fn len(&self) -> usize;

    /// Check if empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the cursor position (index into visible list).
    fn cursor(&self) -> Option<usize>;

    /// Get the cursor node ID.
    fn cursor_id(&self) -> Option<String>;

    /// Check if a visible index is selected.
    fn is_selected_at(&self, visible_index: usize) -> bool;

    /// Get the scroll offset.
    fn scroll_offset(&self) -> u16;

    /// Set the viewport height.
    fn set_viewport_height(&self, height: u16);

    /// Get the viewport height.
    fn viewport_height(&self) -> u16;

    /// Get the visible node range.
    fn visible_range(&self) -> Range<usize>;

    /// Get the total content height.
    fn total_height(&self) -> u16;

    /// Render a specific visible node.
    fn render_item(&self, visible_index: usize) -> Option<Node>;

    /// Clone as boxed trait object.
    fn clone_box(&self) -> Box<dyn AnyTree>;

    /// As Any for downcasting.
    fn as_any(&self) -> &dyn Any;

    // -------------------------------------------------------------------------
    // Event handlers
    // -------------------------------------------------------------------------

    /// Handle a key event.
    fn on_key(&self, key: &KeyCombo, cx: &AppContext) -> EventResult;

    /// Handle a click event at the given position within the tree bounds.
    fn on_click(&self, x: u16, y: u16, cx: &AppContext) -> EventResult;

    /// Handle a hover event at the given position within the tree bounds.
    fn on_hover(&self, x: u16, y: u16, cx: &AppContext) -> EventResult;

    /// Handle a scroll event.
    fn on_scroll(&self, direction: ScrollDirection, amount: u16, cx: &AppContext) -> EventResult;

    /// Handle a drag event.
    fn on_drag(&self, x: u16, y: u16, modifiers: Modifiers, cx: &AppContext) -> EventResult;

    /// Handle a release event.
    fn on_release(&self, cx: &AppContext) -> EventResult;

    /// Handle a click with modifier keys (Ctrl, Shift).
    fn on_click_with_modifiers(
        &self,
        y_in_viewport: u16,
        ctrl: bool,
        shift: bool,
        cx: &AppContext,
    ) -> EventResult;

    // -------------------------------------------------------------------------
    // Scrollbar support
    // -------------------------------------------------------------------------

    /// Get the scrollbar configuration.
    fn scrollbar_config(&self) -> ScrollbarConfig;

    /// Get the vertical scrollbar geometry.
    fn vertical_scrollbar(&self) -> Option<ScrollbarGeometry>;

    /// Set the vertical scrollbar geometry.
    fn set_vertical_scrollbar(&self, geometry: Option<ScrollbarGeometry>);

    /// Check if vertical scrolling is needed.
    fn needs_vertical_scrollbar(&self) -> bool;

    /// Scroll to a position based on a ratio (0.0 - 1.0).
    fn scroll_to_ratio_y(&self, ratio: f32);

    /// Get current drag state.
    fn drag(&self) -> Option<ScrollbarDrag>;

    /// Set current drag state.
    fn set_drag(&self, drag: Option<ScrollbarDrag>);

    /// Get this widget as an AnySelectable trait object.
    fn as_any_selectable(&self) -> &dyn AnySelectable;
}

impl<T: TreeItem + std::fmt::Debug> AnyTree for Tree<T> {
    fn id_string(&self) -> String {
        self.id_string()
    }

    fn item_height(&self) -> u16 {
        T::HEIGHT
    }

    fn len(&self) -> usize {
        self.visible_len()
    }

    fn cursor(&self) -> Option<usize> {
        self.cursor()
    }

    fn cursor_id(&self) -> Option<String> {
        self.cursor_id()
    }

    fn is_selected_at(&self, visible_index: usize) -> bool {
        self.is_selected_at(visible_index)
    }

    fn scroll_offset(&self) -> u16 {
        self.scroll_offset()
    }

    fn set_viewport_height(&self, height: u16) {
        self.set_viewport_height(height);
    }

    fn viewport_height(&self) -> u16 {
        self.viewport_height()
    }

    fn visible_range(&self) -> Range<usize> {
        self.visible_range()
    }

    fn total_height(&self) -> u16 {
        self.total_height()
    }

    fn render_item(&self, visible_index: usize) -> Option<Node> {
        let node = self.visible_node(visible_index)?;
        let is_focused = self.cursor() == Some(visible_index);
        let is_selected = self.is_selected_at(visible_index);
        Some(
            node.item
                .render(is_focused, is_selected, node.depth, node.is_expanded),
        )
    }

    fn clone_box(&self) -> Box<dyn AnyTree> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn on_key(&self, key: &KeyCombo, cx: &AppContext) -> EventResult {
        WidgetEvents::on_key(self, key, cx)
    }

    fn on_click(&self, x: u16, y: u16, cx: &AppContext) -> EventResult {
        WidgetEvents::on_click(self, x, y, cx)
    }

    fn on_hover(&self, x: u16, y: u16, cx: &AppContext) -> EventResult {
        WidgetEvents::on_hover(self, x, y, cx)
    }

    fn on_scroll(&self, direction: ScrollDirection, amount: u16, cx: &AppContext) -> EventResult {
        WidgetEvents::on_scroll(self, direction, amount, cx)
    }

    fn on_drag(&self, x: u16, y: u16, modifiers: Modifiers, cx: &AppContext) -> EventResult {
        WidgetEvents::on_drag(self, x, y, modifiers, cx)
    }

    fn on_release(&self, cx: &AppContext) -> EventResult {
        WidgetEvents::on_release(self, cx)
    }

    fn on_click_with_modifiers(
        &self,
        y_in_viewport: u16,
        ctrl: bool,
        shift: bool,
        cx: &AppContext,
    ) -> EventResult {
        Tree::on_click_with_modifiers(self, y_in_viewport, ctrl, shift, cx)
    }

    fn scrollbar_config(&self) -> ScrollbarConfig {
        ScrollbarState::scrollbar_config(self)
    }

    fn vertical_scrollbar(&self) -> Option<ScrollbarGeometry> {
        ScrollbarState::vertical_scrollbar(self)
    }

    fn set_vertical_scrollbar(&self, geometry: Option<ScrollbarGeometry>) {
        ScrollbarState::set_vertical_scrollbar(self, geometry);
    }

    fn needs_vertical_scrollbar(&self) -> bool {
        ScrollbarState::needs_vertical_scrollbar(self)
    }

    fn scroll_to_ratio_y(&self, ratio: f32) {
        ScrollbarState::scroll_to_ratio(self, None, Some(ratio));
    }

    fn drag(&self) -> Option<ScrollbarDrag> {
        ScrollbarState::drag(self)
    }

    fn set_drag(&self, drag: Option<ScrollbarDrag>) {
        ScrollbarState::set_drag(self, drag);
    }

    fn as_any_selectable(&self) -> &dyn AnySelectable {
        self
    }
}

impl Clone for Box<dyn AnyTree> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

// =============================================================================
// AnySelectable implementation
// =============================================================================

impl<T: TreeItem + std::fmt::Debug> AnySelectable for Tree<T> {
    fn id_string(&self) -> String {
        Tree::id_string(self)
    }

    fn on_click_with_modifiers(
        &self,
        y_in_viewport: u16,
        ctrl: bool,
        shift: bool,
        cx: &AppContext,
    ) -> EventResult {
        Tree::on_click_with_modifiers(self, y_in_viewport, ctrl, shift, cx)
    }
}
