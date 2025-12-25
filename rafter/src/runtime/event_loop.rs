//! Main event loop for the runtime.

use std::sync::{Arc, RwLock};
use std::time::Instant;

use crossterm::event::EventStream;
use futures::StreamExt;
use log::{debug, info, trace, warn};
use tokio::time::{Duration, sleep_until};

use super::wakeup::{self, channel};

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
use super::render::{calculate_toast_removal_time, dim_backdrop, fill_background, has_animating_toasts, render_node, render_toasts};
use super::state::EventLoopState;
use super::terminal::TerminalGuard;

use crate::input::events::Position;
use crate::input::focus::FocusState;

/// Coalesce pending hover events, returning the latest hover position.
/// This drains all pending events and returns:
/// - The latest hover position (if any hover events were found)
/// - Any non-hover events that were encountered (to be processed later)
/// - The count of skipped hover events
async fn coalesce_hover_events(
    events: &mut EventStream,
    initial_position: Position,
) -> (Position, Vec<crossterm::event::Event>, usize) {
    let mut latest_position = initial_position;
    let mut other_events = Vec::new();
    let mut skipped_count = 0;

    // Drain all pending events using non-blocking try_next
    loop {
        // Use tokio's try_recv pattern - poll once without waiting
        let next = tokio::time::timeout(Duration::from_millis(0), events.next()).await;

        match next {
            Ok(Some(Ok(crossterm_event))) => {
                if let Some(rafter_event) = convert_event(crossterm_event.clone()) {
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
            // No more pending events or error - stop draining
            _ => break,
        }
    }

    (latest_position, other_events, skipped_count)
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
/// Result of processing instance commands
struct ProcessCommandsResult {
    /// Whether to exit (all instances closed)
    should_exit: bool,
    /// Whether any commands were processed that need a render
    needs_render: bool,
}

fn process_instance_commands(
    registry: &Arc<RwLock<InstanceRegistry>>,
    app_keybinds: &Arc<RwLock<Keybinds>>,
    systems: &[Box<dyn crate::system::AnySystem>],
    cx: &AppContext,
    wakeup_sender: &wakeup::WakeupSender,
) -> ProcessCommandsResult {
    let commands = cx.take_instance_commands();
    let has_commands = !commands.is_empty();

    if has_commands {
        debug!("Processing {} instance commands", commands.len());
    }

    // Collect instances to close due to BlurPolicy::Close
    let mut to_close: Vec<InstanceId> = Vec::new();

    for cmd in commands {
        match cmd {
            InstanceCommand::Spawn { instance, focus } => {
                let id = instance.id();
                info!("Spawning instance: {:?}", id);

                let mut reg = registry.write().unwrap();

                // Install wakeup sender so State updates trigger re-render
                instance.install_wakeup(wakeup_sender.clone());

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
    ProcessCommandsResult {
        should_exit: reg.is_empty(),
        needs_render: has_commands,
    }
}

use super::animation::AnimationManager;

/// Compute the next deadline for toast expiry and animation completion.
/// Returns None if there are no toasts with expiry times and no animations.
fn compute_next_deadline(
    toasts: &[(crate::context::Toast, Instant)],
    animations: &AnimationManager,
) -> Option<Instant> {
    // Calculate removal time for each toast (accounts for activation time and slide-out)
    let toast_deadline = toasts
        .iter()
        .enumerate()
        .map(|(i, _)| calculate_toast_removal_time(i, toasts))
        .min();
    let animation_deadline = animations.next_completion_time();

    // Return the earliest deadline
    match (toast_deadline, animation_deadline) {
        (Some(t), Some(a)) => Some(t.min(a)),
        (Some(t), None) => Some(t),
        (None, Some(a)) => Some(a),
        (None, None) => None,
    }
}

/// Sleep until a deadline, or wait forever if None.
/// This is used as a conditional branch in tokio::select!
async fn sleep_until_optional(deadline: Option<Instant>) {
    match deadline {
        Some(d) => sleep_until(tokio::time::Instant::from_std(d)).await,
        None => std::future::pending::<()>().await,
    }
}

/// Run the main event loop with an instance registry.
pub async fn run_event_loop(
    registry: Arc<RwLock<InstanceRegistry>>,
    app_keybinds: Arc<RwLock<Keybinds>>,
    mut cx: AppContext,
    theme: Arc<dyn Theme>,
    animation_fps: u16,
    reduce_motion: bool,
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
    let mut state = EventLoopState::new(theme, systems, reduce_motion);

    // Create wakeup channel for passive rendering
    let (wakeup_tx, mut wakeup_rx) = channel();
    wakeup::install_sender(wakeup_tx.clone());
    let wakeup_sender = wakeup_tx.clone();
    cx.set_wakeup_sender(wakeup_tx);

    // Create async event stream
    let mut events = EventStream::new();

    // Create animation interval for active rendering mode
    let frame_duration = Duration::from_secs_f64(1.0 / animation_fps as f64);
    let mut animation_interval = tokio::time::interval(frame_duration);
    animation_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // Track last rendered hit map for event dispatch
    let mut hit_map = HitTestMap::new();

    // Call on_start and on_foreground for the initial instance
    {
        let reg = registry.read().unwrap();
        if let Some(instance) = reg.focused_instance() {
            // Install wakeup sender so State updates trigger re-render
            instance.install_wakeup(wakeup_sender.clone());
            info!("App started: {}", instance.config().name);
            instance.on_start(&cx);
            instance.on_foreground(&cx);
        }
    }

    // Flag to force initial render
    let mut force_render = true;

    // Main event loop
    loop {

        // Check if exit was requested (by a handler from previous iteration)
        if cx.is_exit_requested() {
            info!("Exit requested by handler");
            break;
        }

        // Process pending instance commands
        let cmd_start = Instant::now();
        let cmd_result = process_instance_commands(&registry, &app_keybinds, &state.systems, &cx, &wakeup_sender);
        if cmd_result.should_exit {
            info!("All instances closed, exiting");
            break;
        }
        // If commands were processed, force a render to show the new state
        if cmd_result.needs_render {
            force_render = true;
        }
        let cmd_elapsed = cmd_start.elapsed();
        if cmd_elapsed.as_millis() > 2 {
            warn!("PROFILE: process_instance_commands() took {:?}", cmd_elapsed);
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

        // Check for view changes (instance change or modal stack change)
        // Used to clear animation previous_styles to avoid stale transitions
        let current_view = format!("{}:{}", focused_id, state.modal_stack.len());
        if state.last_view.as_ref() != Some(&current_view) {
            if state.last_view.is_some() {
                debug!("View changed, clearing previous_styles for transition detection");
                state.previous_styles.clear();
            }
            state.last_view = Some(current_view);
        }

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
            let created_at = Instant::now();
            info!("Toast: {}", toast.title);
            state.active_toasts.push((toast, created_at));
        }

        // Remove expired toasts (accounting for activation time and slide-out animation)
        let now = Instant::now();
        let toasts_for_calc = state.active_toasts.clone();
        state.active_toasts = state.active_toasts
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                let removal_time = calculate_toast_removal_time(*i, &toasts_for_calc);
                removal_time > now
            })
            .map(|(_, t)| t.clone())
            .collect();

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

        // Determine if we need to render
        let modal_dirty = state.modal_stack.iter().any(|e| e.modal.is_dirty());
        let toast_dirty = state.active_toasts.len() != state.last_toast_count;
        let toast_animating = has_animating_toasts(&state.active_toasts, Instant::now());
        state.last_toast_count = state.active_toasts.len();
        let instance_is_dirty = instance.is_dirty();

        // Check if focus changed
        let current_focused_id = state.focused_id();
        let focus_changed = current_focused_id != state.last_focused_id;
        state.last_focused_id = current_focused_id;

        // Determine if we need to render
        // We render when:
        // - force_render is set (initial render)
        // - Instance state changed
        // - Modal state changed (dirty or closed)
        // - Toast count changed or animating
        // - Focus changed
        // - We dispatched an event in the previous iteration
        let needs_render = force_render
            || instance_is_dirty
            || modal_dirty
            || toast_dirty
            || toast_animating
            || modal_closed
            || focus_changed
            || state.event_dispatched;

        debug!(
            "Render check: needs={} (force={}, inst_dirty={}, modal_dirty={}, toast_dirty={}, modal_closed={}, focus_changed={}, event_dispatched={})",
            needs_render, force_render, instance_is_dirty, modal_dirty, toast_dirty, modal_closed, focus_changed, state.event_dispatched
        );

        // Get theme and focus info (needed for render and reference)
        let theme = &state.current_theme;
        let focused_element_id = state.focused_id();

        // RENDER - only if something changed
        if needs_render {
            debug!("RENDERING frame");
            // Clear hit map for rebuild
            hit_map.clear();

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

            // Split borrows for animation state (needed inside render closure)
            let animations = &mut state.animations;
            let previous_styles = &mut state.previous_styles;
            let modal_stack_ref = &state.modal_stack;
            let active_toasts_ref = &state.active_toasts;
            
            // Will collect active overlays during render
            let mut active_overlays_result: Vec<ActiveOverlay> = Vec::new();
            
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
                    animations,
                    previous_styles,
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
                        animations,
                        previous_styles,
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
                        animations,
                        previous_styles,
                    );

                    // Track active overlay for click-outside detection
                    active_overlays.push(ActiveOverlay::new(request.owner_id, overlay_area));
                }

                // Store active overlays for later assignment
                active_overlays_result = active_overlays;

                // Render toasts on top of everything (with current time for animations)
                let now = Instant::now();
                render_toasts(frame, active_toasts_ref, theme.as_ref(), now);
            })?;
            
            // Update active overlays from render result
            state.active_overlays = active_overlays_result;
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

            // Animation cleanup after render
            // 1. Remove animations for widgets/containers that are no longer in the render tree
            // We use previous_styles keys since that tracks all IDs with transitions enabled
            // (both widgets and containers). This is more complete than hit_map which only tracks widgets.
            state.animations.cleanup_removed_widgets(state.previous_styles.keys().map(|s| s.as_str()));
            // 2. Remove completed (finite) animations
            let completed = state.animations.cleanup_completed();
            if completed > 0 {
                debug!("Cleaned up {} completed animations", completed);
            }

            // Clear render trigger flags
            force_render = false;
            state.event_dispatched = false;
        }

        // Drop the registry lock before waiting for events
        drop(reg);

        // Compute next deadline for toasts and animations
        let next_deadline = compute_next_deadline(&state.active_toasts, &state.animations);

        // Check if any animations are active (style animations OR toast slide-in)
        let has_active_animations = state.animations.has_active()
            || has_animating_toasts(&state.active_toasts, Instant::now());

        // Wait for something to happen (passive OR active rendering)
        let received_event: Option<Event> = tokio::select! {
            // Branch 1: Crossterm event from terminal
            Some(event_result) = events.next() => {
                match event_result {
                    Ok(crossterm_event) => {
                        trace!("Crossterm event: {:?}", crossterm_event);
                        if let Some(rafter_event) = convert_event(crossterm_event) {
                            debug!("Rafter event: {:?}", rafter_event);

                            // Handle hover coalescing specially
                            match rafter_event {
                                Event::Hover(position) => {
                                    let (final_position, other_events, skipped) =
                                        coalesce_hover_events(&mut events, position).await;

                                    if skipped > 0 {
                                        debug!(
                                            "Hover coalesced: skipped {} events, final pos ({}, {})",
                                            skipped, final_position.x, final_position.y
                                        );
                                    }

                                    if !other_events.is_empty() {
                                        debug!(
                                            "Hover coalescing dropped {} non-hover events",
                                            other_events.len()
                                        );
                                    }

                                    Some(Event::Hover(final_position))
                                }
                                other => Some(other),
                            }
                        } else {
                            None
                        }
                    }
                    Err(e) => {
                        log::error!("Event stream error: {}", e);
                        None
                    }
                }
            }

            // Branch 2: Wakeup signal (state changed from async task, etc.)
            Some(()) = wakeup_rx.recv() => {
                debug!("SELECT: wakeup signal received");
                // Drain any additional wakeup signals that arrived while we were processing
                // Multiple wakeups should collapse into a single render
                wakeup_rx.drain();
                // Force render - the wakeup means something changed
                force_render = true;
                None // No event to dispatch, just re-check state
            }

            // Branch 3: Deadline reached (toast expiry, future: animation completion)
            _ = sleep_until_optional(next_deadline) => {
                debug!("SELECT: deadline reached");
                None // No event to dispatch, toasts will be cleaned up next iteration
            }

            // Branch 4: Animation tick (only enabled when animations are active)
            _ = animation_interval.tick(), if has_active_animations => {
                debug!("SELECT: animation tick");
                // Force render to show animation progress
                force_render = true;
                None
            }
        };

        // Dispatch event if we received one
        if let Some(event_to_dispatch) = received_event {
            debug!("DISPATCH: {:?}", event_to_dispatch.name());

            // Handle terminal focus events for animation lifecycle
            match &event_to_dispatch {
                Event::FocusLost => {
                    debug!("Terminal lost focus - pausing/stopping animations");
                    state.animations.on_blur();
                    force_render = true;
                }
                Event::FocusGained => {
                    debug!("Terminal gained focus - resuming animations");
                    state.animations.on_foreground();
                    force_render = true;
                }
                _ => {}
            }
            
            // For hover events, only trigger a render if focus actually changes
            // Track focus before dispatching
            let focus_before_dispatch = if matches!(event_to_dispatch, Event::Hover(_)) {
                Some(state.focused_id())
            } else {
                None
            };

            // Re-acquire registry lock for event dispatch
            let reg = registry.read().unwrap();
            if let Some(instance) = reg.focused_instance() {
                // Get current page for dispatch
                let page = if let Some(entry) = state.modal_stack.last() {
                    entry.modal.page()
                } else {
                    instance.page()
                };

                if dispatch_event(
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

            // Mark that we dispatched an event (triggers render on next iteration)
            // For hover events, only set this if focus changed (otherwise focus_changed flag will handle it)
            if let Some(focus_before) = focus_before_dispatch {
                let focus_after = state.focused_id();
                if focus_before != focus_after {
                    // Focus changed due to hover - this will be caught by focus_changed flag
                    // Don't set event_dispatched
                } else {
                    // Hover didn't change focus - no need to render
                }
            } else {
                // Not a hover event - always trigger render
                state.event_dispatched = true;
            }

        }
    }

    // Cleanup wakeup sender
    wakeup::uninstall_sender();

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
