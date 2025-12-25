//! App error types for panic handling.

use std::any::Any;

use super::InstanceId;

/// Error information passed to `Runtime::on_error` callback.
///
/// Contains details about what went wrong and which app instance was affected.
#[derive(Debug, Clone)]
pub struct AppError {
    /// Type name of the app (from `std::any::type_name`).
    pub app_name: &'static str,
    /// Instance ID of the affected app.
    pub instance_id: InstanceId,
    /// Error details.
    pub kind: AppErrorKind,
}

/// The kind of app error that occurred.
#[derive(Debug, Clone)]
pub enum AppErrorKind {
    /// A handler panicked.
    Panic {
        /// Name of the handler that panicked.
        handler_name: String,
        /// Panic message extracted from the panic payload.
        message: String,
    },
    /// An async task spawned via `cx.spawn_task()` panicked.
    TaskPanic {
        /// Panic message extracted from the panic payload.
        message: String,
    },
    /// A lifecycle hook panicked.
    LifecyclePanic {
        /// Name of the lifecycle hook (e.g., "on_start", "on_foreground").
        hook_name: &'static str,
        /// Panic message extracted from the panic payload.
        message: String,
    },
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            AppErrorKind::Panic {
                handler_name,
                message,
            } => {
                write!(
                    f,
                    "App '{}' handler '{}' panicked: {}",
                    self.app_name, handler_name, message
                )
            }
            AppErrorKind::TaskPanic { message } => {
                write!(f, "App '{}' task panicked: {}", self.app_name, message)
            }
            AppErrorKind::LifecyclePanic { hook_name, message } => {
                write!(
                    f,
                    "App '{}' lifecycle hook '{}' panicked: {}",
                    self.app_name, hook_name, message
                )
            }
        }
    }
}

impl std::error::Error for AppError {}

/// Extract a human-readable message from a panic payload.
///
/// Panics can contain either `&str` or `String` payloads. This function
/// attempts to extract either, falling back to a generic message.
pub fn extract_panic_message(panic: &Box<dyn Any + Send>) -> String {
    if let Some(s) = panic.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = panic.downcast_ref::<String>() {
        s.clone()
    } else {
        "Unknown panic".to_string()
    }
}
