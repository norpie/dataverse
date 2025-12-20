//! Main event loop for the runtime.

use std::sync::{Arc, RwLock};
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
use super::render::{dim_backdrop, fill_background, render_node_with_input, render_toasts};
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
            debug!("  Keybind: {} ({:?}) => {:?}", bind.id, bind.default_keys, bind.handler);
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

    // Track the currently focused input's value for text editing
    let mut app_input_buffer: String = String::new();
    // ID of the input element the app buffer belongs to
    let mut app_input_buffer_id: Option<String> = None;

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
                input_buffer_id: None,
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
                    // Check if buffer already belongs to this input
                    let current_buffer_id = if let Some(entry) = modal_stack.last() {
                        entry.input_buffer_id.as_deref()
                    } else {
                        app_input_buffer_id.as_deref()
                    };
                    
                    let is_same_input = current_buffer_id == Some(focused_id.as_str());
                    
                    if !is_same_input {
                        // New input - sync buffer with its initial value
                        let initial_value = view.input_value(focused_id).unwrap_or_default();
                        if let Some(entry) = modal_stack.last_mut() {
                            entry.input_buffer = initial_value;
                            entry.input_buffer_id = Some(focused_id.clone());
                            debug!("Initial sync input buffer: {}", entry.input_buffer);
                        } else {
                            app_input_buffer = initial_value;
                            app_input_buffer_id = Some(focused_id.clone());
                            debug!("Initial sync input buffer: {}", app_input_buffer);
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

        // Get input buffer and its ID for rendering
        let (render_input_buffer, render_input_id): (Option<String>, Option<String>) = 
            if let Some(entry) = modal_stack.last() {
                (Some(entry.input_buffer.clone()), entry.input_buffer_id.clone())
            } else {
                (Some(app_input_buffer.clone()), app_input_buffer_id.clone())
            };
        let render_input_ref = render_input_buffer.as_deref();
        let render_input_id_ref = render_input_id.as_deref();

        let modal_stack_ref = &modal_stack;
        term_guard.terminal().draw(|frame| {
            let area = frame.area();

            // Fill entire terminal with theme background color
            if let Some(bg_color) = theme.resolve("background") {
                fill_background(frame, bg_color.to_ratatui());
            }

            // Always render the app first (without input buffer if modal is open)
            let app_focused = if modal_stack_ref.is_empty() {
                focused_id.as_deref()
            } else {
                None
            };
            let (app_input, app_input_id) = if modal_stack_ref.is_empty() {
                (render_input_ref, render_input_id_ref)
            } else {
                (None, None)
            };
            render_node_with_input(
                frame,
                &app_view,
                area,
                &mut hit_map,
                theme.as_ref(),
                app_focused,
                app_input,
                app_input_id,
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

                // Only show focus and input buffer for the top modal
                let is_top_modal = i == modal_stack_ref.len() - 1;
                let modal_focused = if is_top_modal {
                    focused_id.as_deref()
                } else {
                    None
                };
                let (modal_input, modal_input_id) = if is_top_modal {
                    (render_input_ref, render_input_id_ref)
                } else {
                    (None, None)
                };

                // Render modal view
                render_node_with_input(
                    frame,
                    modal_view,
                    modal_area,
                    &mut hit_map,
                    theme.as_ref(),
                    modal_focused,
                    modal_input,
                    modal_input_id,
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
                                // Only sync input buffer if new focused element is an input
                                // (don't clear buffer when focusing buttons - we need it for submit)
                                let new_focused_id = if let Some(entry) = modal_stack.last() {
                                    entry.focus_state.current().map(|f| f.0.clone())
                                } else {
                                    app_focus_state.current().map(|f| f.0.clone())
                                };
                                if let Some(ref focused_id) = new_focused_id {
                                    if view.element_captures_input(focused_id) {
                                        // Focusing an input - only sync if this is a DIFFERENT input
                                        // (preserve buffer if returning to the same input we were editing)
                                        let current_buffer_id = if let Some(entry) = modal_stack.last() {
                                            entry.input_buffer_id.as_deref()
                                        } else {
                                            app_input_buffer_id.as_deref()
                                        };
                                        
                                        let is_same_input = current_buffer_id == Some(focused_id.as_str());
                                        
                                        if !is_same_input {
                                            // Different input - sync buffer with its value
                                            let new_value = view.input_value(focused_id);
                                            if let Some(entry) = modal_stack.last_mut() {
                                                if let Some(value) = new_value {
                                                    entry.input_buffer = value;
                                                } else {
                                                    entry.input_buffer.clear();
                                                }
                                                entry.input_buffer_id = Some(focused_id.clone());
                                                debug!("Synced input buffer to new input: {}", entry.input_buffer);
                                            } else {
                                                if let Some(value) = new_value {
                                                    app_input_buffer = value;
                                                } else {
                                                    app_input_buffer.clear();
                                                }
                                                app_input_buffer_id = Some(focused_id.clone());
                                                debug!("Synced input buffer to new input: {}", app_input_buffer);
                                            }
                                        } else {
                                            debug!("Re-focusing same input, keeping buffer");
                                        }
                                    }
                                    // If focusing a button, keep buffer as-is for potential submit
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
                                        debug!("Set input text for handler: {:?}", cx.input_text());
                                        if let Some(entry) = modal_stack.last() {
                                            entry.modal.dispatch_dyn(&handler_id, &cx);
                                        } else {
                                            dispatch_handler(&app, &handler_id, &cx);
                                        }
                                        // Note: Don't clear input_text here - the async handler
                                        // needs to read it. It will be overwritten on next submit.
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
                                // Get the focused input's ID
                                let focused_input_id = current_focus.clone();
                                
                                if let Some(entry) = modal_stack.last_mut() {
                                    entry.input_buffer.pop();
                                    entry.input_buffer_id = focused_input_id.clone();
                                    debug!("Backspace, buffer: {}", entry.input_buffer);
                                } else {
                                    app_input_buffer.pop();
                                    app_input_buffer_id = focused_input_id.clone();
                                    debug!("Backspace, buffer: {}", app_input_buffer);
                                }
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
                                        // Note: Don't clear input_text - async handler needs it
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
                                    // Get the focused input's ID
                                    let focused_input_id = current_focus.clone();
                                    
                                    // Update buffer and track which input it belongs to
                                    if let Some(entry) = modal_stack.last_mut() {
                                        entry.input_buffer.push(c);
                                        entry.input_buffer_id = focused_input_id.clone();
                                        debug!("Char input '{}', buffer: {}", c, entry.input_buffer);
                                    } else {
                                        app_input_buffer.push(c);
                                        app_input_buffer_id = focused_input_id.clone();
                                        debug!("Char input '{}', buffer: {}", c, app_input_buffer);
                                    }
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
                                            // Note: Don't clear input_text - async handler needs it
                                        }
                                    }
                                    continue;
                                }
                            }

                            // Process keybind (only if not handled above)
                            // Get current view for keybind scoping (only for app, modals don't have views)
                            let current_view = if modal_stack.is_empty() {
                                app.current_view()
                            } else {
                                None
                            };
                            let current_view_ref = current_view.as_deref();
                            
                            let keybind_match = if let Some(entry) = modal_stack.last_mut() {
                                // Modals don't have view scoping, pass None
                                entry
                                    .input_state
                                    .process_key(key_combo.clone(), &entry.keybinds, None)
                            } else {
                                // Read keybinds from the shared lock
                                let kb = app_keybinds.read().unwrap();
                                app_input_state.process_key(key_combo.clone(), &kb, current_view_ref)
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
