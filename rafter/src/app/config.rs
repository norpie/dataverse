//! App configuration types.

use thiserror::Error;

use super::PanicBehavior;

/// Per-app-type configuration.
///
/// This defines the behavior and constraints for all instances of an app type.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Display name for this app type.
    pub name: &'static str,

    /// Behavior when an instance loses focus.
    pub on_blur: BlurPolicy,

    /// Behavior when a handler or lifecycle hook panics.
    pub on_panic: PanicBehavior,

    /// If true, the instance cannot be force-closed by the system.
    /// Useful for background processors that must complete their work.
    pub persistent: bool,

    /// Maximum concurrent instances (None = unlimited).
    /// Set to Some(1) for singleton apps.
    pub max_instances: Option<usize>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            name: "App",
            on_blur: BlurPolicy::Continue,
            on_panic: PanicBehavior::default(),
            persistent: false,
            max_instances: None,
        }
    }
}

impl AppConfig {
    /// Create a new config with the given name.
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            ..Default::default()
        }
    }

    /// Set the blur policy.
    pub fn on_blur(mut self, policy: BlurPolicy) -> Self {
        self.on_blur = policy;
        self
    }

    /// Mark this app as persistent (cannot be force-closed).
    pub fn persistent(mut self) -> Self {
        self.persistent = true;
        self
    }

    /// Set maximum instances (use 1 for singleton).
    pub fn max_instances(mut self, max: usize) -> Self {
        self.max_instances = Some(max);
        self
    }

    /// Make this a singleton app (max 1 instance).
    pub fn singleton(mut self) -> Self {
        self.max_instances = Some(1);
        self
    }
}

/// Behavior when an app instance loses focus.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BlurPolicy {
    /// Keep running async tasks in background.
    #[default]
    Continue,

    /// Pause event delivery and ticking (reduced resource usage).
    /// The instance is not destroyed but enters a dormant state.
    Sleep,

    /// Close the instance when losing focus.
    /// Useful for transient UIs like app launchers.
    Close,
}

/// Errors that can occur when spawning an app instance.
#[derive(Debug, Clone, Error)]
pub enum SpawnError {
    /// Maximum instances reached for this app type.
    #[error("Maximum instances ({max}) reached for app '{app_name}'")]
    MaxInstancesReached {
        /// The app type name.
        app_name: &'static str,
        /// The maximum allowed instances.
        max: usize,
    },

    /// The requested app type is not registered.
    #[error("App type '{0}' not registered")]
    AppNotRegistered(&'static str),
}
