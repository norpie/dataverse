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

use crate::handler_context::{call_handler, call_handler_for_app, EventData, HandlerCallResult};
use crate::instance::{AnyAppInstance, InstanceRegistry};
use crate::modal::ModalEntry;
use crate::registration::AnySystem;
use crate::{AppContext, GlobalContext, Handler, HandlerContext, LifecycleHooks, Modal, WidgetResult};

/// Helper to call a handler and convert panic to DispatchResult.
fn call_and_check(handler: &Handler, hx: &HandlerContext) -> Option<DispatchResult> {
    match call_handler(handler, hx) {
        HandlerCallResult::Ok => None,
        HandlerCallResult::Panicked { message } => Some(DispatchResult::HandlerPanicked { message }),
    }
}

/// Helper to call an app handler and convert panic to DispatchResult.
fn call_app_and_check(
    handler: &Handler,
    hx: &HandlerContext,
    app_name: &str,
    instance_id: crate::InstanceId,
) -> Option<DispatchResult> {
    match call_handler_for_app(handler, hx, app_name, instance_id) {
        HandlerCallResult::Ok => None,
        HandlerCallResult::Panicked { message } => Some(DispatchResult::HandlerPanicked { message }),
    }
}

// =============================================================================
// DispatchResult
// =============================================================================

/// Result of event dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchResult {
    /// Event was not handled.
    NotHandled,
    /// Event was handled by a keybind.
    HandledByKeybind,
    /// Event was handled by a modal.
    HandledByModal,
    /// Event was handled by a widget.
    HandledByWidget(WidgetResult),
    /// Handler panicked.
    HandlerPanicked {
        /// The panic message.
        message: String,
    },
}

impl DispatchResult {
    pub fn is_handled(&self) -> bool {
        !matches!(self, DispatchResult::NotHandled)
    }

