//! App module - traits and registration for Rafter applications.

mod any_instance;
mod config;
mod error;
mod instance;
mod registration;
mod registry;
mod traits;

pub use any_instance::{AnyAppInstance, AppInstance};
pub use config::{AppConfig, BlurPolicy, SpawnError};
pub use error::{AppError, AppErrorKind, extract_panic_message};
pub use instance::{InstanceId, InstanceInfo};
pub use registration::{AppRegistration, CloneableApp, registered_apps};
pub use registry::InstanceRegistry;
pub use traits::{App, PanicBehavior, PersistentApp};
