//! Rafter runtime - manages the event loop, rendering, and app lifecycle.

mod event_loop;
mod events;
pub mod hit_test;
mod input;
mod modal;
pub(crate) mod render;
mod terminal;

use std::io;
use std::sync::Arc;

use log::info;

use crate::app::{App, PanicBehavior};
use crate::theme::{DefaultTheme, Theme};

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
    pub async fn run<A: App>(self, app: A) -> Result<(), RuntimeError> {
        info!("Runtime starting");

        // Initialize terminal
        let mut term_guard = TerminalGuard::new()?;
        info!("Terminal initialized");

        // Run the event loop
        event_loop::run_event_loop(app, self.theme, &mut term_guard).await
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
