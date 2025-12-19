//! Rafter runtime - manages the event loop, rendering, and app lifecycle.

mod events;
mod input;
mod render;
mod terminal;

use std::io;
use std::time::Duration;

use crossterm::event;

use crate::app::{App, PanicBehavior};
use crate::context::AppContext;
use crate::keybinds::HandlerId;

use events::{Event, convert_event};
use input::{InputState, KeybindMatch};
use render::render_node;
use terminal::TerminalGuard;

/// Rafter runtime - the main entry point for running apps.
pub struct Runtime {
    /// Panic behavior for unhandled panics
    panic_behavior: PanicBehavior,
    /// Error handler callback
    error_handler: Option<Box<dyn Fn(RuntimeError) + Send + Sync>>,
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
        }
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
        self.run(Box::new(app)).await
    }

    /// Start the runtime with a boxed app
    pub async fn run(self, app: Box<dyn App>) -> Result<(), RuntimeError> {
        // Initialize terminal
        let mut term_guard = TerminalGuard::new()?;

        // Create app context
        let mut cx = AppContext::new();

        // Create input state for keybind sequence tracking
        let mut input_state = InputState::new();

        // Get initial keybinds
        let keybinds = app.keybinds();

        // Create mutable app state
        let mut app = app;

        // Call on_start
        app.on_start(&mut cx);

        // Main event loop
        loop {
            // Render
            term_guard.terminal().draw(|frame| {
                let area = frame.area();
                let node = app.view();
                render_node(frame, &node, area);
            })?;

            // Clear dirty flags after render
            app.clear_dirty();

            // Wait for events (with timeout for animations later)
            if event::poll(Duration::from_millis(100))?
                && let Ok(crossterm_event) = event::read()
                && let Some(rafter_event) = convert_event(crossterm_event)
            {
                match rafter_event {
                    Event::Quit => {
                        // Exit requested
                        break;
                    }
                    Event::Key(key_combo) => {
                        // Process keybind
                        match input_state.process_key(key_combo, &keybinds) {
                            KeybindMatch::Match(handler_id) => {
                                // Dispatch to handler
                                dispatch_handler(&mut app, &handler_id, &mut cx);

                                // Check if exit was requested
                                if cx.is_exit_requested() {
                                    break;
                                }
                            }
                            KeybindMatch::Pending => {
                                // Waiting for more keys in sequence
                            }
                            KeybindMatch::NoMatch => {
                                // No keybind matched
                            }
                        }
                    }
                    Event::Resize { .. } => {
                        // Terminal resized - will re-render on next loop
                    }
                    Event::Click(_) | Event::Scroll(_) => {
                        // Mouse events - not implemented yet
                    }
                }
            }
        }

        // Call on_stop
        app.on_stop(&mut cx);

        Ok(())
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

/// Dispatch a handler by its ID.
fn dispatch_handler(app: &mut Box<dyn App>, handler_id: &HandlerId, cx: &mut AppContext) {
    app.dispatch(handler_id, cx);
}
