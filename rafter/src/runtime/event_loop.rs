//! Main event loop for the runtime.

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crossterm::event;
use log::{debug, info, trace};

use crate::app::App;
use crate::context::{AppContext, Toast};
use crate::focus::{FocusId, FocusState};
use crate::keybinds::{HandlerId, Key};
use crate::node::Node;
use crate::theme::Theme;

use super::events::{convert_event, Event};
use super::hit_test::HitTestMap;
use super::input::{InputState, KeybindMatch};
use super::modal::{calculate_modal_area, ModalStackEntry};
use super::render::{dim_backdrop, fill_background, render_node, render_toasts};
use super::terminal::TerminalGuard;
use super::RuntimeError;

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

    // Create input state for keybind sequence tracking
    let mut app_input_state = InputState::new();

    // Create focus state
    let mut app_focus_state = FocusState::new();

    // Active toasts with their expiration times
    let mut active_toasts: Vec<(Toast, Instant)> = Vec::new();

    // Modal stack
    let mut modal_stack: Vec<ModalStackEntry> = Vec::new();

    // Call on_start (async)
    app.on_start(&cx).await;
    info!("App started: {}", app.name());

    // Current theme (mutable for theme switching)
    let mut current_theme = theme;

    // Main event loop
    loop {
        // Check if exit was requested (by a handler from previous iteration)
        if cx.is_exit_requested() {
            info!("Exit requested by handler");
            break;
        }

        // Check for pending modal requests
        if let Some(modal) = cx.take_modal_request() {
            info!("Opening modal: {}", modal.name());
            let keybinds = modal.keybinds();
            modal_stack.push(ModalStackEntry {
                modal,
                focus_state: FocusState::new(),
                input_state: InputState::new(),
                keybinds,
            });
        }

        // Remove closed modals from the stack
        let modal_count_before = modal_stack.len();
        modal_stack.retain(|entry| !entry.modal.is_closed());
        let modal_closed = modal_stack.len() < modal_count_before;

        // Determine if we're in modal mode
        let in_modal = !modal_stack.is_empty();

        // Process any pending focus requests (only for app, not modals)
        if !in_modal {
            if let Some(focus_id) = cx.take_focus_request() {
                debug!("Focus requested: {:?}", focus_id);
                app_focus_state.set_focus(focus_id);
            }
        }

        // Process any pending theme change requests
        if let Some(new_theme) = cx.take_theme_request() {
            info!("Theme changed");
            current_theme = new_theme;
        }

        // Process any pending toasts
        for toast in cx.take_toasts() {
            let expiry = Instant::now() + toast.duration;
            info!("Toast: {}", toast.message);
            active_toasts.push((toast, expiry));
        }

        // Remove expired toasts
        let now = Instant::now();
        active_toasts.retain(|(_, expiry)| *expiry > now);

        // Get the view tree for app or top modal
        let view = if let Some(entry) = modal_stack.last() {
            entry.modal.view()
        } else {
            app.view()
        };

        // Update focusable IDs from view tree
        let focusable_ids: Vec<FocusId> = view
            .focusable_ids()
            .into_iter()
            .map(FocusId::new)
            .collect();

        // Update focus state for the active layer
        if let Some(entry) = modal_stack.last_mut() {
            entry.focus_state.set_focusable_ids(focusable_ids);
        } else {
            app_focus_state.set_focusable_ids(focusable_ids);
        }

        // Render and build hit test map
        let mut hit_map = HitTestMap::new();
        let theme = &current_theme;
        let focused_id = if let Some(entry) = modal_stack.last() {
            entry.focus_state.current().map(|f| f.0.clone())
        } else {
            app_focus_state.current().map(|f| f.0.clone())
        };

        // Cache app view (computed once per frame)
        let app_view = app.view();

        // Cache modal views (computed once per frame)
        let modal_views: Vec<_> = modal_stack.iter().map(|e| e.modal.view()).collect();

        let modal_stack_ref = &modal_stack;
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

                // Render modal view
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
            render_toasts(frame, &active_toasts, theme.as_ref());
        })?;

        // Clear dirty flags after render
        if let Some(entry) = modal_stack.last() {
            entry.modal.clear_dirty();
        } else {
            app.clear_dirty();
        }

        // Determine poll timeout - skip waiting if state changed
        let needs_immediate_update =
            modal_closed || app.is_dirty() || modal_stack.iter().any(|e| e.modal.is_dirty());
        let poll_timeout = if needs_immediate_update {
            Duration::from_millis(0)
        } else {
            Duration::from_millis(100)
        };

        // Wait for events (with timeout for animations/toast expiry)
        if event::poll(poll_timeout)? {
            if let Ok(crossterm_event) = event::read() {
                trace!("Crossterm event: {:?}", crossterm_event);

                if let Some(rafter_event) = convert_event(crossterm_event) {
                    debug!("Rafter event: {:?}", rafter_event);

                    match rafter_event {
                        Event::Quit => {
                            info!("Quit requested via system keybind");
                            break;
                        }
                        Event::Key(ref key_combo) => {
                            debug!("Key event: {:?}", key_combo);

                            // Handle Tab/Shift+Tab for focus navigation
                            if key_combo.key == Key::Tab {
                                let focus_state = if let Some(entry) = modal_stack.last_mut() {
                                    &mut entry.focus_state
                                } else {
                                    &mut app_focus_state
                                };
                                if key_combo.modifiers.shift {
                                    debug!("Focus prev");
                                    focus_state.focus_prev();
                                } else {
                                    debug!("Focus next");
                                    focus_state.focus_next();
                                }
                                continue;
                            }

                            // Get current focus
                            let current_focus = if let Some(entry) = modal_stack.last() {
                                entry.focus_state.current().map(|f| f.0.clone())
                            } else {
                                app_focus_state.current().map(|f| f.0.clone())
                            };

                            // Handle Enter key for focused elements
                            if key_combo.key == Key::Enter {
                                if let Some(ref focus_id) = current_focus {
                                    debug!("Enter on focused element: {:?}", focus_id);
                                    if let Some(handler_id) = view.get_submit_handler(focus_id) {
                                        // For inputs, set the input text from the widget
                                        if let Some(widget) = view.get_input_widget(focus_id) {
                                            cx.set_input_text(widget.value());
                                        }
                                        dispatch_to_layer(
                                            &app,
                                            &modal_stack,
                                            &handler_id,
                                            &cx,
                                        );
                                    }
                                    continue;
                                }
                            }

                            // Handle Escape to clear focus
                            if key_combo.key == Key::Escape {
                                debug!("Escape pressed, clearing focus");
                                if let Some(entry) = modal_stack.last_mut() {
                                    entry.focus_state.clear_focus();
                                } else {
                                    app_focus_state.clear_focus();
                                }
                                continue;
                            }

                            // Check if currently focused element is a text input
                            let focused_input = current_focus
                                .as_ref()
                                .and_then(|id| view.get_input_widget(id));

                            if let Some(input) = focused_input {
                                // Handle text input keys
                                match key_combo.key {
                                    Key::Backspace => {
                                        input.delete_char_before();
                                        notify_input_change(&view, &current_focus, &app, &modal_stack, &cx);
                                        continue;
                                    }
                                    Key::Delete => {
                                        input.delete_char_at();
                                        notify_input_change(&view, &current_focus, &app, &modal_stack, &cx);
                                        continue;
                                    }
                                    Key::Left => {
                                        input.cursor_left();
                                        continue;
                                    }
                                    Key::Right => {
                                        input.cursor_right();
                                        continue;
                                    }
                                    Key::Home => {
                                        input.cursor_home();
                                        continue;
                                    }
                                    Key::End => {
                                        input.cursor_end();
                                        continue;
                                    }
                                    Key::Char(c) if !key_combo.modifiers.ctrl && !key_combo.modifiers.alt => {
                                        input.insert_char(c);
                                        notify_input_change(&view, &current_focus, &app, &modal_stack, &cx);
                                        continue;
                                    }
                                    _ => {}
                                }
                            }

                            // Process keybind (only if not handled above)
                            let current_view = if modal_stack.is_empty() {
                                app.current_view()
                            } else {
                                None
                            };
                            let current_view_ref = current_view.as_deref();

                            let keybind_match = if let Some(entry) = modal_stack.last_mut() {
                                entry
                                    .input_state
                                    .process_key(key_combo.clone(), &entry.keybinds, None)
                            } else {
                                let kb = app_keybinds.read().unwrap();
                                app_input_state.process_key(key_combo.clone(), &kb, current_view_ref)
                            };

                            match keybind_match {
                                KeybindMatch::Match(handler_id) => {
                                    info!("Keybind matched: {:?}", handler_id);
                                    dispatch_to_layer(&app, &modal_stack, &handler_id, &cx);
                                }
                                KeybindMatch::Pending => {
                                    debug!("Keybind pending (sequence in progress)");
                                }
                                KeybindMatch::NoMatch => {
                                    debug!("No keybind matched for key");
                                }
                            }
                        }
                        Event::Resize { width, height } => {
                            debug!("Resize: {}x{}", width, height);
                        }
                        Event::Click(ref click) => {
                            debug!("Click at ({}, {})", click.position.x, click.position.y);

                            if let Some(hit_box) =
                                hit_map.hit_test(click.position.x, click.position.y)
                            {
                                debug!("Clicked element: {}", hit_box.id);

                                // Focus the clicked element
                                if let Some(entry) = modal_stack.last_mut() {
                                    entry.focus_state.set_focus(hit_box.id.clone());
                                } else {
                                    app_focus_state.set_focus(hit_box.id.clone());
                                }

                                // If it's a button, dispatch click handler
                                if !hit_box.captures_input {
                                    if let Some(handler_id) = view.get_submit_handler(&hit_box.id) {
                                        dispatch_to_layer(&app, &modal_stack, &handler_id, &cx);
                                    }
                                }
                            }
                        }
                        Event::Hover(ref position) => {
                            if let Some(hit_box) = hit_map.hit_test(position.x, position.y) {
                                let current_focus = if let Some(entry) = modal_stack.last() {
                                    entry.focus_state.current().map(|f| f.0.clone())
                                } else {
                                    app_focus_state.current().map(|f| f.0.clone())
                                };
                                if current_focus.as_deref() != Some(&hit_box.id) {
                                    debug!("Hover focus: {}", hit_box.id);
                                    if let Some(entry) = modal_stack.last_mut() {
                                        entry.focus_state.set_focus(hit_box.id.clone());
                                    } else {
                                        app_focus_state.set_focus(hit_box.id.clone());
                                    }
                                }
                            }
                        }
                        Event::Scroll(ref scroll) => {
                            debug!("Scroll at ({}, {})", scroll.position.x, scroll.position.y);

                            // Find the scrollable element at the scroll position
                            if let Some(hit_box) =
                                hit_map.hit_test(scroll.position.x, scroll.position.y)
                            {
                                // Look for a scrollable widget
                                if let Some(scrollable) = view.get_scrollable_widget(&hit_box.id) {
                                    let amount = scroll.amount as i16;
                                    match scroll.direction {
                                        crate::events::ScrollDirection::Up => {
                                            scrollable.scroll_by(0, -amount);
                                        }
                                        crate::events::ScrollDirection::Down => {
                                            scrollable.scroll_by(0, amount);
                                        }
                                        crate::events::ScrollDirection::Left => {
                                            scrollable.scroll_by(-amount, 0);
                                        }
                                        crate::events::ScrollDirection::Right => {
                                            scrollable.scroll_by(amount, 0);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Call on_stop (async)
    app.on_stop(&cx).await;
    info!("App stopped");

    Ok(())
}

/// Dispatch a handler to the appropriate layer (modal or app)
fn dispatch_to_layer<A: App>(
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

/// Notify on_change handler if present
fn notify_input_change<A: App>(
    view: &Node,
    current_focus: &Option<String>,
    app: &A,
    modal_stack: &[ModalStackEntry],
    cx: &AppContext,
) {
    if let Some(focus_id) = current_focus {
        if let Some(handler_id) = view.get_change_handler(focus_id) {
            if let Some(widget) = view.get_input_widget(focus_id) {
                cx.set_input_text(widget.value());
            }
            dispatch_to_layer(app, modal_stack, &handler_id, cx);
        }
    }
}
