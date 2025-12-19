use std::cell::Cell;
use std::ops::{Deref, DerefMut};

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
