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

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::io;
use std::sync::{Arc, RwLock};

use log::info;

use crate::app::{AnyAppInstance, App, AppInstance, InstanceRegistry, PanicBehavior};
use crate::context::AppContext;
use crate::styling::theme::{DefaultTheme, Theme};

use terminal::TerminalGuard;

/// Type alias for the global data store.
pub type DataStore = HashMap<TypeId, Arc<dyn Any + Send + Sync>>;

/// Rafter runtime - the main entry point for running apps.
pub struct Runtime {
    /// Panic behavior for unhandled panics
    panic_behavior: PanicBehavior,
    /// Error handler callback
    error_handler: Option<Box<dyn Fn(RuntimeError) + Send + Sync>>,
    /// Current theme
    theme: Arc<dyn Theme>,
    /// Initial app instance to spawn on startup
    initial_app: Option<Box<dyn AnyAppInstance>>,
    /// Global data store (type-erased, keyed by TypeId)
    data: DataStore,
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
    /// Create a new runtime builder.
    pub fn new() -> Self {
        Self {
            panic_behavior: PanicBehavior::default(),
            error_handler: None,
            theme: Arc::new(DefaultTheme::default()),
            initial_app: None,
            data: HashMap::new(),
        }
    }

    /// Add global data accessible via `cx.data::<T>()`.
    ///
    /// Data is stored by type and can be retrieved from any app or system handler.
    /// Users are responsible for their own synchronization if the data needs to be
    /// mutated (e.g., wrap in `Arc<Mutex<T>>`).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let client = ApiClient::new();
    ///
    /// Runtime::new()
    ///     .data(client)
    ///     .initial::<MyApp>()
    ///     .run()
    ///     .await?;
    ///
    /// // In a handler:
    /// #[handler]
    /// async fn fetch(&self, cx: &AppContext) {
    ///     let client = cx.data::<ApiClient>();
    ///     let result = client.get("/endpoint").await;
    /// }
    /// ```
    pub fn data<T: Send + Sync + 'static>(mut self, value: T) -> Self {
        self.data.insert(TypeId::of::<T>(), Arc::new(value));
        self
    }

    /// Set the theme.
    pub fn theme<T: Theme>(mut self, theme: T) -> Self {
        self.theme = Arc::new(theme);
        self
    }

    /// Set the default panic behavior.
    pub fn on_panic(mut self, behavior: PanicBehavior) -> Self {
        self.panic_behavior = behavior;
        self
    }

    /// Set the error handler.
    pub fn on_error<F>(mut self, handler: F) -> Self
    where
        F: Fn(RuntimeError) + Send + Sync + 'static,
    {
        self.error_handler = Some(Box::new(handler));
        self
    }

    /// Set the initial app to spawn on startup (using Default).
    ///
    /// # Example
    ///
    /// ```ignore
    /// Runtime::new()
    ///     .initial::<MyApp>()
    ///     .run()
    ///     .await?;
    /// ```
    pub fn initial<A: App + Default>(mut self) -> Self {
        let app = A::default();
        let instance = AppInstance::new(app);
        self.initial_app = Some(Box::new(instance));
        self
    }

    /// Set the initial app to spawn on startup (with a pre-constructed instance).
    ///
    /// Use this when your app requires custom initialization.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let app = MyApp::new(some_config);
    /// Runtime::new()
    ///     .initial_with(app)
    ///     .run()
    ///     .await?;
    /// ```
    pub fn initial_with<A: App>(mut self, app: A) -> Self {
        let instance = AppInstance::new(app);
        self.initial_app = Some(Box::new(instance));
        self
    }

    /// Start the runtime.
    ///
    /// Requires an initial app to be set via `initial()` or `initial_with()`.
    ///
    /// # Panics
    ///
    /// Panics if no initial app was set.
    pub async fn run(self) -> Result<(), RuntimeError> {
        let initial_instance = self
            .initial_app
            .expect("No initial app set. Use .initial::<App>() or .initial_with(app) before .run()");

        info!("Runtime starting");

        // Initialize terminal
        let mut term_guard = TerminalGuard::new()?;
        info!("Terminal initialized");

        // Create the instance registry
        let registry = Arc::new(RwLock::new(InstanceRegistry::new()));

        // Add the initial app instance to registry
        let instance_id = initial_instance.id();
        let keybinds = initial_instance.keybinds();

        {
            let mut reg = registry.write().unwrap();
            reg.insert(instance_id, initial_instance);
            reg.focus(instance_id);
        }

        // Create app context with shared keybinds, registry, and data
        let app_keybinds = Arc::new(RwLock::new(keybinds));
        let data = Arc::new(self.data);
        let mut cx = AppContext::new(app_keybinds.clone(), data);
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
