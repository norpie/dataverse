//! Event handlers extracted from the main event loop.
//!
//! This module contains the event handling logic, extracted from `event_loop.rs`
//! for maintainability. Each handler function processes a specific event type.

use std::ops::ControlFlow;
use std::sync::{Arc, RwLock};

use log::debug;

use crate::app::App;
use crate::widgets::EventResult;
use crate::widgets::events::WidgetEventKind;
use crate::context::AppContext;
use crate::events::{ClickEvent, Position, ScrollEvent};
use crate::keybinds::{HandlerId, Key, KeyCombo, Keybinds};
use crate::node::Node;

use super::events::{DragEvent, Event};
use super::hit_test::HitTestMap;
use super::input::KeybindMatch;
use super::modal::ModalStackEntry;
use super::state::EventLoopState;

// =============================================================================
// Handler Dispatch Helpers
// =============================================================================

/// Dispatch a handler to the appropriate layer (modal or app).
pub fn dispatch_to_layer<A: App>(
    app: &A,
    modal_stack: &[ModalStackEntry],
    handler_id: &HandlerId,
    cx: &AppContext,
) {
    if let Some(entry) = modal_stack.last() {
        entry.modal.dispatch_dyn(handler_id, cx);
    } else {
        app.dispatch(handler_id, cx);
    }
}

