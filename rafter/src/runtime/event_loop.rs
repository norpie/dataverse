//! Main event loop for the runtime.

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crossterm::event;
use log::{debug, info, trace, warn};

use crate::app::{BlurPolicy, InstanceId, InstanceRegistry};
use crate::context::{AppContext, InstanceCommand, RequestTarget};
use crate::input::focus::FocusId;
use crate::input::keybinds::Keybinds;
use crate::layers::overlay::{ActiveOverlay, OverlayRequest, calculate_overlay_position};
use crate::request::RequestError;
use crate::styling::theme::Theme;
use crate::system::registered_systems;

use super::RuntimeError;
use super::events::{Event, convert_event};
use super::handlers::dispatch_event;
use super::hit_test::HitTestMap;
use super::input::InputState;
use super::modal::{ModalStackEntry, calculate_modal_area};
use super::render::{dim_backdrop, fill_background, render_node, render_toasts};
use super::state::EventLoopState;
use super::terminal::TerminalGuard;

use crate::input::events::Position;
use crate::input::focus::FocusState;

/// Coalesce pending hover events, returning the latest hover position.
/// This drains all pending events and returns:
/// - The latest hover position (if any hover events were found)
/// - Any non-hover events that were encountered (to be processed later)
/// - The count of skipped hover events
fn coalesce_hover_events(
    initial_position: Position,
) -> Result<(Position, Vec<event::Event>, usize), std::io::Error> {
    let mut latest_position = initial_position;
    let mut other_events = Vec::new();
    let mut skipped_count = 0;

    // Drain all pending events
    while event::poll(Duration::from_millis(0))? {
        if let Ok(crossterm_event) = event::read()
            && let Some(rafter_event) = convert_event(crossterm_event.clone())
        {
            match rafter_event {
                Event::Hover(pos) => {
                    // Replace with newer hover position
                    latest_position = pos;
                    skipped_count += 1;
                }
                _ => {
                    // Keep non-hover events for later processing
                    other_events.push(crossterm_event);
                }
            }
        }
    }

    Ok((latest_position, other_events, skipped_count))
}

/// Apply blur policy to an instance that is losing focus.
///
/// Returns the ID of an instance that should be closed due to BlurPolicy::Close.
fn apply_blur_policy(
    registry: &mut InstanceRegistry,
    instance_id: InstanceId,
    cx: &AppContext,
) -> Option<InstanceId> {
    let instance = registry.get(instance_id)?;

    let policy = instance.config().on_blur;

    // Call on_background lifecycle hook
    instance.on_background(cx);

    match policy {
        BlurPolicy::Continue => {
            // Nothing special - instance keeps running
            debug!("Instance {:?} continues in background", instance_id);
            None
        }
        BlurPolicy::Sleep => {
            // Mark as sleeping - no events will be delivered
            debug!("Instance {:?} entering sleep", instance_id);
            if let Some(instance) = registry.get_mut(instance_id) {
                instance.set_sleeping(true);
            }
            None
        }
        BlurPolicy::Close => {
            // Schedule for closure
            debug!("Instance {:?} scheduled for close (BlurPolicy::Close)", instance_id);
            Some(instance_id)
        }
    }
}

/// Wake a sleeping instance when it gains focus.
fn wake_instance(registry: &mut InstanceRegistry, instance_id: InstanceId) {
    if let Some(instance) = registry.get_mut(instance_id)
        && instance.is_sleeping()
    {
        debug!("Waking instance {:?}", instance_id);
        instance.set_sleeping(false);
    }
}

