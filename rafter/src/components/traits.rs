//! Shared traits for scrollable components.
//!
//! These traits define the common interface for components that support
//! scrolling and share the same state management patterns.

use super::scrollbar::ScrollbarState;

/// Trait for components that support scrollable content.
///
/// This trait combines identity management, dirty tracking, and scrollbar
/// functionality into a unified interface. Components implementing this
/// trait can use the shared scrollbar event handlers and rendering.
///
/// # Implementors
///
/// - `ScrollArea` - Generic scrollable container
/// - `List<T>` - Virtualized list with selection
/// - Future: `Tree<T>`, `Table<T>`
///
/// # Example
///
/// ```ignore
/// impl ScrollableComponent for MyComponent {
///     fn id_string(&self) -> String {
///         self.id.to_string()
///     }
///
///     fn is_dirty(&self) -> bool {
///         self.dirty.load(Ordering::SeqCst)
///     }
///
///     fn clear_dirty(&self) {
///         self.dirty.store(false, Ordering::SeqCst);
///     }
/// }
/// ```
pub trait ScrollableComponent: ScrollbarState {
    /// Get the unique ID as a string (for node binding).
    fn id_string(&self) -> String;

    /// Check if the component state has changed and needs re-render.
    fn is_dirty(&self) -> bool;

    /// Clear the dirty flag after rendering.
    fn clear_dirty(&self);
}
