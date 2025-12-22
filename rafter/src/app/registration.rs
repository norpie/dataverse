//! App registration for inventory-based discovery.

use super::traits::App;

/// App registration entry for inventory
pub struct AppRegistration {
    /// App name
    pub name: &'static str,
    /// Factory function to create the app (returns type-erased app)
    pub factory: fn() -> Box<dyn CloneableApp>,
}

impl AppRegistration {
    /// Create a new app registration
    pub const fn new(name: &'static str, factory: fn() -> Box<dyn CloneableApp>) -> Self {
        Self { name, factory }
    }
}

// Collect all registered apps
inventory::collect!(AppRegistration);

/// Get all registered apps
pub fn registered_apps() -> impl Iterator<Item = &'static AppRegistration> {
    inventory::iter::<AppRegistration>()
}

/// Trait for type-erased cloneable apps (used by app registry)
pub trait CloneableApp: Send + Sync {
    /// Clone into a Box
    fn clone_box(&self) -> Box<dyn CloneableApp>;
    /// Get the app's display name
    fn name(&self) -> &'static str;
}

impl<T: App> CloneableApp for T {
    fn clone_box(&self) -> Box<dyn CloneableApp> {
        Box::new(self.clone())
    }

    fn name(&self) -> &'static str {
        App::name(self)
    }
}
