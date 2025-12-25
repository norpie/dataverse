//! Event handlers extracted from the main event loop.
//!
//! This module contains the event handling logic, extracted from `event_loop.rs`
//! for maintainability. Each handler function processes a specific event type.

use std::ops::ControlFlow;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use log::{debug, warn};

use crate::app::AnyAppInstance;
use crate::context::AppContext;
use crate::input::events::{ClickEvent, Position, ScrollEvent};
use crate::input::keybinds::{HandlerId, Key, KeyCombo, Keybinds};
use crate::node::Node;
use crate::widgets::EventResult;
use crate::widgets::events::WidgetEventKind;

use super::events::{DragEvent, Event};
use super::hit_test::HitTestMap;
use super::input::KeybindMatch;
use super::modal::ModalStackEntry;
use super::state::EventLoopState;

// =============================================================================
// Focus Change Helpers (with blur dispatch)
// =============================================================================

/// Set focus to a new element, dispatching blur to the old focused widget.
fn set_focus_with_blur(state: &mut EventLoopState, page: &Node, cx: &AppContext, new_id: String) {
    let old_id = state.focused_id();
    if old_id.as_deref() != Some(&new_id) {
        // Dispatch blur to old widget
        if let Some(ref old) = old_id {
            page.dispatch_blur(old, cx);
        }
    }
    state.focus_state_mut().set_focus(new_id);
}

/// Focus next element, dispatching blur to the old focused widget.
fn focus_next_with_blur(state: &mut EventLoopState, page: &Node, cx: &AppContext) {
    let old_id = state.focused_id();
    state.focus_state_mut().focus_next();
    let new_id = state.focused_id();
    if old_id != new_id
        && let Some(ref old) = old_id {
            page.dispatch_blur(old, cx);
        }
}

/// Focus previous element, dispatching blur to the old focused widget.
fn focus_prev_with_blur(state: &mut EventLoopState, page: &Node, cx: &AppContext) {
    let old_id = state.focused_id();
    state.focus_state_mut().focus_prev();
    let new_id = state.focused_id();
    if old_id != new_id
        && let Some(ref old) = old_id {
            page.dispatch_blur(old, cx);
        }
}

/// Clear focus, dispatching blur to the old focused widget.
fn clear_focus_with_blur(state: &mut EventLoopState, page: &Node, cx: &AppContext) {
    if let Some(ref old_id) = state.focused_id() {
        page.dispatch_blur(old_id, cx);
    }
    state.focus_state_mut().clear_focus();
}

// =============================================================================
// Handler Dispatch Helpers
// =============================================================================

/// Dispatch a handler to the appropriate layer (modal or app instance).
pub fn dispatch_to_layer(
    instance: &dyn AnyAppInstance,
    modal_stack: &[ModalStackEntry],
    handler_id: &HandlerId,
    cx: &AppContext,
) {
    if let Some(entry) = modal_stack.last() {
        entry.modal.dispatch_dyn(handler_id, cx);
    } else {
        instance.dispatch(handler_id, cx);
    }
}

/// Dispatch handlers based on context data set by widgets.
///
/// Components push events to the event queue via `AppContext::push_event()`.
/// This function drains the queue and dispatches appropriate handlers based
/// on event kind and the widget ID that triggered it.
pub fn dispatch_component_handlers(
    page: &Node,
    _focus_id: &str,
    instance: &dyn AnyAppInstance,
    modal_stack: &[ModalStackEntry],
    cx: &AppContext,
) {
    // Process the unified event queue
    for event in cx.drain_events() {
        let handler = match event.kind {
            WidgetEventKind::Activate => page.get_submit_handler(&event.widget_id),
            WidgetEventKind::CursorMove => page.get_cursor_handler(&event.widget_id),
            WidgetEventKind::SelectionChange => page.get_selection_handler(&event.widget_id),
            WidgetEventKind::Expand => page.get_expand_handler(&event.widget_id),
            WidgetEventKind::Collapse => page.get_collapse_handler(&event.widget_id),
            WidgetEventKind::Sort => page.get_sort_handler(&event.widget_id),
            WidgetEventKind::Change => page.get_change_handler(&event.widget_id),
        };

        if let Some(handler_id) = handler {
            dispatch_to_layer(instance, modal_stack, &handler_id, cx);
        }
    }

    // Clear the unified fields after processing
    cx.clear_activated();
    cx.clear_cursor();
    cx.clear_selected();
    cx.clear_expanded();
    cx.clear_collapsed();
    cx.clear_sorted();
}

