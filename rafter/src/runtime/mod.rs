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

use crate::app::{App, PanicBehavior};
use crate::context::{AppContext, Toast};
use crate::focus::{FocusId, FocusState};
use crate::keybinds::{HandlerId, Key};
use crate::theme::{DefaultTheme, Theme};

use events::{Event, convert_event};
use hit_test::HitTestMap;
use input::{InputState, KeybindMatch};
use render::{render_node, render_toasts};
use terminal::TerminalGuard;

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
        let mut input_state = InputState::new();

        // Create focus state
        let mut focus_state = FocusState::new();

        // Active toasts with their expiration times
        let mut active_toasts: Vec<(Toast, Instant)> = Vec::new();

        // Get initial keybinds
        let keybinds = app.keybinds();
        info!("Registered {} keybinds", keybinds.all().len());
        for bind in keybinds.all() {
            debug!("  Keybind: {:?} => {:?}", bind.keys, bind.handler);
        }

        // Call on_start (async)
        app.on_start(&cx).await;
        info!("App started: {}", app.name());

        // Track the currently focused input's value for text editing
        let mut input_buffer: String = String::new();

        // Main event loop
        loop {
            // Check if exit was requested (by a handler from previous iteration)
            if cx.is_exit_requested() {
                info!("Exit requested by handler");
                break;
            }

            // Process any pending focus requests
            if let Some(focus_id) = cx.take_focus_request() {
                debug!("Focus requested: {:?}", focus_id);
                focus_state.set_focus(focus_id);
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

            // Get the view tree (used for focus, input values, and rendering)
            let view = app.view();

            // Update focusable IDs from view tree
            let focusable_ids: Vec<FocusId> =
                view.focusable_ids().into_iter().map(FocusId::new).collect();
            let focus_changed = focus_state.take_focus_changed();
            focus_state.set_focusable_ids(focusable_ids);

            // Sync input buffer if focus changed to an input
            if (focus_changed || focus_state.take_focus_changed())
                && let Some(focused_id) = focus_state.current()
                && view.element_captures_input(&focused_id.0)
                && input_buffer.is_empty()
                && let Some(value) = view.input_value(&focused_id.0)
            {
                input_buffer = value;
                debug!("Initial sync input buffer: {}", input_buffer);
            }

            // Render and build hit test map
            let mut hit_map = HitTestMap::new();
            let theme = &self.theme;
            let focused_id = focus_state.current().map(|f| f.0.clone());
            term_guard.terminal().draw(|frame| {
                let area = frame.area();
                render_node(
                    frame,
                    &view,
                    area,
                    &mut hit_map,
                    theme.as_ref(),
                    focused_id.as_deref(),
                );

                // Render toasts in bottom-right corner
                render_toasts(frame, &active_toasts, theme.as_ref());
            })?;

            // Clear dirty flags after render
            app.clear_dirty();

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

                            // Handle Tab/Shift+Tab for focus navigation
                            if key_combo.key == Key::Tab {
                                if key_combo.modifiers.shift {
                                    debug!("Focus prev");
                                    focus_state.focus_prev();
                                } else {
                                    debug!("Focus next");
                                    focus_state.focus_next();
                                }
                                // Sync input buffer with new focused element's value
                                input_buffer.clear();
                                if let Some(focused_id) = focus_state.current()
                                    && view.element_captures_input(&focused_id.0)
                                    && let Some(value) = view.input_value(&focused_id.0)
                                {
                                    input_buffer = value;
                                    debug!("Synced input buffer: {}", input_buffer);
                                }
                                continue;
                            }

                            // Handle Enter key for focused elements
                            if key_combo.key == Key::Enter
                                && let Some(current) = focus_state.current()
                            {
                                debug!("Enter on focused element: {:?}", current);
                                // Get handler from view tree
                                if let Some(handler_id) = view.get_submit_handler(&current.0) {
                                    cx.set_input_text(input_buffer.clone());
                                    dispatch_handler(&app, &handler_id, &cx);
                                    cx.clear_input_text();
                                    input_buffer.clear();
                                }
                                continue;
                            }

                            // Handle Escape to clear focus/input
                            if key_combo.key == Key::Escape {
                                debug!("Escape pressed, clearing input buffer");
                                input_buffer.clear();
                                focus_state.clear_focus();
                                continue;
                            }

                            // Check if currently focused element captures text input
                            let is_text_input_focused = focus_state
                                .current()
                                .map(|id| view.element_captures_input(&id.0))
                                .unwrap_or(false);

                            // Handle Backspace for text input
                            if key_combo.key == Key::Backspace && is_text_input_focused {
                                input_buffer.pop();
                                debug!("Backspace, buffer: {}", input_buffer);
                                // Notify app of change via on_change handler
                                if let Some(current) = focus_state.current()
                                    && let Some(handler_id) = view.get_change_handler(&current.0)
                                {
                                    cx.set_input_text(input_buffer.clone());
                                    dispatch_handler(&app, &handler_id, &cx);
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
                                input_buffer.push(c);
                                debug!("Char input '{}', buffer: {}", c, input_buffer);
                                // Notify app of change via on_change handler
                                if let Some(current) = focus_state.current()
                                    && let Some(handler_id) = view.get_change_handler(&current.0)
                                {
                                    cx.set_input_text(input_buffer.clone());
                                    dispatch_handler(&app, &handler_id, &cx);
                                    cx.clear_input_text();
                                }
                                continue;
                            }

                            // Process keybind (only if not handled above)
                            match input_state.process_key(key_combo.clone(), &keybinds) {
                                KeybindMatch::Match(handler_id) => {
                                    info!("Keybind matched: {:?}", handler_id);
                                    // Dispatch to handler (spawns async task)
                                    dispatch_handler(&app, &handler_id, &cx);
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
                                focus_state.set_focus(hit_box.id.clone());

                                // If it's an input, sync the buffer with the current value
                                if hit_box.captures_input {
                                    input_buffer.clear();
                                    if let Some(value) = view.input_value(&hit_box.id) {
                                        input_buffer = value;
                                        debug!("Synced input buffer on click: {}", input_buffer);
                                    }
                                } else {
                                    // It's a button - dispatch click handler from view
                                    if let Some(handler_id) = view.get_submit_handler(&hit_box.id) {
                                        dispatch_handler(&app, &handler_id, &cx);
                                    }
                                }
                            }
                        }
                        Event::Hover(ref position) => {
                            // Hit test to find hovered element
                            if let Some(hit_box) = hit_map.hit_test(position.x, position.y) {
                                // Only update focus if hovering a different element
                                let current_focus = focus_state.current().map(|f| f.0.clone());
                                if current_focus.as_deref() != Some(&hit_box.id) {
                                    debug!("Hover focus: {}", hit_box.id);
                                    focus_state.set_focus(hit_box.id.clone());

                                    // If it's an input, sync the buffer with the current value
                                    if hit_box.captures_input {
                                        input_buffer.clear();
                                        if let Some(value) = view.input_value(&hit_box.id) {
                                            input_buffer = value;
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
