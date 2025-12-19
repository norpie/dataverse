use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use thiserror::Error;

/// Progress information for loading operations
#[derive(Debug, Clone)]
pub struct ProgressState {
    /// Current progress value
    pub current: u64,
    /// Total value (None for indeterminate)
    pub total: Option<u64>,
    /// Optional message describing current operation
    pub message: Option<String>,
}

impl ProgressState {
    /// Create new progress state
    pub fn new(current: u64, total: Option<u64>) -> Self {
        Self {
            current,
            total,
            message: None,
        }
    }

    /// Create progress with a message
    pub fn with_message(current: u64, total: Option<u64>, message: impl Into<String>) -> Self {
        Self {
            current,
            total,
            message: Some(message.into()),
        }
    }

    /// Get progress as a fraction (0.0 to 1.0)
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

/// Error type for resource loading failures
#[derive(Debug, Clone, Error)]
#[error("{message}")]
pub struct ResourceError {
    /// Error message
    pub message: String,
}

impl ResourceError {
    /// Create a new resource error
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

/// Async-loadable resource state.
///
/// This type is implicitly async state - it can be mutated from async handlers.
#[derive(Debug, Clone, Default)]
pub enum Resource<T> {
    /// Resource has not started loading
    #[default]
    Idle,
    /// Resource is loading (indeterminate)
    Loading,
    /// Resource is loading with progress
    Progress(ProgressState),
    /// Resource loaded successfully
    Ready(T),
    /// Resource failed to load
    Error(ResourceError),
}

impl<T> Resource<T> {
    /// Check if resource is idle
    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    /// Check if resource is loading (either indeterminate or with progress)
    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading | Self::Progress(_))
    }

    /// Check if resource is ready
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }

    /// Check if resource errored
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }

    /// Get reference to ready value
    pub fn as_ready(&self) -> Option<&T> {
        match self {
            Self::Ready(v) => Some(v),
            _ => None,
        }
    }

    /// Get mutable reference to ready value
    pub fn as_ready_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::Ready(v) => Some(v),
            _ => None,
        }
    }

    /// Get the error if present
    pub fn as_error(&self) -> Option<&ResourceError> {
        match self {
            Self::Error(e) => Some(e),
            _ => None,
        }
    }

    /// Map the ready value
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Resource<U> {
        match self {
            Self::Idle => Resource::Idle,
            Self::Loading => Resource::Loading,
            Self::Progress(p) => Resource::Progress(p),
            Self::Ready(v) => Resource::Ready(f(v)),
            Self::Error(e) => Resource::Error(e),
        }
    }
}

/// Async-safe wrapper for Resource that can be mutated from async contexts.
/// Uses Arc<Mutex<>> internally for thread-safe access.
#[derive(Debug)]
pub struct AsyncResource<T> {
    inner: Arc<Mutex<Resource<T>>>,
    dirty: Arc<AtomicBool>,
}

impl<T> AsyncResource<T> {
    /// Create a new async resource
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Resource::Idle)),
            dirty: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Set the resource state
    pub fn set(&self, state: Resource<T>) {
        if let Ok(mut guard) = self.inner.lock() {
            *guard = state;
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Get a clone of the current state
    pub fn get(&self) -> Resource<T>
    where
        T: Clone,
    {
        self.inner.lock().map_or(Resource::Idle, |g| g.clone())
    }

    /// Check if the resource has been modified since last check
    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::SeqCst)
    }

    /// Clear the dirty flag
    pub fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::SeqCst);
    }
}

impl<T> Default for AsyncResource<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Clone for AsyncResource<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            dirty: Arc::clone(&self.dirty),
        }
    }
}