// =============================================================================
// Key Event Handler
// =============================================================================

/// Handle a key event.
///
/// Returns `ControlFlow::Break(())` if the app should exit, `ControlFlow::Continue(true)`
/// if the event was handled and the loop should continue to the next iteration,
/// or `ControlFlow::Continue(false)` if normal processing should continue.
#[allow(clippy::too_many_arguments)]
pub fn handle_key_event(
    key_combo: &KeyCombo,
    page: &Node,
    instance: &dyn AnyAppInstance,
    state: &mut EventLoopState,
    app_keybinds: &Arc<RwLock<Keybinds>>,
    cx: &AppContext,
) -> ControlFlow<(), bool> {
    // Handle Tab/Shift+Tab for focus navigation
    if key_combo.key == Key::Tab {
        if key_combo.modifiers.shift {
            debug!("Focus prev");
            focus_prev_with_blur(state, page, cx);
        } else {
            debug!("Focus next");
            focus_next_with_blur(state, page, cx);
        }
        return ControlFlow::Continue(true);
    }

    // Get current focus
    let current_focus = state.focused_id();

    // Dispatch to focused widget
    if let Some(ref focus_id) = current_focus {
        // Handle Enter key - triggers submit handler
        if key_combo.key == Key::Enter {
            debug!("Enter on focused element: {:?}", focus_id);

            // Dispatch to widget first (widgets push events to context)
            if let Some(result) = page.dispatch_key_event(focus_id, key_combo, cx)
                && result.is_handled()
            {
                // Dispatch handlers based on context data (includes Change events)
                dispatch_component_handlers(page, focus_id, instance, &state.modal_stack, cx);
                return ControlFlow::Continue(true);
            }

            // Fallback for buttons etc - dispatch submit handler directly
            if let Some(handler_id) = page.get_submit_handler(focus_id) {
                dispatch_to_layer(instance, &state.modal_stack, &handler_id, cx);
            }
            return ControlFlow::Continue(true);
        }

        // For all other keys, dispatch to widget
        if let Some(result) = page.dispatch_key_event(focus_id, key_combo, cx)
            && result.is_handled()
        {
            // Dispatch handlers based on context data (includes Change events)
            dispatch_component_handlers(page, focus_id, instance, &state.modal_stack, cx);
            return ControlFlow::Continue(true);
        }
    }

    // Handle Escape to clear focus (if not handled by widget)
    if key_combo.key == Key::Escape {
        debug!("Escape pressed, clearing focus");
        clear_focus_with_blur(state, page, cx);
        return ControlFlow::Continue(true);
    }

    // Check system keybinds FIRST (highest priority)
    // Systems are global and not affected by modals or page scope
    let keybind_start = Instant::now();
    let system_match = state
        .system_input_state
        .process_key(key_combo.clone(), &state.system_keybinds, None);
    let system_keybind_elapsed = keybind_start.elapsed();
    if system_keybind_elapsed.as_micros() > 500 {
        warn!("PROFILE: system keybind matching took {:?}", system_keybind_elapsed);
    }

    if let KeybindMatch::Match(handler_id) = system_match {
        log::info!("System keybind matched: {:?}", handler_id);
        // Find and dispatch to the appropriate system
        for system in &state.systems {
            // Check if this system's keybinds contain this handler
            if system.keybinds().all().iter().any(|b| b.handler == handler_id) {
                system.dispatch(&handler_id, cx);
                return ControlFlow::Continue(true);
            }
        }
        // Fallback: try dispatching to all systems
        for system in &state.systems {
            system.dispatch(&handler_id, cx);
        }
        return ControlFlow::Continue(true);
    }

    // Process app/modal keybind (only if not handled by system)
    let current_page = if state.modal_stack.is_empty() {
        instance.current_page()
    } else {
        None
    };
    let current_page_ref = current_page.as_deref();

    let app_keybind_start = Instant::now();
    let keybind_match = if let Some(entry) = state.modal_stack.last_mut() {
        entry
            .input_state
            .process_key(key_combo.clone(), &entry.keybinds, None)
    } else {
        let kb = app_keybinds.read().unwrap();
        state
            .app_input_state
            .process_key(key_combo.clone(), &kb, current_page_ref)
    };
    let app_keybind_elapsed = app_keybind_start.elapsed();
    if app_keybind_elapsed.as_micros() > 500 {
        warn!("PROFILE: app keybind matching took {:?}", app_keybind_elapsed);
    }

    match keybind_match {
        KeybindMatch::Match(handler_id) => {
            log::info!("Keybind matched: {:?}", handler_id);
            dispatch_to_layer(instance, &state.modal_stack, &handler_id, cx);
        }
        KeybindMatch::Pending => {
            debug!("Keybind pending (sequence in progress)");
        }
        KeybindMatch::NoMatch => {
            debug!("No keybind matched for key");
        }
    }

    ControlFlow::Continue(false)
}

