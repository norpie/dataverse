//! System overlay types and traits.
//!
//! System overlays are persistent UI layers rendered by systems (e.g., taskbar,
//! status bar, floating widgets). They are always visible on top of apps but
//! below modals and toasts.
//!
//! # Overlay Types
//!
//! - **Edge overlays** (Top, Bottom, Left, Right): Fixed to screen edges,
//!   shrink the available app area. Multiple overlays on the same edge stack
//!   inward by registration order.
//!
//! - **Absolute overlays**: Positioned at specific coordinates, render on top
//!   of the app without affecting layout.
//!
//! # Focus Order
//!
//! Overlays integrate into the normal focus cycle:
//! - Top/Left overlays: Prepended (focused first)
//! - Right/Bottom overlays: Appended (focused after app)
//! - Absolute overlays: Appended after edge overlays
//!
//! # Example
//!
//! ```ignore
//! #[system_overlay(position = Bottom, height = 1)]
//! struct Taskbar {
//!     apps: Vec<AppInfo>,
//! }
//!
//! #[system_overlay_impl]
//! impl Taskbar {
//!     fn view(&self) -> Node {
//!         // Render taskbar content
//!     }
//! }
//! ```

use std::any::Any;

use crate::context::AppContext;
use crate::node::Node;
use crate::runtime::wakeup::WakeupSender;
use crate::system::System;

/// Position configuration for a system overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemOverlayPosition {
    /// Fixed to the top edge, shrinks app area.
    Top {
        /// Height in rows.
        height: u16,
    },
    /// Fixed to the bottom edge, shrinks app area.
    Bottom {
        /// Height in rows.
        height: u16,
    },
    /// Fixed to the left edge, shrinks app area.
    Left {
        /// Width in columns.
        width: u16,
    },
    /// Fixed to the right edge, shrinks app area.
    Right {
        /// Width in columns.
        width: u16,
    },
    /// Absolute position, renders on top of app without affecting layout.
    Absolute {
        /// X coordinate (column).
        x: u16,
        /// Y coordinate (row).
        y: u16,
        /// Width in columns.
        width: u16,
        /// Height in rows.
        height: u16,
    },
}

impl SystemOverlayPosition {
    /// Returns true if this is an edge overlay (affects app layout).
    pub fn is_edge(&self) -> bool {
        !matches!(self, Self::Absolute { .. })
    }

    /// Returns true if this overlay should be prepended to focus order.
    /// (Top and Left overlays are focused before app content)
    pub fn prepend_focus(&self) -> bool {
        matches!(self, Self::Top { .. } | Self::Left { .. })
    }
}

/// Trait for system overlays - systems with a persistent visual presence.
///
/// System overlays combine the capabilities of [`System`] (keybinds, handlers,
/// events) with a rendered view that's always visible.
pub trait SystemOverlay: System {
    /// Get the overlay's position configuration.
    fn position(&self) -> SystemOverlayPosition;

    /// Render the overlay's content.
    ///
    /// The returned node will be rendered within the overlay's allocated area.
    fn view(&self) -> Node;

    /// Called once when the overlay is initialized, before the first render.
    ///
    /// Use this to fetch initial state that requires `AppContext`.
    fn on_init(&self, _cx: &AppContext) {
        // Default: no-op
    }

    /// Check if the overlay needs re-rendering.
    fn is_dirty(&self) -> bool {
        true // Default: always re-render
    }

    /// Clear dirty flags after rendering.
    fn clear_dirty(&self) {
        // Default: no-op
    }

    /// Install wakeup sender for state change notifications.
    fn install_wakeup(&self, _sender: WakeupSender) {
        // Default: no state fields
    }
}

/// Type-erased system overlay for runtime storage.
pub trait AnySystemOverlay: Send + Sync {
    /// Get the overlay's name (for debugging).
    fn name(&self) -> &'static str;

    /// Get the overlay's position configuration.
    fn position(&self) -> SystemOverlayPosition;

    /// Render the overlay's content.
    fn view(&self) -> Node;

    /// Called once when the overlay is initialized, before the first render.
    fn on_init(&self, cx: &AppContext);

    /// Check if the overlay needs re-rendering.
    fn is_dirty(&self) -> bool;

    /// Clear dirty flags after rendering.
    fn clear_dirty(&self);

    /// Install wakeup sender for state change notifications.
    fn install_wakeup(&self, sender: WakeupSender);

    /// Clone the overlay into a new box.
    fn clone_box(&self) -> Box<dyn AnySystemOverlay>;

    /// Get as Any for downcasting.
    fn as_any(&self) -> &dyn Any;

