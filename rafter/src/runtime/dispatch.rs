//! Event dispatch for the runtime.
//!
//! Handles dispatching tuidom events to the appropriate handlers:
//! 1. Global modals (capture all input when open)
//! 2. App-scoped modals (capture input for focused app)
//! 3. System keybinds
//! 4. App keybinds (with page scope support)
//! 5. Focused widget (for key events)
//! 6. Target widget (for mouse events)

use std::sync::{Arc, RwLock};

use tuidom::{Event, Key, LayoutResult, Modifiers};

use crate::instance::{AnyAppInstance, InstanceRegistry};
use crate::modal::ModalEntry;
use crate::registration::AnySystem;
use crate::{AppContext, GlobalContext, HandlerContext, Modal, WidgetResult};

// =============================================================================
// DispatchResult
// =============================================================================

/// Result of event dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchResult {
    /// Event was not handled.
    NotHandled,
    /// Event was handled by a keybind.
    HandledByKeybind,
    /// Event was handled by a modal.
    HandledByModal,
    /// Event was handled by a widget.
    HandledByWidget(WidgetResult),
}

impl DispatchResult {
    pub fn is_handled(&self) -> bool {
        !matches!(self, DispatchResult::NotHandled)
    }
}

// =============================================================================
// EventDispatcher
// =============================================================================

/// Dispatches events through the handler chain.
pub struct EventDispatcher<'a> {
    /// Global modals (type-erased).
    global_modals: &'a mut Vec<Box<dyn AnyModal>>,
    /// Systems.
    systems: &'a [Box<dyn AnySystem>],
    /// Instance registry.
    registry: &'a Arc<RwLock<InstanceRegistry>>,
    /// Global context.
    gx: &'a GlobalContext,
    /// Layout result for widget dispatch.
    layout: &'a LayoutResult,
}

impl<'a> EventDispatcher<'a> {
    /// Create a new event dispatcher.
    pub fn new(
        global_modals: &'a mut Vec<Box<dyn AnyModal>>,
        systems: &'a [Box<dyn AnySystem>],
        registry: &'a Arc<RwLock<InstanceRegistry>>,
        gx: &'a GlobalContext,
        layout: &'a LayoutResult,
    ) -> Self {
        Self {
            global_modals,
            systems,
            registry,
            gx,
            layout,
        }
    }

    /// Dispatch an event through the handler chain.
    pub fn dispatch(&mut self, event: &Event) -> DispatchResult {
        // 1. Global modals capture all input
        if let Some(result) = self.dispatch_to_global_modals(event) {
            return result;
        }

        // 2. App-scoped modals capture input for focused app
        if let Some(result) = self.dispatch_to_app_modals(event) {
            return result;
        }

        // 3. System keybinds
        if let Some(result) = self.dispatch_to_system_keybinds(event) {
            return result;
        }

        // 4. App keybinds
        if let Some(result) = self.dispatch_to_app_keybinds(event) {
            return result;
        }

        // 5. Widget dispatch
        if let Some(result) = self.dispatch_to_widgets(event) {
            return result;
        }

        DispatchResult::NotHandled
    }

    // =========================================================================
    // Modal Dispatch
    // =========================================================================

    fn dispatch_to_global_modals(&mut self, event: &Event) -> Option<DispatchResult> {
        if self.global_modals.is_empty() {
            return None;
        }

        // Get the topmost modal
        let modal = self.global_modals.last_mut()?;

        // Check if modal is closed
        if modal.is_closed() {
            self.global_modals.pop();
            return None;
        }

        // Dispatch to modal's keybinds
        if let Event::Key { key, modifiers, .. } = event {
            let keybinds = modal.keybinds();
            if let Some(handler) = keybinds.match_key(*key, *modifiers) {
                // Create a default AppContext for global modals
                let cx = AppContext::default();
                let hx = HandlerContext::for_app(&cx, self.gx);
                handler(&hx);
                return Some(DispatchResult::HandledByModal);
            }
        }

        // Modal captures all input even if not handled
        Some(DispatchResult::HandledByModal)
    }