// =============================================================================
// Click Event Handler
// =============================================================================

/// Handle a click event.
///
/// This function uses the unified widget dispatch to handle clicks on all widgets.
///
/// Returns `true` if the loop should continue to the next iteration.
pub fn handle_click_event(
    click: &ClickEvent,
    page: &Node,
    hit_map: &HitTestMap,
    instance: &dyn AnyAppInstance,
    state: &mut EventLoopState,
    cx: &AppContext,
) -> bool {
    // First check if click is on an active overlay - route to overlay owner
    for overlay in &state.active_overlays {
        if overlay.contains(click.position.x, click.position.y) {
            debug!("Click on overlay owned by: {}", overlay.owner_id);

            // Calculate position relative to overlay area
            let x_rel = click.position.x.saturating_sub(overlay.area.x);
            let y_rel = click.position.y.saturating_sub(overlay.area.y);

            // Dispatch overlay click to owner widget
            if let Some(result) = page.dispatch_overlay_click(&overlay.owner_id, x_rel, y_rel, cx)
                && result.is_handled() {
                    dispatch_component_handlers(
                        page,
                        &overlay.owner_id,
                        instance,
                        &state.modal_stack,
                        cx,
                    );
                    return true;
                }

            // If overlay didn't handle it, don't propagate to widgets below
            return true;
        }
    }

    let Some(hit_box) = hit_map.hit_test(click.position.x, click.position.y) else {
        return false;
    };

    // Focus the clicked element (with blur dispatch to old element)
    set_focus_with_blur(state, page, cx, hit_box.id.clone());

    // Calculate viewport-relative coordinates once at the start
    let x_rel = click.position.x.saturating_sub(hit_box.rect.x);
    let y_rel = click.position.y.saturating_sub(hit_box.rect.y);

    // Check if this is a selectable widget (List/Tree/Table) via capability query
    if let Some(widget) = page.get_widget(&hit_box.id)
        && let Some(selectable) = widget.as_selectable()
    {
        debug!("Clicked on selectable element: {}", hit_box.id);

        // First check if click is on scrollbar (dispatch_click_event handles this)
        if let Some(result) = page.dispatch_click_event(&hit_box.id, x_rel, y_rel, cx) {
            match result {
                EventResult::StartDrag => {
                    state.drag_widget_id = Some(hit_box.id.clone());
                    return true;
                }
                EventResult::Consumed => {
                    dispatch_component_handlers(
                        page,
                        &hit_box.id,
                        instance,
                        &state.modal_stack,
                        cx,
                    );
                    return true;
                }
                EventResult::Ignored => {}
            }
        }

        // Check for header click (Table only has headers)
        if selectable.has_header() && y_rel == 0 {
            debug!("Header click at x={}", x_rel);
            selectable.on_header_click(x_rel, cx);
        } else {
            // Data row click - handle with modifiers
            let ctrl = click.modifiers.ctrl;
            let shift = click.modifiers.shift;
            selectable.on_click_with_modifiers(y_rel, ctrl, shift, cx);
        }

        // Dispatch handlers based on context data
        dispatch_component_handlers(page, &hit_box.id, instance, &state.modal_stack, cx);
        return true;
    }

    // Not a selectable widget - dispatch click to widget (coordinates already relative)
    if let Some(result) = page.dispatch_click_event(&hit_box.id, x_rel, y_rel, cx) {
        match result {
            EventResult::StartDrag => {
                state.drag_widget_id = Some(hit_box.id.clone());
                return true;
            }
            EventResult::Consumed => {
                // Widgets push Change events when their state changes
                dispatch_component_handlers(page, &hit_box.id, instance, &state.modal_stack, cx);
                return true;
            }
            EventResult::Ignored => {}
        }
    }

    debug!("Clicked element: {}", hit_box.id);

    // If it's a button (non-capturing widget), dispatch submit handler
    if !hit_box.captures_input
        && let Some(handler_id) = page.get_submit_handler(&hit_box.id) {
            dispatch_to_layer(instance, &state.modal_stack, &handler_id, cx);
        }

    false
}