/// Dispatch handlers based on context data set by widgets.
///
/// Components push events to the event queue via `AppContext::push_event()`.
/// This function drains the queue and dispatches appropriate handlers based
/// on event kind and the widget ID that triggered it.
pub fn dispatch_component_handlers<A: App>(
    page: &Node,
    _focus_id: &str,
    app: &A,
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
        };

        if let Some(handler_id) = handler {
            dispatch_to_layer(app, modal_stack, &handler_id, cx);
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
pub fn handle_key_event<A: App>(
    key_combo: &KeyCombo,
    page: &Node,
    app: &A,
    state: &mut EventLoopState,
    app_keybinds: &Arc<RwLock<Keybinds>>,
    cx: &AppContext,
) -> ControlFlow<(), bool> {
    // Handle Tab/Shift+Tab for focus navigation
    if key_combo.key == Key::Tab {
        let focus_state = state.focus_state_mut();
        if key_combo.modifiers.shift {
            debug!("Focus prev");
            focus_state.focus_prev();
        } else {
            debug!("Focus next");
            focus_state.focus_next();
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

            // Check if this is a checkbox (Enter toggles it)
            let old_checkbox_state = page
                .get_checkbox_component(focus_id)
                .map(|c| c.is_checked());

            // Dispatch to widget first (sets context data)
            if let Some(result) = page.dispatch_key_event(focus_id, key_combo, cx)
                && result.is_handled()
            {
                // Dispatch handlers based on context data
                dispatch_component_handlers(page, focus_id, app, &state.modal_stack, cx);

                // For checkboxes, dispatch on_change if state changed
                if let Some(old) = old_checkbox_state
                    && let Some(widget) = page.get_checkbox_component(focus_id)
                    && widget.is_checked() != old
                    && let Some(handler_id) = page.get_change_handler(focus_id)
                {
                    dispatch_to_layer(app, &state.modal_stack, &handler_id, cx);
                }

                return ControlFlow::Continue(true);
            }
            // Fallback for buttons etc
            if let Some(handler_id) = page.get_submit_handler(focus_id) {
                dispatch_to_layer(app, &state.modal_stack, &handler_id, cx);
            }
            return ControlFlow::Continue(true);
        }

        // For all other keys, dispatch to widget
        let old_value = page.get_input_component(focus_id).map(|c| c.value());
        let old_checkbox_state = page
            .get_checkbox_component(focus_id)
            .map(|c| c.is_checked());
        let old_radio_selection = page
            .get_radio_group_component(focus_id)
            .map(|c| c.selected());

        if let Some(result) = page.dispatch_key_event(focus_id, key_combo, cx)
            && result.is_handled()
        {
            // Dispatch handlers based on context data
            dispatch_component_handlers(page, focus_id, app, &state.modal_stack, cx);

            // For inputs, check if value changed to trigger on_change
            if let Some(old) = old_value
                && let Some(widget) = page.get_input_component(focus_id)
                && widget.value() != old
            {
                cx.set_input_text(widget.value());
                if let Some(handler_id) = page.get_change_handler(focus_id) {
                    dispatch_to_layer(app, &state.modal_stack, &handler_id, cx);
                }
            }

            // For checkboxes, check if state changed to trigger on_change (Space key)
            if let Some(old) = old_checkbox_state
                && let Some(widget) = page.get_checkbox_component(focus_id)
                && widget.is_checked() != old
                && let Some(handler_id) = page.get_change_handler(focus_id)
            {
                dispatch_to_layer(app, &state.modal_stack, &handler_id, cx);
            }

            // For radio groups, check if selection changed to trigger on_change
            if let Some(old) = old_radio_selection
                && let Some(widget) = page.get_radio_group_component(focus_id)
                && widget.selected() != old
                && let Some(handler_id) = page.get_change_handler(focus_id)
            {
                dispatch_to_layer(app, &state.modal_stack, &handler_id, cx);
            }

            return ControlFlow::Continue(true);
        }
    }

    // Handle Escape to clear focus (if not handled by widget)
    if key_combo.key == Key::Escape {
        debug!("Escape pressed, clearing focus");
        state.focus_state_mut().clear_focus();
        return ControlFlow::Continue(true);
    }

    // Process keybind (only if not handled above)
    let current_page = if state.modal_stack.is_empty() {
        app.current_page()
    } else {
        None
    };
    let current_page_ref = current_page.as_deref();

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

    match keybind_match {
        KeybindMatch::Match(handler_id) => {
            log::info!("Keybind matched: {:?}", handler_id);
            dispatch_to_layer(app, &state.modal_stack, &handler_id, cx);
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
/// This function uses `AnySelectable` to handle List/Tree/Table clicks uniformly,
/// eliminating the previous 3-way branching logic.
///
/// Returns `true` if the loop should continue to the next iteration.
pub fn handle_click_event<A: App>(
    click: &ClickEvent,
    page: &Node,
    hit_map: &HitTestMap,
    app: &A,
    state: &mut EventLoopState,
    cx: &AppContext,
) -> bool {
    let Some(hit_box) = hit_map.hit_test(click.position.x, click.position.y) else {
        return false;
    };

    // Focus the clicked element
    state.focus_state_mut().set_focus(hit_box.id.clone());

    // First, check if this is a selectable widget (List/Tree/Table)
    // Using AnySelectable for unified handling
    if let Some(selectable) = page.get_selectable_component(&hit_box.id) {
        debug!("Clicked on selectable element: {}", hit_box.id);

        // First check if click is on scrollbar (dispatch_click_event handles this)
        if let Some(result) =
            page.dispatch_click_event(&hit_box.id, click.position.x, click.position.y, cx)
        {
            match result {
                EventResult::StartDrag => {
                    state.drag_widget_id = Some(hit_box.id.clone());
                    return true;
                }
                EventResult::Consumed => {
                    dispatch_component_handlers(page, &hit_box.id, app, &state.modal_stack, cx);
                    return true;
                }
                EventResult::Ignored => {}
            }
        }

        // Calculate viewport-relative coordinates
        let x_in_viewport = click.position.x.saturating_sub(hit_box.rect.x);
        let y_in_viewport = click.position.y.saturating_sub(hit_box.rect.y);

        // Check for header click (Table only has headers)
        if selectable.has_header() && y_in_viewport == 0 {
            debug!("Header click at x={}", x_in_viewport);
            selectable.on_header_click(x_in_viewport, cx);
        } else {
            // Data row click - handle with modifiers
            let ctrl = click.modifiers.ctrl;
            let shift = click.modifiers.shift;
            selectable.on_click_with_modifiers(y_in_viewport, ctrl, shift, cx);
        }

        // Dispatch handlers based on context data
        dispatch_component_handlers(page, &hit_box.id, app, &state.modal_stack, cx);
        return true;
    }

    // Not a selectable widget - try other widgets (ScrollArea, Input, Button)
    if let Some(result) =
        page.dispatch_click_event(&hit_box.id, click.position.x, click.position.y, cx)
    {
        match result {
            EventResult::StartDrag => {
                state.drag_widget_id = Some(hit_box.id.clone());
                return true;
            }
            EventResult::Consumed => return true,
            EventResult::Ignored => {}
        }
    }

    debug!("Clicked element: {}", hit_box.id);

    // If it's a checkbox, toggle it and dispatch on_change handler
    if let Some(checkbox) = page.get_checkbox_component(&hit_box.id) {
        checkbox.toggle();
        if let Some(handler_id) = page.get_change_handler(&hit_box.id) {
            dispatch_to_layer(app, &state.modal_stack, &handler_id, cx);
        }
        return true;
    }

    // If it's a radio group, select the clicked option and dispatch on_change handler
    if let Some(radio_group) = page.get_radio_group_component(&hit_box.id) {
        // Calculate which option was clicked based on Y position within the widget
        let y_in_component = click.position.y.saturating_sub(hit_box.rect.y) as usize;
        let old_selection = radio_group.selected();
        radio_group.select(y_in_component);
        if radio_group.selected() != old_selection {
            if let Some(handler_id) = page.get_change_handler(&hit_box.id) {
                dispatch_to_layer(app, &state.modal_stack, &handler_id, cx);
            }
        }
        return true;
    }

    // If it's a button, dispatch click handler
    if !hit_box.captures_input
        && let Some(handler_id) = page.get_submit_handler(&hit_box.id)
    {
        dispatch_to_layer(app, &state.modal_stack, &handler_id, cx);
    }

    false
}

// =============================================================================
// Hover Event Handler
// =============================================================================

/// Handle a hover event.
///
/// Returns `true` if the loop should continue to the next iteration.
pub fn handle_hover_event<A: App>(
    position: Position,
    page: &Node,
    hit_map: &HitTestMap,
    app: &A,
    state: &mut EventLoopState,
    cx: &AppContext,
) -> bool {
    let Some(hit_box) = hit_map.hit_test(position.x, position.y) else {
        return false;
    };

    // Focus if not already focused
    let current_focus = state.focused_id();
    if current_focus.as_deref() != Some(&hit_box.id) {
        debug!("Hover focus: {}", hit_box.id);
        state.focus_state_mut().set_focus(hit_box.id.clone());
    }

    // Dispatch hover event to widget (lists use this to move cursor)
    if let Some(result) = page.dispatch_hover_event(
        &hit_box.id,
        position.x,
        position.y.saturating_sub(hit_box.rect.y),
        cx,
    ) && result.is_handled()
    {
        dispatch_component_handlers(page, &hit_box.id, app, &state.modal_stack, cx);
    }

    false
}

// =============================================================================
// Scroll Event Handler
// =============================================================================

/// Handle a scroll event.
pub fn handle_scroll_event<A: App>(
    scroll: &ScrollEvent,
    page: &Node,
    hit_map: &HitTestMap,
    app: &A,
    state: &EventLoopState,
    cx: &AppContext,
) {
    let Some(hit_box) = hit_map.hit_test(scroll.position.x, scroll.position.y) else {
        return;
    };

    // Dispatch scroll event to widget
    page.dispatch_scroll_event(&hit_box.id, scroll.direction, scroll.amount, cx);

    // Dispatch on_scroll handler if present
    if let Some(handler_id) = page.get_list_scroll_handler(&hit_box.id) {
        dispatch_to_layer(app, &state.modal_stack, &handler_id, cx);
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
pub fn dispatch_event<A: App>(
    event: &Event,
    page: &Node,
    hit_map: &HitTestMap,
    app: &A,
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
                handle_key_event(key_combo, page, app, state, app_keybinds, cx)
            {
                return ControlFlow::Break(());
            }
        }
        Event::Resize { width, height } => {
            debug!("Resize: {}x{}", width, height);
        }
        Event::Click(click) => {
            debug!("Click at ({}, {})", click.position.x, click.position.y);
            handle_click_event(click, page, hit_map, app, state, cx);
        }
        Event::Hover(position) => {
            // Note: Hover coalescing is still handled in event_loop.rs before calling this
            handle_hover_event(*position, page, hit_map, app, state, cx);
        }
        Event::Scroll(scroll) => {
            debug!("Scroll at ({}, {})", scroll.position.x, scroll.position.y);
            handle_scroll_event(scroll, page, hit_map, app, state, cx);
        }
        Event::Release(_) => {
            handle_release_event(page, state, cx);
        }
        Event::Drag(drag) => {
            handle_drag_event(drag, page, state, cx);
        }
    }

    ControlFlow::Continue(())
}
