//! Main event loop for the runtime.

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crossterm::event;
use log::{debug, info, trace, warn};

use crate::app::App;
use crate::context::AppContext;
use crate::focus::FocusId;
use crate::theme::Theme;

use super::RuntimeError;
use super::events::{Event, convert_event};
use super::handlers::dispatch_event;
use super::hit_test::HitTestMap;
use super::input::InputState;
use super::modal::{ModalStackEntry, calculate_modal_area};
use super::render::{dim_backdrop, fill_background, render_node, render_toasts};
use super::state::EventLoopState;
use super::terminal::TerminalGuard;

use crate::events::Position;
use crate::focus::FocusState;

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

/// Run the main event loop for an app.
pub async fn run_event_loop<A: App>(
    app: A,
    theme: Arc<dyn Theme>,
    term_guard: &mut TerminalGuard,
) -> Result<(), RuntimeError> {
    // Get initial keybinds and wrap in Arc<RwLock<>> for runtime mutation
    let app_keybinds = Arc::new(RwLock::new(app.keybinds()));
    {
        let kb = app_keybinds.read().unwrap();
        info!("Registered {} keybinds", kb.all().len());
        for bind in kb.all() {
            debug!(
                "  Keybind: {} ({:?}) => {:?}",
                bind.id, bind.default_keys, bind.handler
            );
        }
    }

    // Create app context with shared keybinds
    let cx = AppContext::new(app_keybinds.clone());

    // Initialize event loop state
    let mut state = EventLoopState::new(theme);

    // Call on_start (async)
    app.on_start(&cx).await;
    info!("App started: {}", app.name());

    // Main event loop
    loop {
        // Check if exit was requested (by a handler from previous iteration)
        if cx.is_exit_requested() {
            info!("Exit requested by handler");
            break;
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
            let expiry = Instant::now() + toast.duration;
            info!("Toast: {}", toast.message);
            state.active_toasts.push((toast, expiry));
        }

        // Remove expired toasts
        let now = Instant::now();
        state.active_toasts.retain(|(_, expiry)| *expiry > now);

        // Get the page tree for app or top modal
        let page = if let Some(entry) = state.modal_stack.last() {
            entry.modal.page()
        } else {
            app.page()
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
        let focused_id = state.focused_id();

        // Cache app page (computed once per frame)
        let view_start = Instant::now();
        let app_view = app.page();
        let view_elapsed = view_start.elapsed();
        if view_elapsed.as_millis() > 5 {
            warn!("PROFILE: app.page() took {:?}", view_elapsed);
        }

        // Cache modal views (computed once per frame)
        let modal_views: Vec<_> = state.modal_stack.iter().map(|e| e.modal.page()).collect();

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
                focused_id.as_deref()
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
                    focused_id.as_deref()
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
                );
            }

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
            app.clear_dirty();
        }

        // Determine poll timeout - skip waiting if state changed
        let needs_immediate_update = state.needs_immediate_update(&app, modal_closed);
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
                if dispatch_event(
                    &event_to_dispatch,
                    &page,
                    &hit_map,
                    &app,
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

    // Call on_stop (async)
    app.on_stop(&cx).await;
    info!("App stopped");

    Ok(())
}
