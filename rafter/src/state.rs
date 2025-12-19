use std::cell::Cell;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Reactive state wrapper that tracks changes.
///
/// Accessing via `Deref` is read-only.
/// Accessing via `DerefMut` marks the state as dirty.
#[derive(Debug)]
pub struct State<T> {
    value: T,
    dirty: Cell<bool>,
}

impl<T> State<T> {
    /// Create a new state with the given value
    pub fn new(value: T) -> Self {
        Self {
            value,
            dirty: Cell::new(false),
        }
    }

    /// Check if the state has been modified since last check
    pub fn is_dirty(&self) -> bool {
        self.dirty.get()
    }

    /// Clear the dirty flag
    pub fn clear_dirty(&self) {
        self.dirty.set(false);
    }

    /// Get the inner value, consuming the state
    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T> Deref for State<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for State<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.dirty.set(true);
        &mut self.value
    }
}

impl<T: Default> Default for State<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: Clone> Clone for State<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            dirty: Cell::new(self.dirty.get()),
        }
    }
}

impl<T: PartialEq> PartialEq for State<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

/// Async-safe state wrapper for fields that need mutation from spawned tasks.
///
/// Use `#[state(async)]` attribute on fields to wrap them in this type.
/// This type is `Clone` (cheap Arc clone) and `Send + Sync`, so it can be
/// moved into spawned async tasks.
///
/// # Example
///
/// ```ignore
/// #[app]
/// struct MyApp {
///     #[state(async)]
///     status: String,
/// }
///
/// #[handler]
/// fn start_work(&mut self, cx: &mut AppContext) {
///     let status = self.status.clone();
///     cx.spawn(async move {
///         status.set("Working...".to_string());
///         // ... do async work ...
///         status.set("Done!".to_string());
///     });
/// }
/// ```
#[derive(Debug)]
pub struct AsyncState<T> {
    inner: Arc<Mutex<T>>,
    dirty: Arc<AtomicBool>,
}

impl<T> AsyncState<T> {
    /// Create a new async state with the given value
    pub fn new(value: T) -> Self {
        Self {
            inner: Arc::new(Mutex::new(value)),
            dirty: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get a clone of the current value
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.inner
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_else(|poisoned| poisoned.into_inner().clone())
    }

    /// Set a new value
    pub fn set(&self, value: T) {
        if let Ok(mut guard) = self.inner.lock() {
            *guard = value;
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Update the value using a closure
    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut T),
    {
        if let Ok(mut guard) = self.inner.lock() {
            f(&mut guard);
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Check if the state has been modified since last check
    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::SeqCst)
    }

    /// Clear the dirty flag
    pub fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::SeqCst);
    }
}

impl<T> Clone for AsyncState<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            dirty: Arc::clone(&self.dirty),
        }
    }
}

impl<T: Default> Default for AsyncState<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}