// =============================================================================
// Hover Event Handler
// =============================================================================

/// Handle a hover event.
///
/// Returns `true` if the loop should continue to the next iteration.
pub fn handle_hover_event(
    position: Position,
    page: &Node,
    hit_map: &HitTestMap,
    instance: &dyn AnyAppInstance,
    state: &mut EventLoopState,
    cx: &AppContext,
) -> bool {
    // First check if hover is on an active overlay - route to overlay owner
    for overlay in &state.active_overlays {
        if overlay.contains(position.x, position.y) {
            // Calculate position relative to overlay area
            let x_rel = position.x.saturating_sub(overlay.area.x);
            let y_rel = position.y.saturating_sub(overlay.area.y);

            // Dispatch overlay hover to owner widget
            if let Some(result) = page.dispatch_overlay_hover(&overlay.owner_id, x_rel, y_rel, cx)
                && result.is_handled() {
                    dispatch_component_handlers(
                        page,
                        &overlay.owner_id,
                        instance,
                        &state.modal_stack,
                        cx,
                    );
                }

            // Don't propagate to widgets below overlay
            return false;
        }
    }

    let Some(hit_box) = hit_map.hit_test(position.x, position.y) else {
        return false;
    };

    // Focus if not already focused (with blur dispatch to old element)
    let current_focus = state.focused_id();
    if current_focus.as_deref() != Some(&hit_box.id) {
        debug!("Hover focus: {}", hit_box.id);
        set_focus_with_blur(state, page, cx, hit_box.id.clone());
    }

    // Dispatch hover event to widget (lists use this to move cursor)
    if let Some(result) = page.dispatch_hover_event(
        &hit_box.id,
        position.x,
        position.y.saturating_sub(hit_box.rect.y),
        cx,
    ) && result.is_handled()
    {
        dispatch_component_handlers(page, &hit_box.id, instance, &state.modal_stack, cx);
    }

    false
}

// =============================================================================
// Scroll Event Handler
// =============================================================================

/// Handle a scroll event.
pub fn handle_scroll_event(
    scroll: &ScrollEvent,
    page: &Node,
    hit_map: &HitTestMap,
    instance: &dyn AnyAppInstance,
    state: &EventLoopState,
    cx: &AppContext,
) {
    // First check if scroll is on an active overlay - route to overlay owner
    for overlay in &state.active_overlays {
        if overlay.contains(scroll.position.x, scroll.position.y) {
            debug!("Scroll on overlay owned by: {}", overlay.owner_id);

            // Dispatch overlay scroll to owner widget
            if let Some(result) =
                page.dispatch_overlay_scroll(&overlay.owner_id, scroll.direction, scroll.amount, cx)
                && result.is_handled() {
                    // Dispatch on_scroll handler if present
                    if let Some(handler_id) = page.get_list_scroll_handler(&overlay.owner_id) {
                        dispatch_to_layer(instance, &state.modal_stack, &handler_id, cx);
                    }
                }

            // Don't propagate to widgets below overlay
            return;
        }
    }

    let Some(hit_box) = hit_map.hit_test(scroll.position.x, scroll.position.y) else {
        return;
    };

    // Dispatch scroll event to widget
    page.dispatch_scroll_event(&hit_box.id, scroll.direction, scroll.amount, cx);

    // Dispatch on_scroll handler if present
    if let Some(handler_id) = page.get_list_scroll_handler(&hit_box.id) {
        dispatch_to_layer(instance, &state.modal_stack, &handler_id, cx);
    }
}