    fn dispatch_to_app_modals(&mut self, event: &Event) -> Option<DispatchResult> {
        // TODO: Implement app-scoped modal dispatch
        // This requires tracking app-scoped modals in the registry or instance
        let _ = event;
        None
    }

    // =========================================================================
    // Keybind Dispatch
    // =========================================================================

    fn dispatch_to_system_keybinds(&self, event: &Event) -> Option<DispatchResult> {
        let Event::Key { key, modifiers, .. } = event else {
            return None;
        };

        for system in self.systems {
            let keybinds = system.keybinds();
            if let Some(handler) = keybinds.match_key(*key, *modifiers) {
                let hx = HandlerContext::for_system(self.gx);
                handler(&hx);
                return Some(DispatchResult::HandledByKeybind);
            }
        }

        None
    }

    fn dispatch_to_app_keybinds(&self, event: &Event) -> Option<DispatchResult> {
        let Event::Key { key, modifiers, .. } = event else {
            return None;
        };

        let reg = self.registry.read().ok()?;
        let instance = reg.focused_instance()?;

        // Check if focused widget captures input (e.g., text input)
        // If so, skip keybind matching for character keys
        // TODO: Check widget.captures_input() when we have widget tracking

        let keybinds = instance.keybinds();
        let current_page = instance.current_page();

        // Use page-scoped keybind matching
        if let Some(handler) = keybinds.match_key_for_page(*key, *modifiers, current_page.as_deref()) {
            let cx = AppContext::new(instance.id(), self.gx.clone(), instance.config().name);
            let hx = HandlerContext::for_app(&cx, self.gx);
            handler(&hx);
            return Some(DispatchResult::HandledByKeybind);
        }

        None
    }

    // =========================================================================
    // Widget Dispatch
    // =========================================================================

    fn dispatch_to_widgets(&self, event: &Event) -> Option<DispatchResult> {
        let reg = self.registry.read().ok()?;
        let instance = reg.focused_instance()?;
        let cx = AppContext::new(instance.id(), self.gx.clone(), instance.config().name);
        let hx = HandlerContext::for_app(&cx, self.gx);
        let handlers = instance.handlers();

        match event {
            Event::Key { key, modifiers, target } => {
                // Dispatch to focused widget (if any)
                if let Some(target_id) = target {
                    let result = dispatch_key_to_instance(instance, *key, *modifiers, self.layout);
                    if result.is_handled() {
                        dispatch_widget_result(handlers, target_id, &result, &hx);
                        return Some(DispatchResult::HandledByWidget(result));
                    }
                }
            }

            Event::Click { target, x: _, y: _, button: _ } => {
                if let Some(target_id) = target {
                    // Click is an activation - look up on_activate handler
                    if let Some(handler) = handlers.get(target_id, "on_activate") {
                        handler(&hx);
                        return Some(DispatchResult::HandledByWidget(WidgetResult::Activated));
                    }
                }
            }

            Event::Scroll { target, delta_y, .. } => {
                if let Some(target_id) = target {
                    let result = dispatch_scroll_to_instance(instance, *delta_y, self.layout);
                    if result.is_handled() {
                        dispatch_widget_result(handlers, target_id, &result, &hx);
                        return Some(DispatchResult::HandledByWidget(result));
                    }
                }
            }

            Event::Drag { target, x, y, button: _ } => {
                if let Some(target_id) = target {
                    let result = dispatch_drag_to_instance(instance, *x, *y, self.layout);
                    if result.is_handled() {
                        dispatch_widget_result(handlers, target_id, &result, &hx);
                        return Some(DispatchResult::HandledByWidget(result));
                    }
                }
            }

            Event::Release { target, .. } => {
                if let Some(target_id) = target {
                    let result = dispatch_release_to_instance(instance, self.layout);
                    if result.is_handled() {
                        dispatch_widget_result(handlers, target_id, &result, &hx);
                        return Some(DispatchResult::HandledByWidget(result));
                    }
                }
            }

            // Focus/Blur are handled by FocusState, not dispatched to widgets
            Event::Focus { .. } | Event::Blur { .. } => {}

            // MouseMove and Resize are not dispatched to widgets
            Event::MouseMove { .. } | Event::Resize { .. } => {}
        }

        None
    }
}

