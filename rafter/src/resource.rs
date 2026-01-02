use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use thiserror::Error;

use crate::wakeup::{WakeupHandle, WakeupSender};

/// Progress information for loading operations.
#[derive(Debug, Clone)]
pub struct ProgressState {
    /// Current progress value.
    pub current: u64,
    /// Total value (None for indeterminate).
    pub total: Option<u64>,
    /// Optional message describing current operation.
    pub message: Option<String>,
}

impl ProgressState {
    /// Create new progress state.
    pub fn new(current: u64, total: Option<u64>) -> Self {
        Self {
            current,
            total,
            message: None,
        }
    }

    /// Create progress with a message.
    pub fn with_message(current: u64, total: Option<u64>, message: impl Into<String>) -> Self {
        Self {
            current,
            total,
            message: Some(message.into()),
        }
    }

    /// Get progress as a fraction (0.0 to 1.0).
    pub fn fraction(&self) -> Option<f32> {
        self.total.map(|t| {
            if t == 0 {
                1.0
            } else {
                self.current as f32 / t as f32
            }
        })
    }
}

/// Error type for resource loading failures.
#[derive(Debug, Clone, Error)]
#[error("{message}")]
pub struct ResourceError {
    pub message: String,
}

impl ResourceError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl From<std::io::Error> for ResourceError {
    fn from(err: std::io::Error) -> Self {
        Self::new(err.to_string())
    }
}

impl From<String> for ResourceError {
    fn from(message: String) -> Self {
        Self::new(message)
    }
}

impl From<&str> for ResourceError {
    fn from(message: &str) -> Self {
        Self::new(message)
    }
}

/// The state of an async-loadable resource.
#[derive(Debug, Clone, Default)]
pub enum ResourceState<T> {
    /// Resource has not started loading.
    #[default]
    Idle,
    /// Resource is loading (indeterminate).
    Loading,
    /// Resource is loading with progress.
    Progress(ProgressState),
    /// Resource loaded successfully.
    Ready(T),
    /// Resource failed to load.
    Error(ResourceError),
}

impl<T> ResourceState<T> {
    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading | Self::Progress(_))
    }

    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }

    pub fn as_ready(&self) -> Option<&T> {
        match self {
            Self::Ready(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_error(&self) -> Option<&ResourceError> {
        match self {
            Self::Error(e) => Some(e),
            _ => None,
        }
    }

    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> ResourceState<U> {
        match self {
            Self::Idle => ResourceState::Idle,
            Self::Loading => ResourceState::Loading,
            Self::Progress(p) => ResourceState::Progress(p),
            Self::Ready(v) => ResourceState::Ready(f(v)),
            Self::Error(e) => ResourceState::Error(e),
        }
    }
}

/// Async-loadable resource with interior mutability.
///
/// `Resource<T>` wraps a `ResourceState<T>` with thread-safe, async-compatible
/// state management.
///
/// # Example
///
/// ```ignore
/// #[app]
/// struct MyApp {
///     data: Resource<Vec<Item>>,
/// }
///
/// #[handler]
/// async fn load_data(&self, cx: &AppContext) {
///     self.data.set_loading();
///
///     match fetch_items().await {
///         Ok(items) => self.data.set_ready(items),
///         Err(e) => self.data.set_error(e.to_string()),
///     }
/// }
/// ```
#[derive(Debug)]
pub struct Resource<T> {
    inner: Arc<RwLock<ResourceState<T>>>,
    dirty: Arc<AtomicBool>,
    wakeup: WakeupHandle,
}

impl<T> Resource<T> {
    /// Create a new resource in idle state.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(ResourceState::Idle)),
            dirty: Arc::new(AtomicBool::new(false)),
            wakeup: WakeupHandle::new(),
        }
    }

    /// Install a wakeup sender for this resource.
    pub fn install_wakeup(&self, sender: WakeupSender) {
        self.wakeup.install(sender);
    }

    /// Get a clone of the current state.
    pub fn get(&self) -> ResourceState<T>
    where
        T: Clone,
    {
        self.inner
            .read()
            .map(|guard| guard.clone())
            .unwrap_or(ResourceState::Idle)
    }

    /// Set to idle state.
    pub fn set_idle(&self) {
        if let Ok(mut guard) = self.inner.write() {
            *guard = ResourceState::Idle;
            self.dirty.store(true, Ordering::SeqCst);
            self.wakeup.send();
        }
    }

    /// Set to loading state.
    pub fn set_loading(&self) {
        if let Ok(mut guard) = self.inner.write() {
            *guard = ResourceState::Loading;
            self.dirty.store(true, Ordering::SeqCst);
            self.wakeup.send();
        }
    }

    /// Set to progress state.
    pub fn set_progress(&self, progress: ProgressState) {
        if let Ok(mut guard) = self.inner.write() {
            *guard = ResourceState::Progress(progress);
            self.dirty.store(true, Ordering::SeqCst);
            self.wakeup.send();
        }
    }

    /// Set to ready state with value.
    pub fn set_ready(&self, value: T) {
        if let Ok(mut guard) = self.inner.write() {
            *guard = ResourceState::Ready(value);
            self.dirty.store(true, Ordering::SeqCst);
            self.wakeup.send();
        }
    }

    /// Set to error state.
    pub fn set_error(&self, err: impl Into<ResourceError>) {
        if let Ok(mut guard) = self.inner.write() {
            *guard = ResourceState::Error(err.into());
            self.dirty.store(true, Ordering::SeqCst);
            self.wakeup.send();
        }
    }

    /// Check if the resource has been modified since last check.
    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::SeqCst)
    }

    /// Clear the dirty flag.
    pub fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::SeqCst);
    }

    /// Check if resource is idle.
    pub fn is_idle(&self) -> bool {
        self.inner
            .read()
            .map(|guard| guard.is_idle())
            .unwrap_or(true)
    }

    /// Check if resource is loading.
    pub fn is_loading(&self) -> bool {
        self.inner
            .read()
            .map(|guard| guard.is_loading())
            .unwrap_or(false)
    }

    /// Check if resource is ready.
    pub fn is_ready(&self) -> bool {
        self.inner
            .read()
            .map(|guard| guard.is_ready())
            .unwrap_or(false)
    }

    /// Check if resource has an error.
    pub fn is_error(&self) -> bool {
        self.inner
            .read()
            .map(|guard| guard.is_error())
            .unwrap_or(false)
    }
}

impl<T> Default for Resource<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Clone for Resource<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            dirty: Arc::clone(&self.dirty),
            wakeup: self.wakeup.clone(),
        }
    }
}
