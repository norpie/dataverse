//! Rafter runtime - manages the event loop, rendering, and app lifecycle.

mod events;
pub mod hit_test;
mod input;
mod render;
mod terminal;

use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event;
use log::{debug, info, trace};
use ratatui::layout::Rect;

use crate::app::{App, PanicBehavior};
use crate::context::{AppContext, Toast};
use crate::focus::{FocusId, FocusState};
use crate::keybinds::{HandlerId, Key, Keybinds};
use crate::modal::{ModalDyn, ModalPosition, ModalSize};
use crate::theme::{DefaultTheme, Theme};

use events::{Event, convert_event};
use hit_test::HitTestMap;
use input::{InputState, KeybindMatch};
use render::{dim_backdrop, render_node, render_toasts};
use terminal::TerminalGuard;

/// A modal entry in the modal stack
struct ModalStackEntry {
    /// The modal itself (type-erased)
    modal: Box<dyn ModalDyn>,
    /// Focus state for this modal
    focus_state: FocusState,
    /// Input state for keybind sequences
    input_state: InputState,
    /// Input buffer for text input
    input_buffer: String,
    /// Cached keybinds
    keybinds: Keybinds,
}

/// Rafter runtime - the main entry point for running apps.
pub struct Runtime {
    /// Panic behavior for unhandled panics
    panic_behavior: PanicBehavior,
    /// Error handler callback
    error_handler: Option<Box<dyn Fn(RuntimeError) + Send + Sync>>,
    /// Current theme
    theme: Arc<dyn Theme>,
}

/// Runtime error
#[derive(Debug)]
pub struct RuntimeError {
    /// Error message
    pub message: String,
}

impl RuntimeError {
    /// Create a new runtime error
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for RuntimeError {}

impl From<io::Error> for RuntimeError {
    fn from(err: io::Error) -> Self {
        Self::new(err.to_string())
    }
}

impl Runtime {
    /// Create a new runtime builder
    pub fn new() -> Self {
        Self {
            panic_behavior: PanicBehavior::default(),
            error_handler: None,
            theme: Arc::new(DefaultTheme::default()),
        }
    }

    /// Set the theme
    pub fn theme<T: Theme>(mut self, theme: T) -> Self {
        self.theme = Arc::new(theme);
        self
    }

    /// Set the default panic behavior
    pub fn on_panic(mut self, behavior: PanicBehavior) -> Self {
        self.panic_behavior = behavior;
        self
    }

    /// Set the error handler
    pub fn on_error<F>(mut self, handler: F) -> Self
    where
        F: Fn(RuntimeError) + Send + Sync + 'static,
    {
        self.error_handler = Some(Box::new(handler));
        self
    }

    /// Start the runtime with a specific app
    pub async fn start_with<A: App + Default>(self) -> Result<(), RuntimeError> {
        let app = A::default();
        self.run(app).await
    }

