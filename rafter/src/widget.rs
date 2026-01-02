//! Widget trait for interactive components.
//!
//! Widgets are stateful interactive components that:
//! - Generate a tuidom Element for rendering
//! - Handle input events (keys, clicks, scroll)
//! - Return what happened via WidgetResult
//!
//! The framework dispatches to user handlers based on WidgetResult.

use tuidom::{Key, LayoutResult, Modifiers};

/// Result of a widget handling an input event.
///
/// This tells the framework what happened, so it can dispatch
/// to the appropriate user-defined handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WidgetResult {
    /// Event was not handled by this widget.
    #[default]
    Ignored,
    /// Event was handled but no semantic action occurred.
    Handled,
    /// Widget was activated (Enter, click on button, etc.).
    Activated,
    /// Widget value changed (input text, checkbox toggle, etc.).
    Changed,
    /// Cursor moved to a new item (list, tree, table).
    CursorMoved,
    /// Item was selected from a dropdown (select, autocomplete).
    Selected,
    /// Tree node was expanded.
    Expanded,
    /// Tree node was collapsed.
    Collapsed,
    /// Table column was sorted.
    Sorted,
    /// Form was submitted (Enter on input with submit handler).
    Submitted,
}

impl WidgetResult {
    /// Check if the event was handled (not Ignored).
    pub fn is_handled(&self) -> bool {
        !matches!(self, WidgetResult::Ignored)
    }
}

/// Trait for interactive widgets.
///
/// Widgets implement this trait to participate in the rafter event system.
/// The framework calls handle_* methods when input events occur, and
/// dispatches to user handlers based on the returned WidgetResult.
///
/// # Lifecycle
///
/// 1. Widget state is stored in `State<T>` for reactivity
/// 2. `element()` is called to generate the visual representation
/// 3. Input events trigger `handle_*` methods
/// 4. WidgetResult determines which user handler to call
///
/// # Example
///
/// ```ignore
/// impl Widget for Button {
///     fn id(&self) -> &str {
///         &self.id
///     }
///
///     fn element(&self) -> Element {
///         // Build tuidom Element
///     }
///
///     fn handle_click(&mut self, x: u16, y: u16, layout: &LayoutResult) -> WidgetResult {
///         WidgetResult::Activated
///     }
///
///     fn handle_key(&mut self, key: Key, mods: Modifiers, layout: &LayoutResult) -> WidgetResult {
///         if key == Key::Enter {
///             WidgetResult::Activated
///         } else {
///             WidgetResult::Ignored
///         }
///     }
/// }
/// ```
pub trait Widget: Send + Sync {
    /// Get the unique identifier for this widget instance.
    fn id(&self) -> &str;

    /// Check if this widget can receive keyboard focus.
    fn is_focusable(&self) -> bool {
        true
    }

    /// Check if this widget captures text input when focused.
    ///
    /// When true, character keys are sent to this widget instead of
    /// being processed as keybinds. Tab and Escape still work for navigation.
    fn captures_input(&self) -> bool {
        false
    }

    // =========================================================================
    // Event Handlers
    // =========================================================================

    /// Handle a key event when this widget is focused.
    fn handle_key(&self, key: Key, mods: Modifiers, layout: &LayoutResult) -> WidgetResult {
        let _ = (key, mods, layout);
        WidgetResult::Ignored
    }

    /// Handle a click event.
    ///
    /// Coordinates are relative to the widget's top-left corner.
    fn handle_click(&self, x: u16, y: u16, layout: &LayoutResult) -> WidgetResult {
        let _ = (x, y, layout);
        WidgetResult::Ignored
    }

    /// Handle a scroll event.
    ///
    /// Positive delta = scroll down, negative = scroll up.
    fn handle_scroll(&self, delta: i16, layout: &LayoutResult) -> WidgetResult {
        let _ = (delta, layout);
        WidgetResult::Ignored
    }

    /// Handle mouse hover.
    ///
    /// Coordinates are relative to the widget's top-left corner.
    fn handle_hover(&self, x: u16, y: u16, layout: &LayoutResult) -> WidgetResult {
        let _ = (x, y, layout);
        WidgetResult::Ignored
    }

    /// Handle drag movement.
    ///
    /// Called when mouse is dragged over this widget.
    fn handle_drag(&self, x: u16, y: u16, layout: &LayoutResult) -> WidgetResult {
        let _ = (x, y, layout);
        WidgetResult::Ignored
    }

    /// Handle mouse release.
    fn handle_release(&self, layout: &LayoutResult) -> WidgetResult {
        let _ = layout;
        WidgetResult::Ignored
    }

    /// Handle focus loss (blur).
    ///
    /// Called when this widget loses focus. Useful for closing dropdowns, etc.
    fn handle_blur(&self) {
        // Default: do nothing
    }
}
