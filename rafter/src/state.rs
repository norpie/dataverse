use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use crate::wakeup::{WakeupHandle, WakeupSender};

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
/// ```
#[derive(Debug)]
pub struct State<T> {
    inner: Arc<RwLock<T>>,
    dirty: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
    wakeup: WakeupHandle,
}

impl<T> State<T> {
    /// Create a new state with the given value.
    pub fn new(value: T) -> Self {
        Self {
            inner: Arc::new(RwLock::new(value)),
            dirty: Arc::new(AtomicBool::new(false)),
            generation: Arc::new(AtomicU64::new(0)),
            wakeup: WakeupHandle::new(),
        }
    }

    /// Install a wakeup sender for this state.
    ///
    /// Called by the runtime when the app is registered.
    /// All clones of this State share the same wakeup sender.
    pub fn install_wakeup(&self, sender: WakeupSender) {
        self.wakeup.install(sender);
    }

    /// Get the current generation number.
    ///
    /// The generation number increments every time the state is modified.
    /// Used by derived state to detect when dependencies have changed.
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    /// Get a clone of the current value.
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.inner
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_else(|poisoned| poisoned.into_inner().clone())
    }

    /// Read the value without cloning.
    ///
    /// Use this when you only need to read data and don't need ownership.
    /// More efficient than `get()` for large state objects.
    pub fn with_ref<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        let guard = self.inner.read().unwrap_or_else(|p| p.into_inner());
        f(&guard)
    }

    /// Set a new value.
    pub fn set(&self, value: T) {
        if let Ok(mut guard) = self.inner.write() {
            *guard = value;
            self.dirty.store(true, Ordering::SeqCst);
            self.generation.fetch_add(1, Ordering::SeqCst);
            self.wakeup.send();
        }
    }

    /// Update the value using a closure.
    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut T),
    {
        if let Ok(mut guard) = self.inner.write() {
            f(&mut guard);
            self.dirty.store(true, Ordering::SeqCst);
            self.generation.fetch_add(1, Ordering::SeqCst);
            self.wakeup.send();
        }
    }

    /// Check if the state has been modified since last check.
    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::SeqCst)
    }

    /// Clear the dirty flag.
    pub fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::SeqCst);
    }
}

impl<T> Clone for State<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            dirty: Arc::clone(&self.dirty),
            generation: Arc::clone(&self.generation),
            wakeup: self.wakeup.clone(),
        }
    }
}

impl<T: Default> Default for State<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}
