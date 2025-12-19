//! Main event loop for the runtime.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event;
use log::{debug, info, trace};

use crate::app::App;
use crate::context::{AppContext, Toast};
use crate::focus::{FocusId, FocusState};
use crate::keybinds::{HandlerId, Key};
use crate::theme::Theme;

use super::events::{Event, convert_event};
use super::hit_test::HitTestMap;
use super::input::{InputState, KeybindMatch};
use super::modal::{ModalStackEntry, calculate_modal_area};
use super::render::{dim_backdrop, fill_background, render_node, render_toasts};
use super::terminal::TerminalGuard;
use super::RuntimeError;

/// Run the main event loop for an app.
pub async fn run_event_loop<A: App>(
    app: A,
    theme: Arc<dyn Theme>,
    term_guard: &mut TerminalGuard,
) -> Result<(), RuntimeError> {
    // Create app context (now Clone + interior mutable)
    let cx = AppContext::new();

    // Create input state for keybind sequence tracking
    let mut app_input_state = InputState::new();

    // Create focus state
    let mut app_focus_state = FocusState::new();

    // Active toasts with their expiration times
    let mut active_toasts: Vec<(Toast, Instant)> = Vec::new();

    // Modal stack
    let mut modal_stack: Vec<ModalStackEntry> = Vec::new();

    // Get initial keybinds
    let app_keybinds = app.keybinds();
    info!("Registered {} keybinds", app_keybinds.all().len());
    for bind in app_keybinds.all() {
        debug!("  Keybind: {:?} => {:?}", bind.keys, bind.handler);
    }

    // Call on_start (async)
    app.on_start(&cx).await;
    info!("App started: {}", app.name());

    // Track the currently focused input's value for text editing
    let mut app_input_buffer: String = String::new();

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
                input_buffer: String::new(),
                keybinds,
            });
        }

        // Remove closed modals from the stack
        modal_stack.retain(|entry| !entry.modal.is_closed());

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
        let focusable_ids: Vec<FocusId> =
            view.focusable_ids().into_iter().map(FocusId::new).collect();

        // Update focus state for the active layer
        let focus_changed = if let Some(entry) = modal_stack.last_mut() {
            let changed = entry.focus_state.take_focus_changed();
            entry.focus_state.set_focusable_ids(focusable_ids);
            changed || entry.focus_state.take_focus_changed()
        } else {
            let changed = app_focus_state.take_focus_changed();
            app_focus_state.set_focusable_ids(focusable_ids);
            changed || app_focus_state.take_focus_changed()
        };

        // Sync input buffer if focus changed to an input
        if focus_changed {
            let focused_id = if let Some(entry) = modal_stack.last() {
                entry.focus_state.current().map(|f| f.0.clone())
            } else {
                app_focus_state.current().map(|f| f.0.clone())
            };
            if let Some(ref focused_id) = focused_id {
                if view.element_captures_input(focused_id) {
                    let input_buffer = if let Some(entry) = modal_stack.last_mut() {
                        &mut entry.input_buffer
                    } else {
                        &mut app_input_buffer
                    };
                    if input_buffer.is_empty() {
                        if let Some(value) = view.input_value(focused_id) {
                            *input_buffer = value;
                            debug!("Initial sync input buffer: {}", input_buffer);
                        }
                    }
                }
            }
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
                let modal_focused = if i == modal_stack_ref.len() - 1 {
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

        // Wait for events (with timeout for animations/toast expiry)
        if event::poll(Duration::from_millis(100))? {
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
                                // Sync input buffer with new focused element's value
                                let input_buffer = if let Some(entry) = modal_stack.last_mut() {
                                    &mut entry.input_buffer
                                } else {
                                    &mut app_input_buffer
                                };
                                input_buffer.clear();
                                let focus_state = if let Some(entry) = modal_stack.last() {
                                    &entry.focus_state
                                } else {
                                    &app_focus_state
                                };
                                if let Some(focused_id) = focus_state.current() {
                                    if view.element_captures_input(&focused_id.0) {
                                        if let Some(value) = view.input_value(&focused_id.0) {
                                            let input_buffer = if let Some(entry) = modal_stack.last_mut() {
                                                &mut entry.input_buffer
                                            } else {
                                                &mut app_input_buffer
                                            };
                                            *input_buffer = value;
                                            debug!("Synced input buffer: {}", input_buffer);
                                        }
                                    }
                                }
                                continue;
                            }

                            // Handle Enter key for focused elements
                            let current_focus = if let Some(entry) = modal_stack.last() {
                                entry.focus_state.current().map(|f| f.0.clone())
                            } else {
                                app_focus_state.current().map(|f| f.0.clone())
                            };
                            if key_combo.key == Key::Enter {
                                if let Some(ref current) = current_focus {
                                    debug!("Enter on focused element: {:?}", current);
                                    // Get handler from view tree
                                    if let Some(handler_id) = view.get_submit_handler(current) {
                                        let input_buffer = if let Some(entry) = modal_stack.last() {
                                            &entry.input_buffer
                                        } else {
                                            &app_input_buffer
                                        };
                                        cx.set_input_text(input_buffer.clone());
                                        if let Some(entry) = modal_stack.last() {
                                            entry.modal.dispatch_dyn(&handler_id, &cx);
                                        } else {
                                            dispatch_handler(&app, &handler_id, &cx);
                                        }
                                        cx.clear_input_text();
                                        let input_buffer = if let Some(entry) = modal_stack.last_mut() {
                                            &mut entry.input_buffer
                                        } else {
                                            &mut app_input_buffer
                                        };
                                        input_buffer.clear();
                                    }
                                    continue;
                                }
                            }

                            // Handle Escape to clear focus/input (but NOT close modal - explicit only)
                            if key_combo.key == Key::Escape {
                                debug!("Escape pressed, clearing input buffer");
                                if let Some(entry) = modal_stack.last_mut() {
                                    entry.input_buffer.clear();
                                    entry.focus_state.clear_focus();
                                } else {
                                    app_input_buffer.clear();
                                    app_focus_state.clear_focus();
                                }
                                continue;
                            }

                            // Check if currently focused element captures text input
                            let is_text_input_focused = if let Some(entry) = modal_stack.last() {
                                entry
                                    .focus_state
                                    .current()
                                    .map(|id| view.element_captures_input(&id.0))
                                    .unwrap_or(false)
                            } else {
                                app_focus_state
                                    .current()
                                    .map(|id| view.element_captures_input(&id.0))
                                    .unwrap_or(false)
                            };

                            // Handle Backspace for text input
                            if key_combo.key == Key::Backspace && is_text_input_focused {
                                let input_buffer = if let Some(entry) = modal_stack.last_mut() {
                                    &mut entry.input_buffer
                                } else {
                                    &mut app_input_buffer
                                };
                                input_buffer.pop();
                                debug!("Backspace, buffer: {}", input_buffer);
                                // Notify of change via on_change handler
                                let current = if let Some(entry) = modal_stack.last() {
                                    entry.focus_state.current().map(|f| f.0.clone())
                                } else {
                                    app_focus_state.current().map(|f| f.0.clone())
                                };
                                if let Some(ref current) = current {
                                    if let Some(handler_id) = view.get_change_handler(current) {
                                        let input_buffer = if let Some(entry) = modal_stack.last() {
                                            &entry.input_buffer
                                        } else {
                                            &app_input_buffer
                                        };
                                        cx.set_input_text(input_buffer.clone());
                                        if let Some(entry) = modal_stack.last() {
                                            entry.modal.dispatch_dyn(&handler_id, &cx);
                                        } else {
                                            dispatch_handler(&app, &handler_id, &cx);
                                        }
                                        cx.clear_input_text();
                                    }
                                }
                                continue;
                            }

                            // Handle character input for focused input fields only
                            if let Key::Char(c) = key_combo.key {
                                if is_text_input_focused
                                    && !key_combo.modifiers.ctrl
                                    && !key_combo.modifiers.alt
                                {
                                    let input_buffer = if let Some(entry) = modal_stack.last_mut() {
                                        &mut entry.input_buffer
                                    } else {
                                        &mut app_input_buffer
                                    };
                                    input_buffer.push(c);
                                    debug!("Char input '{}', buffer: {}", c, input_buffer);
                                    // Notify of change via on_change handler
                                    let current = if let Some(entry) = modal_stack.last() {
                                        entry.focus_state.current().map(|f| f.0.clone())
                                    } else {
                                        app_focus_state.current().map(|f| f.0.clone())
                                    };
                                    if let Some(ref current) = current {
                                        if let Some(handler_id) = view.get_change_handler(current) {
                                            let input_buffer = if let Some(entry) = modal_stack.last() {
                                                &entry.input_buffer
                                            } else {
                                                &app_input_buffer
                                            };
                                            cx.set_input_text(input_buffer.clone());
                                            if let Some(entry) = modal_stack.last() {
                                                entry.modal.dispatch_dyn(&handler_id, &cx);
                                            } else {
                                                dispatch_handler(&app, &handler_id, &cx);
                                            }
                                            cx.clear_input_text();
                                        }
                                    }
                                    continue;
                                }
                            }

                            // Process keybind (only if not handled above)
                            let keybind_match = if let Some(entry) = modal_stack.last_mut() {
                                entry
                                    .input_state
                                    .process_key(key_combo.clone(), &entry.keybinds)
                            } else {
                                app_input_state.process_key(key_combo.clone(), &app_keybinds)
                            };
                            match keybind_match {
                                KeybindMatch::Match(handler_id) => {
                                    info!("Keybind matched: {:?}", handler_id);
                                    // Dispatch to modal or app
                                    if let Some(entry) = modal_stack.last() {
                                        entry.modal.dispatch_dyn(&handler_id, &cx);
                                    } else {
                                        dispatch_handler(&app, &handler_id, &cx);
                                    }
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

                            // Hit test to find clicked element
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

                                // If it's an input, sync the buffer with the current value
                                if hit_box.captures_input {
                                    let input_buffer = if let Some(entry) = modal_stack.last_mut() {
                                        &mut entry.input_buffer
                                    } else {
                                        &mut app_input_buffer
                                    };
                                    input_buffer.clear();
                                    if let Some(value) = view.input_value(&hit_box.id) {
                                        *input_buffer = value;
                                        debug!("Synced input buffer on click: {}", input_buffer);
                                    }
                                } else {
                                    // It's a button - dispatch click handler from view
                                    if let Some(handler_id) = view.get_submit_handler(&hit_box.id) {
                                        if let Some(entry) = modal_stack.last() {
                                            entry.modal.dispatch_dyn(&handler_id, &cx);
                                        } else {
                                            dispatch_handler(&app, &handler_id, &cx);
                                        }
                                    }
                                }
                            }
                        }
                        Event::Hover(ref position) => {
                            // Hit test to find hovered element
                            if let Some(hit_box) = hit_map.hit_test(position.x, position.y) {
                                // Only update focus if hovering a different element
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

                                    // If it's an input, sync the buffer with the current value
                                    if hit_box.captures_input {
                                        let input_buffer =
                                            if let Some(entry) = modal_stack.last_mut() {
                                                &mut entry.input_buffer
                                            } else {
                                                &mut app_input_buffer
                                            };
                                        input_buffer.clear();
                                        if let Some(value) = view.input_value(&hit_box.id) {
                                            *input_buffer = value;
                                            debug!(
                                                "Synced input buffer on hover: {}",
                                                input_buffer
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        Event::Scroll(_) => {
                            // Scroll events - not implemented yet
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

/// Dispatch a handler by its ID.
/// The handler is spawned as an async task by the app's dispatch implementation.
fn dispatch_handler<A: App>(app: &A, handler_id: &HandlerId, cx: &AppContext) {
    app.dispatch(handler_id, cx);
}
