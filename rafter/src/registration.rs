//! Registration types for inventory-based auto-discovery.

use crate::app::App;
use crate::instance::{AnyAppInstance, AppInstance};
use crate::keybinds::KeybindClosures;
use crate::system::{Overlay, System};
use crate::wakeup::WakeupSender;
use crate::GlobalContext;

/// App registration entry for inventory.
pub struct AppRegistration {
    /// App name.
    pub name: &'static str,
    /// Factory function to create the app.
    pub factory: fn() -> Box<dyn CloneableApp>,
}

impl AppRegistration {
    /// Create a new app registration.
    pub const fn new(name: &'static str, factory: fn() -> Box<dyn CloneableApp>) -> Self {
        Self { name, factory }
    }
}

inventory::collect!(AppRegistration);

/// Get all registered apps.
pub fn registered_apps() -> impl Iterator<Item = &'static AppRegistration> {
    inventory::iter::<AppRegistration>()
}

/// Trait for type-erased cloneable apps.
pub trait CloneableApp: Send + Sync {
    /// Clone into a Box.
    fn clone_box(&self) -> Box<dyn CloneableApp>;
    /// Get the app's display name.
    fn name(&self) -> &'static str;
    /// Convert into a type-erased instance.
    fn into_instance(self: Box<Self>, gx: GlobalContext) -> Box<dyn AnyAppInstance>;
}

impl<T: App> CloneableApp for T {
    fn clone_box(&self) -> Box<dyn CloneableApp> {
        Box::new(self.clone())
    }

    fn name(&self) -> &'static str {
        App::name(self)
    }

    fn into_instance(self: Box<Self>, gx: GlobalContext) -> Box<dyn AnyAppInstance> {
        Box::new(AppInstance::new(*self, gx))
    }
}

/// System registration entry for inventory.
pub struct SystemRegistration {
    /// System name.
    pub name: &'static str,
    /// Factory function to create the system.
    pub factory: fn() -> Box<dyn AnySystem>,
}

impl SystemRegistration {
    /// Create a new system registration.
    pub const fn new(name: &'static str, factory: fn() -> Box<dyn AnySystem>) -> Self {
        Self { name, factory }
    }
}

inventory::collect!(SystemRegistration);

/// Get all registered systems.
pub fn registered_systems() -> impl Iterator<Item = &'static SystemRegistration> {
    inventory::iter::<SystemRegistration>()
}

/// Trait for type-erased systems.
pub trait AnySystem: Send + Sync {
    /// Clone into a Box.
    fn clone_box(&self) -> Box<dyn AnySystem>;
    /// Get the system's name.
    fn name(&self) -> &'static str;
    /// Get the system's keybinds (closure-based).
    fn keybinds(&self) -> KeybindClosures;
    /// Get the system's overlay.
    fn overlay(&self) -> Option<Overlay>;
    /// Called on initialization.
    fn on_init(&self);
    /// Install wakeup sender.
    fn install_wakeup(&self, sender: WakeupSender);
    /// Check if this system has a handler for the given event type.
    fn has_event_handler(&self, event_type: std::any::TypeId) -> bool;
    /// Dispatch an event to this system.
    fn dispatch_event(
        &self,
        event_type: std::any::TypeId,
        event: &(dyn std::any::Any + Send + Sync),
        gx: &GlobalContext,
    ) -> bool;
}

impl<T: System> AnySystem for T {
    fn clone_box(&self) -> Box<dyn AnySystem> {
        Box::new(self.clone())
    }

    fn name(&self) -> &'static str {
        System::name(self)
    }

    fn keybinds(&self) -> KeybindClosures {
        System::keybinds(self)
    }

    fn overlay(&self) -> Option<Overlay> {
        System::overlay(self)
    }

    fn on_init(&self) {
        System::on_init(self)
    }

    fn install_wakeup(&self, sender: WakeupSender) {
        System::install_wakeup(self, sender)
    }

    fn has_event_handler(&self, event_type: std::any::TypeId) -> bool {
        System::has_event_handler(self, event_type)
    }

    fn dispatch_event(
        &self,
        event_type: std::any::TypeId,
        event: &(dyn std::any::Any + Send + Sync),
        gx: &GlobalContext,
    ) -> bool {
        System::dispatch_event(self, event_type, event, gx)
    }
}
