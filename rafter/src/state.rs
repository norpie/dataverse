use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

/// Reactive state wrapper with interior mutability.
///
/// `State<T>` provides thread-safe, async-compatible state management.
/// It uses `Arc<RwLock<T>>` internally, making it cheap to clone and
/// safe to use across async task boundaries.
///
/// # Example
///
/// ```ignore
/// #[app]
/// struct Counter {
///     value: i32,  // Becomes State<i32>
/// }
///
/// #[handler]
/// async fn increment(&self, cx: &AppContext) {
///     self.value.update(|v| *v += 1);
/// }
///
/// #[handler]
/// async fn load_value(&self, cx: &AppContext) {
///     let result = fetch_value().await;
///     self.value.set(result);
/// }
/// ```
#[derive(Debug)]
pub struct State<T> {
    inner: Arc<RwLock<T>>,
    dirty: Arc<AtomicBool>,
}

impl<T> State<T> {
    /// Create a new state with the given value
    pub fn new(value: T) -> Self {
        Self {
            inner: Arc::new(RwLock::new(value)),
            dirty: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get a clone of the current value
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.inner
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_else(|poisoned| poisoned.into_inner().clone())
    }

    /// Set a new value
    pub fn set(&self, value: T) {
        if let Ok(mut guard) = self.inner.write() {
            *guard = value;
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Update the value using a closure
    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut T),
    {
        if let Ok(mut guard) = self.inner.write() {
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

impl<T> Clone for State<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            dirty: Arc::clone(&self.dirty),
        }
    }
}

impl<T: Default> Default for State<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}