    /// Start the runtime with an app instance
    pub async fn run<A: App>(mut self, app: A) -> Result<(), RuntimeError> {
        info!("Runtime starting");

        // Initialize terminal
        let mut term_guard = TerminalGuard::new()?;
        info!("Terminal initialized");

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
            if !in_modal && let Some(focus_id) = cx.take_focus_request() {
                debug!("Focus requested: {:?}", focus_id);
                app_focus_state.set_focus(focus_id);
            }

            // Process any pending theme change requests
            if let Some(new_theme) = cx.take_theme_request() {
                info!("Theme changed");
                self.theme = new_theme;
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
                if let Some(focused_id) = focused_id
                    && view.element_captures_input(&focused_id)
                {
                    let input_buffer = if let Some(entry) = modal_stack.last_mut() {
                        &mut entry.input_buffer
                    } else {
                        &mut app_input_buffer
                    };
                    if input_buffer.is_empty()
                        && let Some(value) = view.input_value(&focused_id)
                    {
                        *input_buffer = value;
                        debug!("Initial sync input buffer: {}", input_buffer);
                    }
                }
            }

            // Render and build hit test map
            let mut hit_map = HitTestMap::new();
            let theme = &self.theme;
            let focused_id = if let Some(entry) = modal_stack.last() {
                entry.focus_state.current().map(|f| f.0.clone())
            } else {
                app_focus_state.current().map(|f| f.0.clone())
            };
            let modal_stack_ref = &modal_stack;
            term_guard.terminal().draw(|frame| {
                let area = frame.area();

                // Always render the app first
                let app_view = app.view();
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
                for (i, entry) in modal_stack_ref.iter().enumerate() {
                    // Dim the backdrop
                    dim_backdrop(frame.buffer_mut(), 0.4);

                    // Calculate modal area based on position and size
                    let modal_area = calculate_modal_area(
                        area,
                        entry.modal.position(),
                        entry.modal.size(),
                        &entry.modal.view(),
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
                    let modal_view = entry.modal.view();
                    render_node(
                        frame,
                        &modal_view,
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
            if event::poll(Duration::from_millis(100))?
                && let Ok(crossterm_event) = event::read()
            {
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

                            // Get the current view
                            let view = if let Some(entry) = modal_stack.last() {
                                entry.modal.view()
                            } else {
                                app.view()
                            };

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
                                if let Some(focused_id) = focus_state.current()
                                    && view.element_captures_input(&focused_id.0)
                                    && let Some(value) = view.input_value(&focused_id.0)
                                {
                                    let input_buffer = if let Some(entry) = modal_stack.last_mut() {
                                        &mut entry.input_buffer
                                    } else {
                                        &mut app_input_buffer
                                    };
                                    *input_buffer = value;
                                    debug!("Synced input buffer: {}", input_buffer);
                                }
                                continue;
                            }

                            // Handle Enter key for focused elements
                            let current_focus = if let Some(entry) = modal_stack.last() {
                                entry.focus_state.current().map(|f| f.0.clone())
                            } else {
                                app_focus_state.current().map(|f| f.0.clone())
                            };
                            if key_combo.key == Key::Enter
                                && let Some(current) = current_focus
                            {
                                debug!("Enter on focused element: {:?}", current);
                                // Get handler from view tree
                                if let Some(handler_id) = view.get_submit_handler(&current) {
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
                                if let Some(current) = current
                                    && let Some(handler_id) = view.get_change_handler(&current)
                                {
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
                                continue;
                            }

                            // Handle character input for focused input fields only
                            if let Key::Char(c) = key_combo.key
                                && is_text_input_focused
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
                                if let Some(current) = current
                                    && let Some(handler_id) = view.get_change_handler(&current)
                                {
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
                                continue;
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

                            // Get the current view
                            let view = if let Some(entry) = modal_stack.last() {
                                entry.modal.view()
                            } else {
                                app.view()
                            };

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
                            // Get the current view
                            let view = if let Some(entry) = modal_stack.last() {
                                entry.modal.view()
                            } else {
                                app.view()
                            };

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

        // Call on_stop (async)
        app.on_stop(&cx).await;
        info!("App stopped");

        Ok(())
    }
}

/// Calculate the modal's render area based on position and size settings.
fn calculate_modal_area(
    screen: Rect,
    position: ModalPosition,
    size: ModalSize,
    view: &crate::node::Node,
) -> Rect {
    // Calculate dimensions
    let (width, height) = match size {
        ModalSize::Auto => {
            // Use intrinsic size from view
            let w = view.intrinsic_width().min(screen.width.saturating_sub(4));
            let h = view.intrinsic_height().min(screen.height.saturating_sub(4));
            (w.max(10), h.max(3))
        }
        ModalSize::Fixed { width, height } => (width.min(screen.width), height.min(screen.height)),
        ModalSize::Proportional { width, height } => {
            let w = (screen.width as f32 * width) as u16;
            let h = (screen.height as f32 * height) as u16;
            (w.max(10), h.max(3))
        }
    };

    // Calculate position
    let (x, y) = match position {
        ModalPosition::Centered => {
            let x = (screen.width.saturating_sub(width)) / 2;
            let y = (screen.height.saturating_sub(height)) / 2;
            (x, y)
        }
        ModalPosition::At { x, y } => (
            x.min(screen.width.saturating_sub(width)),
            y.min(screen.height.saturating_sub(height)),
        ),
    };

    Rect::new(x, y, width, height)
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

/// Dispatch a handler by its ID.
/// The handler is spawned as an async task by the app's dispatch implementation.
fn dispatch_handler<A: App>(app: &A, handler_id: &HandlerId, cx: &AppContext) {
    app.dispatch(handler_id, cx);
}
