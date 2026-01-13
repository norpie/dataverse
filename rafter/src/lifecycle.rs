//! Lifecycle hooks for components.
//!
//! Lifecycle hooks are closure-based, similar to handlers. They are detected
//! via attributes (`#[on_start]`, `#[on_foreground]`, etc.) and the macros
//! generate closures that extract contexts from HandlerContext.

use crate::{Handler, HandlerContext};

/// Lifecycle hook closures for a component.
///
/// Components (systems, apps, modals) return this from `lifecycle_hooks()`.
/// The runtime calls the appropriate hook methods at lifecycle events.
#[derive(Default)]
pub struct LifecycleHooks {
    /// Called when the component starts.
    pub on_start: Option<Handler>,
    /// Called when an app gains focus (apps only).
    pub on_foreground: Option<Handler>,
    /// Called when an app loses focus (apps only).
    pub on_background: Option<Handler>,
    /// Called when an app is closing (apps only).
    pub on_close: Option<Handler>,
}

impl LifecycleHooks {
    /// Create empty lifecycle hooks.
    pub fn new() -> Self {
        Self::default()
    }

    /// Call the on_start hook if present.
    pub fn call_on_start(&self, hx: &HandlerContext) {
        if let Some(handler) = &self.on_start {
            handler(hx);
        }
    }

    /// Call the on_foreground hook if present.
    pub fn call_on_foreground(&self, hx: &HandlerContext) {
        if let Some(handler) = &self.on_foreground {
            handler(hx);
        }
    }

    /// Call the on_background hook if present.
    pub fn call_on_background(&self, hx: &HandlerContext) {
        if let Some(handler) = &self.on_background {
            handler(hx);
        }
    }

    /// Call the on_close hook if present.
    pub fn call_on_close(&self, hx: &HandlerContext) {
        if let Some(handler) = &self.on_close {
            handler(hx);
        }
    }
}