    // System trait methods (delegated)

    /// Get the overlay's keybinds.
    fn keybinds(&self) -> crate::input::keybinds::Keybinds;

    /// Dispatch a handler by ID.
    fn dispatch(&self, handler_id: &crate::input::keybinds::HandlerId, cx: &AppContext);

    /// Check if this overlay has a handler for the given event type.
    fn has_event_handler(&self, event_type: std::any::TypeId) -> bool;

    /// Check if this overlay has a handler for the given request type.
    fn has_request_handler(&self, request_type: std::any::TypeId) -> bool;

    /// Dispatch an event to this overlay's handlers.
    fn dispatch_event(
        &self,
        event_type: std::any::TypeId,
        event: Box<dyn Any + Send + Sync>,
        cx: &AppContext,
    ) -> bool;

    /// Dispatch a request to this overlay's handlers.
    fn dispatch_request(
        &self,
        request_type: std::any::TypeId,
        request: Box<dyn Any + Send + Sync>,
        cx: &AppContext,
    ) -> Option<std::pin::Pin<Box<dyn std::future::Future<Output = Box<dyn Any + Send + Sync>> + Send>>>;
}

/// Wrapper that implements AnySystemOverlay for any SystemOverlay.
pub struct SystemOverlayInstance<S: SystemOverlay> {
    overlay: S,
}

impl<S: SystemOverlay> SystemOverlayInstance<S> {
    /// Create a new system overlay instance.
    pub fn new(overlay: S) -> Self {
        Self { overlay }
    }
}

impl<S: SystemOverlay + Clone + 'static> AnySystemOverlay for SystemOverlayInstance<S> {
    fn name(&self) -> &'static str {
        self.overlay.name()
    }

    fn position(&self) -> SystemOverlayPosition {
        self.overlay.position()
    }

    fn view(&self) -> Node {
        self.overlay.view()
    }

    fn on_init(&self, cx: &AppContext) {
        SystemOverlay::on_init(&self.overlay, cx)
    }

    fn is_dirty(&self) -> bool {
        SystemOverlay::is_dirty(&self.overlay)
    }

    fn clear_dirty(&self) {
        SystemOverlay::clear_dirty(&self.overlay)
    }

    fn install_wakeup(&self, sender: WakeupSender) {
        SystemOverlay::install_wakeup(&self.overlay, sender)
    }

    fn clone_box(&self) -> Box<dyn AnySystemOverlay> {
        Box::new(Self {
            overlay: self.overlay.clone(),
        })
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn keybinds(&self) -> crate::input::keybinds::Keybinds {
        System::keybinds(&self.overlay)
    }

    fn dispatch(&self, handler_id: &crate::input::keybinds::HandlerId, cx: &AppContext) {
        System::dispatch(&self.overlay, handler_id, cx)
    }

    fn has_event_handler(&self, event_type: std::any::TypeId) -> bool {
        System::has_event_handler(&self.overlay, event_type)
    }

    fn has_request_handler(&self, request_type: std::any::TypeId) -> bool {
        System::has_request_handler(&self.overlay, request_type)
    }

    fn dispatch_event(
        &self,
        event_type: std::any::TypeId,
        event: Box<dyn Any + Send + Sync>,
        cx: &AppContext,
    ) -> bool {
        System::dispatch_event(&self.overlay, event_type, event, cx)
    }

    fn dispatch_request(
        &self,
        request_type: std::any::TypeId,
        request: Box<dyn Any + Send + Sync>,
        cx: &AppContext,
    ) -> Option<std::pin::Pin<Box<dyn std::future::Future<Output = Box<dyn Any + Send + Sync>> + Send>>> {
        System::dispatch_request(&self.overlay, request_type, request, cx)
    }
}

/// Registration entry for a system overlay.
///
/// Used with the `inventory` crate for automatic registration.
pub struct SystemOverlayRegistration {
    /// Name of the overlay type.
    pub name: &'static str,
    /// Factory function to create a new instance.
    pub factory: fn() -> Box<dyn AnySystemOverlay>,
}

impl SystemOverlayRegistration {
    /// Create a new registration entry.
    pub const fn new(name: &'static str, factory: fn() -> Box<dyn AnySystemOverlay>) -> Self {
        Self { name, factory }
    }
}

// Register with inventory
inventory::collect!(SystemOverlayRegistration);

/// Get all registered system overlays.
pub fn registered_system_overlays() -> impl Iterator<Item = &'static SystemOverlayRegistration> {
    inventory::iter::<SystemOverlayRegistration>()
}