// =============================================================================
// Widget Dispatch Helpers
// =============================================================================

/// Dispatch a key event to an instance's focused widget.
///
/// This will be implemented by macro-generated code on the App.
/// For now, returns Ignored.
fn dispatch_key_to_instance(
    _instance: &dyn AnyAppInstance,
    _key: Key,
    _modifiers: Modifiers,
    _layout: &LayoutResult,
) -> WidgetResult {
    // TODO: Call instance.dispatch_widget_key() when macros generate it
    WidgetResult::Ignored
}

/// Dispatch a scroll event to an instance's target widget.
fn dispatch_scroll_to_instance(
    _instance: &dyn AnyAppInstance,
    _delta: i16,
    _layout: &LayoutResult,
) -> WidgetResult {
    // TODO: Call instance.dispatch_widget_scroll() when macros generate it
    WidgetResult::Ignored
}

/// Dispatch a drag event to an instance's target widget.
fn dispatch_drag_to_instance(
    _instance: &dyn AnyAppInstance,
    _x: u16,
    _y: u16,
    _layout: &LayoutResult,
) -> WidgetResult {
    // TODO: Call instance.dispatch_widget_drag() when macros generate it
    WidgetResult::Ignored
}

/// Dispatch a release event to an instance's target widget.
fn dispatch_release_to_instance(
    _instance: &dyn AnyAppInstance,
    _layout: &LayoutResult,
) -> WidgetResult {
    // TODO: Call instance.dispatch_widget_release() when macros generate it
    WidgetResult::Ignored
}

/// Map a WidgetResult to the appropriate handler dispatch.
///
/// Looks up the handler for the given element and event type, then calls it.
fn dispatch_widget_result(
    handlers: &crate::HandlerRegistry,
    element_id: &str,
    result: &WidgetResult,
    hx: &HandlerContext,
) {
    let event_type = match result {
        WidgetResult::Ignored | WidgetResult::Handled => return,
        WidgetResult::Activated => "on_activate",
        WidgetResult::Changed => "on_change",
        WidgetResult::CursorMoved => "on_cursor_moved",
        WidgetResult::Selected => "on_select",
        WidgetResult::Expanded => "on_expand",
        WidgetResult::Collapsed => "on_collapse",
        WidgetResult::Sorted => "on_sort",
        WidgetResult::Submitted => "on_submit",
    };

    if let Some(handler) = handlers.get(element_id, event_type) {
        handler(hx);
    }
}

// =============================================================================
// AnyModal trait
// =============================================================================

/// Type-erased modal for runtime storage.
pub trait AnyModal: Send + Sync {
    /// Check if the modal is closed.
    fn is_closed(&self) -> bool;
    /// Get the modal's keybinds (closure-based).
    fn keybinds(&self) -> crate::KeybindClosures;
    /// Get the handler registry for widget events.
    fn handlers(&self) -> &crate::HandlerRegistry;
}

impl<M: Modal> AnyModal for ModalEntry<M> {
    fn is_closed(&self) -> bool {
        ModalEntry::is_closed(self)
    }

    fn keybinds(&self) -> crate::KeybindClosures {
        ModalEntry::keybinds(self)
    }

    fn handlers(&self) -> &crate::HandlerRegistry {
        ModalEntry::handlers(self)
    }
}

// =============================================================================
// Convenience function for runtime
// =============================================================================

/// Dispatch an event using the full handler chain.
///
/// This is the main entry point for the runtime to dispatch events.
pub fn dispatch_event(
    event: &Event,
    global_modals: &mut Vec<Box<dyn AnyModal>>,
    systems: &[Box<dyn AnySystem>],
    registry: &Arc<RwLock<InstanceRegistry>>,
    gx: &GlobalContext,
    layout: &LayoutResult,
) -> DispatchResult {
    let mut dispatcher = EventDispatcher::new(global_modals, systems, registry, gx, layout);
    dispatcher.dispatch(event)
}