// =============================================================================
// Drag/Release Event Handlers
// =============================================================================

/// Handle a drag event.
pub fn handle_drag_event(drag: &DragEvent, page: &Node, state: &EventLoopState, cx: &AppContext) {
    if let Some(ref id) = state.drag_widget_id {
        page.dispatch_drag_event(id, drag.position.x, drag.position.y, drag.modifiers, cx);
    }
}

/// Handle a release event (end of drag).
pub fn handle_release_event(page: &Node, state: &mut EventLoopState, cx: &AppContext) {
    if let Some(id) = state.drag_widget_id.take() {
        page.dispatch_release_event(&id, cx);
    }
}

// =============================================================================
// Main Event Dispatcher
// =============================================================================

/// Dispatch an event to the appropriate handler.
///
/// Returns `ControlFlow::Break(())` if the app should exit.
#[allow(clippy::too_many_arguments)]
pub fn dispatch_event(
    event: &Event,
    page: &Node,
    hit_map: &HitTestMap,
    instance: &dyn AnyAppInstance,
    state: &mut EventLoopState,
    app_keybinds: &Arc<RwLock<Keybinds>>,
    cx: &AppContext,
) -> ControlFlow<()> {
    let dispatch_start = Instant::now();
    let result = dispatch_event_inner(event, page, hit_map, instance, state, app_keybinds, cx);
    let dispatch_elapsed = dispatch_start.elapsed();
    if dispatch_elapsed.as_millis() > 2 {
        warn!("PROFILE: dispatch_event({:?}) took {:?}", event.name(), dispatch_elapsed);
    }
    result
}

/// Inner dispatch function (separated for profiling).
#[allow(clippy::too_many_arguments)]
fn dispatch_event_inner(
    event: &Event,
    page: &Node,
    hit_map: &HitTestMap,
    instance: &dyn AnyAppInstance,
    state: &mut EventLoopState,
    app_keybinds: &Arc<RwLock<Keybinds>>,
    cx: &AppContext,
) -> ControlFlow<()> {
    match event {
        Event::Quit => {
            log::info!("Quit requested via system keybind");
            return ControlFlow::Break(());
        }
        Event::Key(key_combo) => {
            debug!("Key event: {:?}", key_combo);
            if let ControlFlow::Break(()) =
                handle_key_event(key_combo, page, instance, state, app_keybinds, cx)
            {
                return ControlFlow::Break(());
            }
        }
        Event::Resize { width, height } => {
            debug!("Resize: {}x{}", width, height);
        }
        Event::Click(click) => {
            debug!("Click at ({}, {})", click.position.x, click.position.y);
            handle_click_event(click, page, hit_map, instance, state, cx);
        }
        Event::Hover(position) => {
            // Note: Hover coalescing is still handled in event_loop.rs before calling this
            handle_hover_event(*position, page, hit_map, instance, state, cx);
        }
        Event::Scroll(scroll) => {
            debug!("Scroll at ({}, {})", scroll.position.x, scroll.position.y);
            handle_scroll_event(scroll, page, hit_map, instance, state, cx);
        }
        Event::Release(_) => {
            handle_release_event(page, state, cx);
        }
        Event::Drag(drag) => {
            handle_drag_event(drag, page, state, cx);
        }
        Event::FocusGained | Event::FocusLost => {
            // Handled in event_loop.rs for animation lifecycle
            // No widget-level handling needed
        }
    }

    ControlFlow::Continue(())
}
