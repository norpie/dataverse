use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use crate::runtime::wakeup::WakeupSender;

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
    wakeup: Arc<Mutex<Option<WakeupSender>>>,
}

impl<T> State<T> {
    /// Create a new state with the given value
    pub fn new(value: T) -> Self {
        Self {
            inner: Arc::new(RwLock::new(value)),
            dirty: Arc::new(AtomicBool::new(false)),
            wakeup: Arc::new(Mutex::new(None)),
        }
    }

    /// Install a wakeup sender for this state.
    ///
    /// Called by the runtime when the app is registered.
    /// All clones of this State share the same wakeup sender.
    pub fn install_wakeup(&self, sender: WakeupSender) {
        if let Ok(mut guard) = self.wakeup.lock() {
            *guard = Some(sender);
        }
    }

    /// Send a wakeup signal if a sender is installed
    fn send_wakeup(&self) {
        if let Ok(guard) = self.wakeup.lock()
            && let Some(sender) = guard.as_ref() {
                sender.send();
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
            log::debug!("State::set() sending wakeup");
            self.send_wakeup();
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
            log::debug!("State::update() sending wakeup");
            self.send_wakeup();
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
            wakeup: Arc::clone(&self.wakeup),
        }
    }
}

impl<T: Default> Default for State<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}
