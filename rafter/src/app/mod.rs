//! App module - traits and registration for Rafter applications.

mod registration;
mod traits;

pub use registration::{AppRegistration, CloneableApp, registered_apps};
pub use traits::{App, PanicBehavior};