    pub fn is_panic(&self) -> bool {
        matches!(self, DispatchResult::HandlerPanicked { .. })
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

    /// Dispatch an event to a modal's widgets.
    ///
    /// This handles keybinds, clicks, text input, and other widget events for modals.
    /// Returns Some(DispatchResult) if the modal captures the event (which it always does).
    fn dispatch_event_to_modal(
        &self,
        modal: &dyn AnyModal,
        event: &Event,
        cx: &AppContext,
        mx: &(dyn std::any::Any + Send + Sync),
        app_info: Option<(&str, crate::InstanceId)>,
    ) -> Option<DispatchResult> {
        let handlers = modal.handlers();
        let keybinds = modal.keybinds();

        // Helper to call handler with optional app info for error reporting
        let call = |handler: &Handler, hx: &HandlerContext| -> Option<DispatchResult> {
            if let Some((app_name, instance_id)) = app_info {
                call_app_and_check(handler, hx, app_name, instance_id)
            } else {
                call_and_check(handler, hx)
            }
        };

        match event {
            Event::Key { key, modifiers, target } => {
                // First try keybinds
                if let Some(handler) = keybinds.match_key(*key, *modifiers) {
                    let hx = HandlerContext::for_modal_any(cx, self.gx, mx);
                    if let Some(panic_result) = call(&handler, &hx) {
                        return Some(panic_result);
                    }
                    return Some(DispatchResult::HandledByModal);
                }

                // Enter key triggers on_activate on focused widget
                if *key == Key::Enter {
                    if let Some(target_id) = target {
                        if let Some(handler) = handlers.get(target_id, "on_activate") {
                            let hx = HandlerContext::for_modal_any(cx, self.gx, mx);
                            if let Some(panic_result) = call(&handler, &hx) {
                                return Some(panic_result);
                            }
                            return Some(DispatchResult::HandledByModal);
                        }
                    }
                }
            }

            Event::Click { target, x, y, .. } => {
                if let Some(target_id) = target {
                    if let Some(handler) = handlers.get(target_id, "on_activate") {
                        let hx = HandlerContext::for_modal_any_with_event(
                            cx,
                            self.gx,
                            mx,
                            EventData::Click { x: *x, y: *y },
                        );
                        if let Some(panic_result) = call(&handler, &hx) {
                            return Some(panic_result);
                        }
                        return Some(DispatchResult::HandledByModal);
                    }
                }
            }

            Event::Change { target, text } => {
                if let Some(handler) = handlers.get(target, "on_change") {
                    let hx = HandlerContext::for_modal_any_with_event(
                        cx,
                        self.gx,
                        mx,
                        EventData::Change { text: text.clone() },
                    );
                    if let Some(panic_result) = call(&handler, &hx) {
                        return Some(panic_result);
                    }
                    return Some(DispatchResult::HandledByModal);
                }
            }

            Event::Submit { target } => {
                if let Some(handler) = handlers.get(target, "on_submit") {
                    let hx = HandlerContext::for_modal_any_with_event(
                        cx,
                        self.gx,
                        mx,
                        EventData::Submit,
                    );
                    if let Some(panic_result) = call(&handler, &hx) {
                        return Some(panic_result);
                    }
                    return Some(DispatchResult::HandledByModal);
                }
            }

            Event::Focus { target } => {
                if let Some(handler) = handlers.get(target, "on_focus") {
                    let hx = HandlerContext::for_modal_any_with_event(
                        cx,
                        self.gx,
                        mx,
                        EventData::Focus,
                    );
                    if let Some(panic_result) = call(&handler, &hx) {
                        return Some(panic_result);
                    }
                }
            }

            Event::Blur { target, new_target } => {
                if let Some(handler) = handlers.get(target, "on_blur") {
                    let hx = HandlerContext::for_modal_any_with_event(
                        cx,
                        self.gx,
                        mx,
                        EventData::Blur { new_target: new_target.clone() },
                    );
                    if let Some(panic_result) = call(&handler, &hx) {
                        return Some(panic_result);
                    }
                }
            }

            // Other events are captured but not dispatched to widgets
            _ => {}
        }

        // Modal captures all input even if no handler matched
        Some(DispatchResult::HandledByModal)
    }

    fn dispatch_to_global_modals(&mut self, event: &Event) -> Option<DispatchResult> {
        if self.global_modals.is_empty() {
            return None;
        }

        // Get the topmost modal
        let modal = self.global_modals.last()?;

        // Check if modal is closed
        if modal.is_closed() {
            self.global_modals.pop();
            return None;
        }

        // Create a default AppContext for global modals
        let cx = AppContext::default();
        let mx = modal.modal_context();

        self.dispatch_event_to_modal(modal.as_ref(), event, &cx, mx, None)
    }

    fn dispatch_to_app_modals(&mut self, event: &Event) -> Option<DispatchResult> {
        let reg = self.registry.read().ok()?;
        let instance = reg.focused_instance()?;

        let mut modals = instance.modals().write().ok()?;
        log::debug!("dispatch_to_app_modals: modals_count={}", modals.len());

        // Pop any closed modals first
        while modals.last().map(|m| m.is_closed()).unwrap_or(false) {
            log::debug!("dispatch_to_app_modals: popping closed modal");
            modals.pop();
        }

        if modals.is_empty() {
            return None;
        }

        // Get the topmost modal
        let modal = modals.last()?;
        let cx = instance.app_context();
        let mx = modal.modal_context();
        let app_info = Some((instance.config().name, instance.id()));

        self.dispatch_event_to_modal(modal.as_ref(), event, &cx, mx, app_info)
    }

    // =========================================================================
    // Keybind Dispatch
    // =========================================================================

    fn dispatch_to_system_keybinds(&self, event: &Event) -> Option<DispatchResult> {
        let Event::Key { key, modifiers, .. } = event else {
            return None;
        };

        log::debug!(
            "dispatch_to_system_keybinds: key={:?}, modifiers={:?}",
            key, modifiers
        );

        for system in self.systems {
            let keybinds = system.keybinds();
            log::debug!(
                "dispatch_to_system_keybinds: checking system keybinds, count={}",
                keybinds.len()
            );
            if let Some(handler) = keybinds.match_key(*key, *modifiers) {
                let hx = HandlerContext::for_system(self.gx);
                if let Some(panic_result) = call_and_check(&handler, &hx) {
                    return Some(panic_result);
                }
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

        log::debug!(
            "dispatch_to_app_keybinds: key={:?}, modifiers={:?}, keybinds_count={}",
            key,
            modifiers,
            keybinds.len()
        );

        // Use page-scoped keybind matching
        if let Some(handler) = keybinds.match_key_for_page(*key, *modifiers, current_page.as_deref()) {
            log::debug!("dispatch_to_app_keybinds: matched keybind, calling handler");
            let cx = instance.app_context();
            let hx = HandlerContext::for_app(&cx, self.gx);
            if let Some(panic_result) = call_app_and_check(&handler, &hx, instance.config().name, instance.id()) {
                return Some(panic_result);
            }
            log::debug!("dispatch_to_app_keybinds: handler returned");
            return Some(DispatchResult::HandledByKeybind);
        }

        log::debug!("dispatch_to_app_keybinds: no keybind matched");
        None
    }

    // =========================================================================
    // Widget Dispatch
    // =========================================================================

    fn dispatch_to_widgets(&self, event: &Event) -> Option<DispatchResult> {
        let reg = self.registry.read().ok()?;
        let instance = reg.focused_instance()?;
        let cx = instance.app_context();
        let hx = HandlerContext::for_app(&cx, self.gx);
        let handlers = instance.handlers();

        match event {
            Event::Key { key, modifiers, target } => {
                // Enter key triggers on_activate on focused element (like click)
                if *key == tuidom::Key::Enter && modifiers.none() {
                    if let Some(target_id) = target {
                        // First check app instance handlers
                        if let Some(handler) = handlers.get(target_id, "on_activate") {
                            if let Some(panic_result) = call_app_and_check(&handler, &hx, instance.config().name, instance.id()) {
                                return Some(panic_result);
                            }
                            return Some(DispatchResult::HandledByWidget(WidgetResult::Activated));
                        }

                        // Then check system handlers
                        for system in self.systems {
                            let system_handlers = system.handlers();
                            if let Some(handler) = system_handlers.get(target_id, "on_activate") {
                                let system_hx = HandlerContext::for_system(self.gx);
                                if let Some(panic_result) = call_and_check(&handler, &system_hx) {
                                    return Some(panic_result);
                                }
                                return Some(DispatchResult::HandledByWidget(WidgetResult::Activated));
                            }
                        }
                    }
                }

                // Arrow keys dispatch to on_key_up/down/left/right handlers
                // (Used by List/Tree widgets for boundary scrolling and expand/collapse)
                if modifiers.none() {
                    if let Some(target_id) = target {
                        let handler_name = match key {
                            tuidom::Key::Up => Some("on_key_up"),
                            tuidom::Key::Down => Some("on_key_down"),
                            tuidom::Key::Left => Some("on_key_left"),
                            tuidom::Key::Right => Some("on_key_right"),
                            _ => None,
                        };
                        if let Some(name) = handler_name {
                            log::debug!("dispatch_to_widgets: looking for {} handler on {}", name, target_id);
                            if let Some(handler) = handlers.get(target_id, name) {
                                log::debug!("dispatch_to_widgets: found {} handler, calling", name);
                                if let Some(panic_result) = call_app_and_check(&handler, &hx, instance.config().name, instance.id()) {
                                    return Some(panic_result);
                                }
                                // For Left/Right, consume the event (don't let focus nav continue)
                                // For Up/Down, let focus navigation continue after handler
                                if matches!(key, tuidom::Key::Left | tuidom::Key::Right) {
                                    return Some(DispatchResult::HandledByWidget(WidgetResult::Handled));
                                }
                            } else {
                                log::debug!("dispatch_to_widgets: no {} handler found for {}", name, target_id);
                            }
                        }
                    }
                }

                // Dispatch to focused widget (if any)
                if let Some(target_id) = target {
                    let result = dispatch_key_to_instance(instance, *key, *modifiers, self.layout);
                    if result.is_handled() {
                        if let Some(panic_result) = dispatch_widget_result(handlers, target_id, &result, &hx, instance.config().name, instance.id()) {
                            return Some(panic_result);
                        }
                        return Some(DispatchResult::HandledByWidget(result));
                    }
                }
            }

            Event::Click { target, x, y, button: _ } => {
                log::debug!("dispatch_to_widgets: Click event, target={:?}, pos=({}, {})", target, x, y);
                if let Some(target_id) = target {
                    // Create handler context with click position
                    let hx_with_click = HandlerContext::for_app_with_event(
                        &cx,
                        self.gx,
                        EventData::Click { x: *x, y: *y },
                    );

                    // Click is an activation - look up on_activate handler
                    log::debug!(
                        "dispatch_to_widgets: looking up handler for target={}, event=on_activate",
                        target_id
                    );

                    // First check app instance handlers
                    if let Some(handler) = handlers.get(target_id, "on_activate") {
                        log::debug!("dispatch_to_widgets: found app handler, calling");
                        if let Some(panic_result) = call_app_and_check(&handler, &hx_with_click, instance.config().name, instance.id()) {
                            return Some(panic_result);
                        }
                        log::debug!("dispatch_to_widgets: handler returned");
                        return Some(DispatchResult::HandledByWidget(WidgetResult::Activated));
                    }

                    // Then check system handlers (for overlay buttons)
                    for system in self.systems {
                        let system_handlers = system.handlers();
                        if let Some(handler) = system_handlers.get(target_id, "on_activate") {
                            log::debug!("dispatch_to_widgets: found system handler for {}, calling", system.name());
                            let system_hx = HandlerContext::for_system(self.gx);
                            if let Some(panic_result) = call_and_check(&handler, &system_hx) {
                                return Some(panic_result);
                            }
                            log::debug!("dispatch_to_widgets: system handler returned");
                            return Some(DispatchResult::HandledByWidget(WidgetResult::Activated));
                        }
                    }

                    log::debug!("dispatch_to_widgets: no handler found");
                }
            }

            Event::Scroll { target, delta_x, delta_y, action, .. } => {
                if let Some(target_id) = target {
                    // Check for on_scroll handler on the scrollable element
                    if let Some(handler) = handlers.get(target_id, "on_scroll") {
                        let hx_with_event = HandlerContext::for_app_with_event(
                            &cx,
                            self.gx,
                            EventData::ScrollInput {
                                delta_x: *delta_x,
                                delta_y: *delta_y,
                                action: *action,
                            },
                        );
                        if let Some(panic_result) = call_app_and_check(&handler, &hx_with_event, instance.config().name, instance.id()) {
                            return Some(panic_result);
                        }
                        return Some(DispatchResult::HandledByWidget(WidgetResult::Handled));
                    }
                }
                // No handler - fall through to automatic scroll handling in runtime
            }

            Event::Drag { target, x, y, button: _ } => {
                log::debug!("dispatch_to_widgets: Drag event, target={:?}, pos=({}, {})", target, x, y);
                if let Some(target_id) = target {
                    // Check for on_drag handler
                    if let Some(handler) = handlers.get(target_id, "on_drag") {
                        let hx_with_drag = HandlerContext::for_app_with_event(
                            &cx,
                            self.gx,
                            EventData::Drag { x: *x, y: *y },
                        );
                        log::debug!("dispatch_to_widgets: found on_drag handler, calling");
                        if let Some(panic_result) = call_app_and_check(&handler, &hx_with_drag, instance.config().name, instance.id()) {
                            return Some(panic_result);
                        }
                        return Some(DispatchResult::HandledByWidget(WidgetResult::Handled));
                    }

                    // Fall back to instance dispatch
                    let result = dispatch_drag_to_instance(instance, *x, *y, self.layout);
                    if result.is_handled() {
                        if let Some(panic_result) = dispatch_widget_result(handlers, target_id, &result, &hx, instance.config().name, instance.id()) {
                            return Some(panic_result);
                        }
                        return Some(DispatchResult::HandledByWidget(result));
                    }
                }
            }

            Event::Release { target, x, y, button: _ } => {
                log::debug!("dispatch_to_widgets: Release event, target={:?}, pos=({}, {})", target, x, y);
                if let Some(target_id) = target {
                    // Check for on_release handler
                    if let Some(handler) = handlers.get(target_id, "on_release") {
                        log::debug!("dispatch_to_widgets: found on_release handler, calling");
                        if let Some(panic_result) = call_app_and_check(&handler, &hx, instance.config().name, instance.id()) {
                            return Some(panic_result);
                        }
                        return Some(DispatchResult::HandledByWidget(WidgetResult::Handled));
                    }

                    // Fall back to instance dispatch
                    let result = dispatch_release_to_instance(instance, self.layout);
                    if result.is_handled() {
                        if let Some(panic_result) = dispatch_widget_result(handlers, target_id, &result, &hx, instance.config().name, instance.id()) {
                            return Some(panic_result);
                        }
                        return Some(DispatchResult::HandledByWidget(result));
                    }
                }
            }

            // Focus events: dispatch to on_focus handlers
            Event::Focus { target } => {
                log::debug!("dispatch_to_widgets: Focus event, target={}", target);
                // First check app instance handlers
                if let Some(handler) = handlers.get(target, "on_focus") {
                    log::debug!("dispatch_to_widgets: found app on_focus handler for {}", target);
                    let hx_with_event = HandlerContext::for_app_with_event(
                        &cx,
                        self.gx,
                        EventData::Focus,
                    );
                    if let Some(panic_result) = call_app_and_check(&handler, &hx_with_event, instance.config().name, instance.id()) {
                        return Some(panic_result);
                    }
                } else {
                    // Then check system handlers
                    for system in self.systems {
                        let system_handlers = system.handlers();
                        if let Some(handler) = system_handlers.get(target, "on_focus") {
                            log::debug!("dispatch_to_widgets: found system on_focus handler for {} in {}", target, system.name());
                            let system_hx = HandlerContext::for_system_with_event(self.gx, EventData::Focus);
                            if let Some(panic_result) = call_and_check(&handler, &system_hx) {
                                return Some(panic_result);
                            }
                            break;
                        }
                    }
                    log::debug!("dispatch_to_widgets: no on_focus handler found for {}", target);
                }
            }

            // Blur events: dispatch to on_blur handlers with new_target info
            Event::Blur { target, new_target } => {
                // First check app instance handlers
                if let Some(handler) = handlers.get(target, "on_blur") {
                    let hx_with_event = HandlerContext::for_app_with_event(
                        &cx,
                        self.gx,
                        EventData::Blur {
                            new_target: new_target.clone(),
                        },
                    );
                    if let Some(panic_result) = call_app_and_check(&handler, &hx_with_event, instance.config().name, instance.id()) {
                        return Some(panic_result);
                    }
                } else {
                    // Then check system handlers
                    for system in self.systems {
                        let system_handlers = system.handlers();
                        if let Some(handler) = system_handlers.get(target, "on_blur") {
                            let system_hx = HandlerContext::for_system_with_event(
                                self.gx,
                                EventData::Blur {
                                    new_target: new_target.clone(),
                                },
                            );
                            if let Some(panic_result) = call_and_check(&handler, &system_hx) {
                                return Some(panic_result);
                            }
                            break;
                        }
                    }
                }
            }

            // MouseMove and Resize are not dispatched to widgets
            Event::MouseMove { .. } | Event::Resize { .. } => {}

            // Change events: pass text via EventData to handler
            Event::Change { target, text } => {
                if let Some(handler) = handlers.get(target, "on_change") {
                    // Create HandlerContext with event data containing the new text
                    let hx_with_event = HandlerContext::for_app_with_event(
                        &cx,
                        self.gx,
                        EventData::Change {
                            text: text.clone(),
                        },
                    );
                    if let Some(panic_result) = call_app_and_check(&handler, &hx_with_event, instance.config().name, instance.id()) {
                        return Some(panic_result);
                    }
                    return Some(DispatchResult::HandledByWidget(WidgetResult::Changed));
                }
            }

            // Submit events: pass EventData::Submit to handler
            Event::Submit { target } => {
                if let Some(handler) = handlers.get(target, "on_submit") {
                    let hx_with_event = HandlerContext::for_app_with_event(
                        &cx,
                        self.gx,
                        EventData::Submit,
                    );
                    if let Some(panic_result) = call_app_and_check(&handler, &hx_with_event, instance.config().name, instance.id()) {
                        return Some(panic_result);
                    }
                    return Some(DispatchResult::HandledByWidget(WidgetResult::Submitted));
                }
            }

            // ScopeClick events: click on interaction_scope backdrop
            Event::ScopeClick { target, x, y } => {
                if let Some(handler) = handlers.get(target, "on_scope_click") {
                    let hx_with_event = HandlerContext::for_app_with_event(
                        &cx,
                        self.gx,
                        EventData::Click { x: *x, y: *y },
                    );
                    if let Some(panic_result) = call_app_and_check(&handler, &hx_with_event, instance.config().name, instance.id()) {
                        return Some(panic_result);
                    }
                    return Some(DispatchResult::HandledByWidget(WidgetResult::Handled));
                }
            }
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
/// Returns Some(DispatchResult) if the handler panicked.
fn dispatch_widget_result(
    handlers: &crate::HandlerRegistry,
    element_id: &str,
    result: &WidgetResult,
    hx: &HandlerContext,
    app_name: &str,
    instance_id: crate::InstanceId,
) -> Option<DispatchResult> {
    let event_type = match result {
        WidgetResult::Ignored | WidgetResult::Handled => return None,
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
        return call_app_and_check(&handler, hx, app_name, instance_id);
    }
    None
}

// =============================================================================
// AnyModal trait
// =============================================================================

/// Type-erased modal for runtime storage.
pub trait AnyModal: Send + Sync {
    /// Check if the modal is closed.
    fn is_closed(&self) -> bool;
    /// Get the modal's kind (App or System).
    fn kind(&self) -> crate::ModalKind;
    /// Get lifecycle hook closures.
    fn lifecycle_hooks(&self) -> LifecycleHooks;
    /// Get the modal's keybinds (closure-based).
    fn keybinds(&self) -> crate::KeybindClosures;
    /// Get the handler registry for widget events.
    fn handlers(&self) -> &crate::HandlerRegistry;
    /// Get the modal's element for rendering.
    fn element(&self) -> tuidom::Element;
    /// Get the modal context as a type-erased reference.
    fn modal_context(&self) -> &(dyn std::any::Any + Send + Sync);
    /// Get the modal's size configuration.
    fn size(&self) -> crate::ModalSize;
    /// Get the modal's position configuration.
    fn position(&self) -> crate::ModalPosition;
}

impl<M: Modal> AnyModal for ModalEntry<M> {
    fn is_closed(&self) -> bool {
        ModalEntry::is_closed(self)
    }

    fn kind(&self) -> crate::ModalKind {
        self.modal.kind()
    }

    fn lifecycle_hooks(&self) -> LifecycleHooks {
        self.modal.lifecycle_hooks()
    }

    fn keybinds(&self) -> crate::KeybindClosures {
        ModalEntry::keybinds(self)
    }

    fn handlers(&self) -> &crate::HandlerRegistry {
        ModalEntry::handlers(self)
    }

    fn element(&self) -> tuidom::Element {
        ModalEntry::element(self)
    }

    fn modal_context(&self) -> &(dyn std::any::Any + Send + Sync) {
        ModalEntry::context(self)
    }

    fn size(&self) -> crate::ModalSize {
        self.modal.size()
    }

    fn position(&self) -> crate::ModalPosition {
        self.modal.position()
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
