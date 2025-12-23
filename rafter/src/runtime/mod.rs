//! Rafter runtime - manages the event loop, rendering, and app lifecycle.

mod event_loop;
mod events;
mod handlers;
pub mod hit_test;
mod input;
mod modal;
pub(crate) mod render;
mod state;
mod terminal;

use std::io;
use std::sync::{Arc, RwLock};

use log::info;

use crate::app::{AnyAppInstance, App, AppInstance, InstanceRegistry, PanicBehavior};
use crate::context::AppContext;
use crate::styling::theme::{DefaultTheme, Theme};

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

        // Create the instance registry
        let registry = Arc::new(RwLock::new(InstanceRegistry::new()));

        // Create the initial app instance and add to registry
        let instance = AppInstance::new(app);
        let instance_id = instance.id();
        let keybinds = instance.keybinds();

        {
            let mut reg = registry.write().unwrap();
            reg.insert(instance_id, Box::new(instance));
            reg.focus(instance_id);
        }

        // Create app context with shared keybinds and registry
        let app_keybinds = Arc::new(RwLock::new(keybinds));
        let mut cx = AppContext::new(app_keybinds.clone());
        cx.set_registry(registry.clone());
        cx.set_instance_id(instance_id);

        // Run the event loop with the registry
        event_loop::run_event_loop(registry, app_keybinds, cx, self.theme, &mut term_guard).await
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
