//! Handler context bundle for unified context passing to handlers.
//!
//! This module provides `HandlerContext`, a single struct that bundles all
//! available contexts (AppContext, GlobalContext, and optionally ModalContext)
//! for passing to widget handlers.
//!
//! Handlers declare what contexts they need in their signature, and the
//! `page!` macro generates code to extract the appropriate contexts from
//! the HandlerContext bundle.

use std::any::Any;

use crate::{AppContext, GlobalContext, ModalContext};

/// Context bundle passed to all widget handlers (inline and keybind).
///
/// This struct provides unified access to all available contexts. The `page!`
/// macro generates closures that accept `&HandlerContext` and extract the
/// specific contexts each handler needs.
///
/// # For Apps
///
/// App handlers have access to `cx()` and `gx()`. Attempting to use `mx()`
/// will panic (but compile-time checks in `#[app_impl]` prevent this).
///
/// # For Modals
///
/// Modal handlers have access to `cx()`, `gx()`, and `mx()`.
pub struct HandlerContext<'a> {
    cx: &'a AppContext,
    gx: &'a GlobalContext,
    /// Type-erased modal context (None for apps/systems)
    modal_context: Option<&'a (dyn Any + Send + Sync)>,
}

impl<'a> HandlerContext<'a> {
    /// Create a HandlerContext for app handlers (no modal context).
    pub fn for_app(cx: &'a AppContext, gx: &'a GlobalContext) -> Self {
        Self {
            cx,
            gx,
            modal_context: None,
        }
    }

    /// Create a HandlerContext for modal handlers.
    pub fn for_modal<R: Send + Sync + 'static>(
        cx: &'a AppContext,
        gx: &'a GlobalContext,
        mx: &'a ModalContext<R>,
    ) -> Self {
        Self {
            cx,
            gx,
            modal_context: Some(mx),
        }
    }

    /// Get the app context.
    pub fn cx(&self) -> &AppContext {
        self.cx
    }

    /// Get the global context.
    pub fn gx(&self) -> &GlobalContext {
        self.gx
    }

    /// Get the modal context.
    ///
    /// # Panics
    ///
    /// Panics if called outside a modal context. With compile-time checks
    /// in `#[app_impl]` and `#[modal_impl]`, this should never happen in
    /// correctly written code.
    pub fn mx<R: Send + Sync + 'static>(&self) -> &ModalContext<R> {
        self.modal_context
            .expect("mx() called outside modal context")
            .downcast_ref::<ModalContext<R>>()
            .expect("ModalContext type mismatch")
    }

    /// Try to get the modal context (returns None if not in a modal).
    pub fn try_mx<R: Send + Sync + 'static>(&self) -> Option<&ModalContext<R>> {
        self.modal_context?.downcast_ref()
    }

    /// Check if this context is for a modal.
    pub fn is_modal(&self) -> bool {
        self.modal_context.is_some()
    }
}