/// Process instance commands from the context.
///
/// Returns true if the runtime should exit (all instances closed).
fn process_instance_commands(
    registry: &Arc<RwLock<InstanceRegistry>>,
    app_keybinds: &Arc<RwLock<Keybinds>>,
    systems: &[Box<dyn crate::system::AnySystem>],
    cx: &AppContext,
) -> bool {
    let commands = cx.take_instance_commands();

    // Collect instances to close due to BlurPolicy::Close
    let mut to_close: Vec<InstanceId> = Vec::new();

    for cmd in commands {
        match cmd {
            InstanceCommand::Spawn { instance, focus } => {
                let id = instance.id();
                info!("Spawning instance: {:?}", id);

                let mut reg = registry.write().unwrap();

                // Call on_start before inserting (the instance is about to start)
                instance.on_start(cx);

                reg.insert(id, instance);

                if focus {
                    // Apply blur policy to old focused instance
                    if let Some(old_id) = reg.focused()
                        && old_id != id
                        && let Some(close_id) = apply_blur_policy(&mut reg, old_id, cx)
                    {
                        to_close.push(close_id);
                    }

                    reg.focus(id);

                    // Update keybinds to new instance's keybinds
                    if let Some(new_instance) = reg.get(id) {
                        let new_keybinds = new_instance.keybinds();
                        if let Ok(mut kb) = app_keybinds.write() {
                            *kb = new_keybinds;
                        }
                        new_instance.on_foreground(cx);
                    }
                }
            }
            InstanceCommand::Close { id, force } => {
                info!("Closing instance: {:?} (force={})", id, force);

                let mut reg = registry.write().unwrap();

                // Check on_close_request if not forcing
                if !force
                    && let Some(instance) = reg.get(id)
                        && !instance.on_close_request(cx) {
                            debug!("Close cancelled by on_close_request");
                            continue;
                        }

                // Call on_close lifecycle hook
                if let Some(instance) = reg.get(id) {
                    instance.on_close(cx);
                }

                // Actually close the instance
                reg.close(id, force);

                // If this was focused, update keybinds to new focused instance
                if let Some(new_focused) = reg.focused_instance() {
                    let new_keybinds = new_focused.keybinds();
                    if let Ok(mut kb) = app_keybinds.write() {
                        *kb = new_keybinds;
                    }
                }
            }
            InstanceCommand::Focus { id } => {
                info!("Focusing instance: {:?}", id);

                let mut reg = registry.write().unwrap();

                // Apply blur policy to old focused instance
                if let Some(old_id) = reg.focused()
                    && old_id != id
                    && let Some(close_id) = apply_blur_policy(&mut reg, old_id, cx)
                {
                    to_close.push(close_id);
                }

                if reg.focus(id) {
                    // Wake the instance if it was sleeping
                    wake_instance(&mut reg, id);

                    // Update keybinds to new instance's keybinds
                    if let Some(new_instance) = reg.get(id) {
                        let new_keybinds = new_instance.keybinds();
                        if let Ok(mut kb) = app_keybinds.write() {
                            *kb = new_keybinds;
                        }
                        new_instance.on_foreground(cx);
                    }
                }
            }
            InstanceCommand::PublishEvent { event } => {
                let event_type = event.type_id();
                debug!("Publishing event: {:?}", event_type);

                // Dispatch to all systems that have a handler
                for system in systems {
                    if system.has_event_handler(event_type) {
                        let event_clone = event.clone();
                        system.dispatch_event(event_type, event_clone.into_inner(), cx);
                    }
                }

                let reg = registry.read().unwrap();

                // Dispatch to all non-sleeping instances that have a handler
                for instance in reg.iter() {
                    if instance.is_sleeping() {
                        continue;
                    }
                    if instance.has_event_handler(event_type) {
                        // Clone the event for each subscriber
                        let event_clone = event.clone();
                        instance.dispatch_event(event_type, event_clone.into_inner(), cx);
                    }
                }
            }
            InstanceCommand::SendRequest {
                target,
                request,
                request_type,
                response_tx,
            } => {
                debug!("Processing request: {:?} -> {:?}", request_type, target);

                let reg = registry.read().unwrap();

                // Find target instance
                let target_instance = match &target {
                    RequestTarget::AppType(type_id) => {
                        // Find first non-sleeping instance of this type
                        reg.iter()
                            .find(|i| i.type_id() == *type_id && !i.is_sleeping())
                    }
                    RequestTarget::Instance(id) => reg.get(*id),
                };

                let Some(instance) = target_instance else {
                    let err = match &target {
                        RequestTarget::AppType(_) => RequestError::NoInstance,
                        RequestTarget::Instance(id) => RequestError::InstanceNotFound(*id),
                    };
                    let _ = response_tx.send(Err(err));
                    continue;
                };

                // Check if sleeping (only relevant for Instance target)
                if matches!(&target, RequestTarget::Instance(_)) && instance.is_sleeping() {
                    let _ = response_tx.send(Err(RequestError::InstanceSleeping(instance.id())));
                    continue;
                }

                // Check if handler exists
                if !instance.has_request_handler(request_type) {
                    let _ = response_tx.send(Err(RequestError::NoHandler));
                    continue;
                }

                // Dispatch the request
                if let Some(future) = instance.dispatch_request(request_type, request, cx) {
                    // Spawn task to await the response and send it back
                    tokio::spawn(async move {
                        let response = future.await;
                        let _ = response_tx.send(Ok(response));
                    });
                } else {
                    let _ = response_tx.send(Err(RequestError::NoHandler));
                }
            }
        }
    }

    // Process deferred closes from BlurPolicy::Close
    if !to_close.is_empty() {
        let mut reg = registry.write().unwrap();
        for id in to_close {
            info!("Closing instance {:?} due to BlurPolicy::Close", id);
            if let Some(instance) = reg.get(id) {
                instance.on_close(cx);
            }
            reg.close(id, true); // Force close - policy already decided
        }

        // Update keybinds if needed
        if let Some(new_focused) = reg.focused_instance() {
            let new_keybinds = new_focused.keybinds();
            if let Ok(mut kb) = app_keybinds.write() {
                *kb = new_keybinds;
            }
        }
    }

    // Check if we should exit (no instances left)
    let reg = registry.read().unwrap();
    reg.is_empty()
}

