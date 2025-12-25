//! Rafter runtime - manages the event loop, rendering, and app lifecycle.

pub mod animation;
mod event_loop;
mod events;
mod handlers;
pub mod hit_test;
mod input;
mod modal;
pub(crate) mod render;
mod state;
mod terminal;
pub mod wakeup;

pub use animation::{AnimatedProperty, AnimatedValue, Animation, AnimationManager, Easing};

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::io;
use std::sync::{Arc, RwLock};

use log::info;

use crate::app::{AnyAppInstance, App, AppError, AppInstance, InstanceRegistry, PanicBehavior};
use crate::context::AppContext;
use crate::styling::theme::{DefaultTheme, Theme};

use terminal::TerminalGuard;

/// Type alias for the global data store.
pub type DataStore = HashMap<TypeId, Arc<dyn Any + Send + Sync>>;

/// Rafter runtime - the main entry point for running apps.
pub struct Runtime {
    /// Panic behavior for unhandled panics
    panic_behavior: PanicBehavior,
    /// Error handler callback for app errors (panics, task failures)
    error_handler: Option<Arc<dyn Fn(AppError) + Send + Sync>>,
    /// Current theme
    theme: Arc<dyn Theme>,
    /// Initial app instance to spawn on startup
    initial_app: Option<Box<dyn AnyAppInstance>>,
    /// Global data store (type-erased, keyed by TypeId)
    data: DataStore,
    /// Animation frame rate (frames per second)
    animation_fps: u16,
    /// Disable all animations (accessibility)
    reduce_motion: bool,
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
            animation_fps: 60,
            reduce_motion: false,
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

    /// Set the error handler for app errors.
    ///
    /// Called when a handler panics or an async task fails. This is informational
    /// only - the actual error handling (close, restart, ignore) is determined by
    /// the app's `on_panic` policy.
    ///
    /// # Example
    ///
    /// ```ignore
    /// Runtime::new()
    ///     .on_error(|error| {
    ///         log::error!("App error: {}", error);
    ///     })
    ///     .initial::<MyApp>()
    ///     .run()
    ///     .await?;
    /// ```
    pub fn on_error<F>(mut self, handler: F) -> Self
    where
        F: Fn(AppError) + Send + Sync + 'static,
    {
        self.error_handler = Some(Arc::new(handler));
        self
    }

    /// Set animation frame rate (requires restart to take effect).
    ///
    /// Default: 60fps (16.67ms frame time).
    /// Clamped to range 1-120.
    ///
    /// # Example
    ///
    /// ```ignore
    /// Runtime::new()
    ///     .animation_fps(30)  // Lower FPS for slower machines
    ///     .initial::<MyApp>()
    ///     .run()
    ///     .await?;
    /// ```
    pub fn animation_fps(mut self, fps: u16) -> Self {
        self.animation_fps = fps.clamp(1, 120);
        self
    }

    /// Disable all animations for accessibility.
    ///
    /// When enabled, animations complete instantly (properties jump to final value).
    ///
    /// # Example
    ///
    /// ```ignore
    /// Runtime::new()
    ///     .reduce_motion(true)
    ///     .initial::<MyApp>()
    ///     .run()
    ///     .await?;
    /// ```
    pub fn reduce_motion(mut self, enabled: bool) -> Self {
        self.reduce_motion = enabled;
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
        event_loop::run_event_loop(
            registry,
            app_keybinds,
            cx,
            self.theme,
            self.animation_fps,
            self.reduce_motion,
            self.error_handler,
            &mut term_guard,
        )
        .await
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