/// Run the main event loop with an instance registry.
pub async fn run_event_loop(
    registry: Arc<RwLock<InstanceRegistry>>,
    app_keybinds: Arc<RwLock<Keybinds>>,
    cx: AppContext,
    theme: Arc<dyn Theme>,
    term_guard: &mut TerminalGuard,
) -> Result<(), RuntimeError> {
    // Collect registered systems
    let systems: Vec<_> = registered_systems()
        .map(|reg| (reg.factory)())
        .collect();

    info!("Registered {} systems", systems.len());
    for system in &systems {
        info!("  System: {}", system.name());
        for bind in system.keybinds().all() {
            debug!(
                "    Keybind: {} ({:?}) => {:?}",
                bind.id, bind.default_keys, bind.handler
            );
        }
    }

    // Log app keybinds
    {
        let kb = app_keybinds.read().unwrap();
        info!("Registered {} app keybinds", kb.all().len());
        for bind in kb.all() {
            debug!(
                "  Keybind: {} ({:?}) => {:?}",
                bind.id, bind.default_keys, bind.handler
            );
        }
    }

    // Initialize event loop state with systems
    let mut state = EventLoopState::new(theme, systems);

    // Call on_start and on_foreground for the initial instance
    {
        let reg = registry.read().unwrap();
        if let Some(instance) = reg.focused_instance() {
            info!("App started: {}", instance.config().name);
            instance.on_start(&cx);
            instance.on_foreground(&cx);
        }
    }

    // Main event loop
    loop {
        // Check if exit was requested (by a handler from previous iteration)
        if cx.is_exit_requested() {
            info!("Exit requested by handler");
            break;
        }

        // Process pending instance commands
        if process_instance_commands(&registry, &app_keybinds, &state.systems, &cx) {
            info!("All instances closed, exiting");
            break;
        }

        // Get focused instance - if none, exit
        let focused_id: InstanceId;
        {
            let reg = registry.read().unwrap();
            match reg.focused() {
                Some(id) => focused_id = id,
                None => {
                    info!("No focused instance, exiting");
                    break;
                }
            }
        }

        // Update context with current instance ID
        cx.set_instance_id(focused_id);

        // Process pending modal requests
        if let Some(modal) = cx.take_modal_request() {
            info!("Opening modal: {}", modal.name());
            let keybinds = modal.keybinds();
            state.modal_stack.push(ModalStackEntry {
                modal,
                focus_state: FocusState::new(),
                input_state: InputState::new(),
                keybinds,
            });
        }

        // Remove closed modals from the stack
        let modal_count_before = state.modal_stack.len();
        state.modal_stack.retain(|entry| !entry.modal.is_closed());
        let modal_closed = state.modal_stack.len() < modal_count_before;

        // Process any pending focus requests (only for app, not modals)
        if !state.in_modal()
            && let Some(focus_id) = cx.take_focus_request()
        {
            debug!("Focus requested: {:?}", focus_id);
            state.app_focus_state.set_focus(focus_id);
        }

        // Process any pending theme change requests
        if let Some(new_theme) = cx.take_theme_request() {
            info!("Theme changed");
            state.current_theme = new_theme;
        }

        // Process any pending toasts
        for toast in cx.take_toasts() {
            let expiry = Instant::now() + toast.duration;
            info!("Toast: {}", toast.message);
            state.active_toasts.push((toast, expiry));
        }

        // Remove expired toasts
        let now = Instant::now();
        state.active_toasts.retain(|(_, expiry)| *expiry > now);

        // Get page and render using focused instance
        let reg = registry.read().unwrap();
        let Some(instance) = reg.focused_instance() else {
            drop(reg);
            info!("Focused instance disappeared, exiting");
            break;
        };

        // Get the page tree for focused instance or top modal
        let page = if let Some(entry) = state.modal_stack.last() {
            entry.modal.page()
        } else {
            instance.page()
        };

        // Update focusable IDs from page tree
        let focusable_ids: Vec<FocusId> =
            page.focusable_ids().into_iter().map(FocusId::new).collect();

        // Update focus state for the active layer
        if let Some(entry) = state.modal_stack.last_mut() {
            entry.focus_state.set_focusable_ids(focusable_ids);
        } else {
            state.app_focus_state.set_focusable_ids(focusable_ids);
        }

        // Render and build hit test map
        let mut hit_map = HitTestMap::new();
        let theme = &state.current_theme;
        let focused_element_id = state.focused_id();

        // Cache instance page (computed once per frame)
        let view_start = Instant::now();
        let app_view = instance.page();
        let view_elapsed = view_start.elapsed();
        if view_elapsed.as_millis() > 5 {
            warn!("PROFILE: instance.page() took {:?}", view_elapsed);
        }

        // Cache modal views (computed once per frame)
        let modal_views: Vec<_> = state.modal_stack.iter().map(|e| e.modal.page()).collect();

        // Collect overlay requests during rendering
        let mut overlay_requests: Vec<OverlayRequest> = Vec::new();

        let modal_stack_ref = &state.modal_stack;
        let draw_start = Instant::now();
        term_guard.terminal().draw(|frame| {
            let area = frame.area();

            // Fill entire terminal with theme background color
            if let Some(bg_color) = theme.resolve("background") {
                fill_background(frame, bg_color.to_ratatui());
            }

            // Always render the app first
            let app_focused = if modal_stack_ref.is_empty() {
                focused_element_id.as_deref()
            } else {
                None
            };
            render_node(
                frame,
                &app_view,
                area,
                &mut hit_map,
                theme.as_ref(),
                app_focused,
                &mut overlay_requests,
            );

            // Render modals on top with backdrop dimming
            for (i, (entry, modal_view)) in
                modal_stack_ref.iter().zip(modal_views.iter()).enumerate()
            {
                // Dim the backdrop
                dim_backdrop(frame.buffer_mut(), 0.4);

                // Calculate modal area based on position and size
                let modal_area = calculate_modal_area(
                    area,
                    entry.modal.position(),
                    entry.modal.size(),
                    modal_view,
                );

                // Clear the modal area
                frame.render_widget(ratatui::widgets::Clear, modal_area);

                // Only show focus for the top modal
                let is_top_modal = i == modal_stack_ref.len() - 1;
                let modal_focused = if is_top_modal {
                    focused_element_id.as_deref()
                } else {
                    None
                };

                // Render modal page
                render_node(
                    frame,
                    modal_view,
                    modal_area,
                    &mut hit_map,
                    theme.as_ref(),
                    modal_focused,
                    &mut overlay_requests,
                );
            }

            // Render overlay layer (above modals, below toasts)
            let mut active_overlays: Vec<ActiveOverlay> = Vec::new();
            for request in overlay_requests.drain(..) {
                let content_size = (
                    request.content.intrinsic_width().max(1),
                    request.content.intrinsic_height().max(1),
                );
                let overlay_area = calculate_overlay_position(
                    area,
                    request.anchor,
                    content_size,
                    request.position,
                );

                // Clear the overlay area
                frame.render_widget(ratatui::widgets::Clear, overlay_area);

                // Render overlay content (no focus - overlays don't trap focus)
                // Note: We pass an empty overlay_requests here since overlays shouldn't nest
                let mut nested_overlays: Vec<OverlayRequest> = Vec::new();
                render_node(
                    frame,
                    &request.content,
                    overlay_area,
                    &mut hit_map,
                    theme.as_ref(),
                    None, // Overlays don't show focus indicators
                    &mut nested_overlays,
                );

                // Track active overlay for click-outside detection
                active_overlays.push(ActiveOverlay::new(request.owner_id, overlay_area));
            }

            // Store active overlays in state for event handling
            state.active_overlays = active_overlays;

            // Render toasts on top of everything
            render_toasts(frame, &state.active_toasts, theme.as_ref());
        })?;
        let draw_elapsed = draw_start.elapsed();
        if draw_elapsed.as_millis() > 5 {
            warn!("PROFILE: terminal.draw() took {:?}", draw_elapsed);
        }

        // Clear dirty flags after render
        if let Some(entry) = state.modal_stack.last() {
            entry.modal.clear_dirty();
        } else {
            instance.clear_dirty();
        }

        // Need to check dirty on instance, so we need to get instance info before dropping reg
        let instance_dirty = instance.is_dirty();

        // Drop the registry lock before waiting for events
        drop(reg);

        // Determine poll timeout - skip waiting if state changed
        let needs_immediate_update =
            state.needs_immediate_update_multi(instance_dirty, modal_closed);
        let poll_timeout = if needs_immediate_update {
            Duration::from_millis(0)
        } else {
            Duration::from_millis(100)
        };

        // Wait for events (with timeout for animations/toast expiry)
        if event::poll(poll_timeout)?
            && let Ok(crossterm_event) = event::read()
        {
            trace!("Crossterm event: {:?}", crossterm_event);

            if let Some(rafter_event) = convert_event(crossterm_event) {
                debug!("Rafter event: {:?}", rafter_event);

                // Handle hover coalescing specially
                let event_to_dispatch = match rafter_event {
                    Event::Hover(position) => {
                        let (final_position, other_events, skipped) =
                            coalesce_hover_events(position)?;

                        if skipped > 0 {
                            debug!(
                                "Hover coalesced: skipped {} events, final pos ({}, {})",
                                skipped, final_position.x, final_position.y
                            );
                        }

                        // TODO: Process other_events that were collected during coalescing
                        // For now, we drop them - this is a trade-off for responsiveness
                        if !other_events.is_empty() {
                            debug!(
                                "Hover coalescing dropped {} non-hover events",
                                other_events.len()
                            );
                        }

                        Event::Hover(final_position)
                    }
                    other => other,
                };

                // Dispatch the event using the unified handler
                // Need to re-acquire registry lock for event dispatch
                let reg = registry.read().unwrap();
                if let Some(instance) = reg.focused_instance()
                    && dispatch_event(
                        &event_to_dispatch,
                        &page,
                        &hit_map,
                        instance,
                        &mut state,
                        &app_keybinds,
                        &cx,
                    )
                    .is_break()
                    {
                        break;
                    }
            }
        }
    }

    // Call on_stop for all instances
    {
        let reg = registry.read().unwrap();
        for instance in reg.iter() {
            instance.on_close(&cx);
        }
    }
    info!("Runtime stopped");

    Ok(())
}
