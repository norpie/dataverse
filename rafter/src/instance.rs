//! Instance management types.
//!
//! This module contains types for managing app instances in the runtime.
//! Full instance registry implementation comes in Chunk 3.

use std::any::TypeId;

/// Unique identifier for an app instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InstanceId(u64);

impl InstanceId {
    /// Create a new instance ID.
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Get the raw ID value.
    pub fn raw(&self) -> u64 {
        self.0
    }
}

/// Information about a running instance.
#[derive(Debug, Clone)]
pub struct InstanceInfo {
    /// Instance ID.
    pub id: InstanceId,
    /// App type ID.
    pub type_id: TypeId,
    /// App name.
    pub name: &'static str,
    /// Instance title.
    pub title: String,
}

/// Error when spawning an instance.
#[derive(Debug, Clone)]
pub enum SpawnError {
    /// Maximum instances of this app type reached.
    MaxInstancesReached {
        /// App name.
        app_name: &'static str,
        /// Maximum allowed instances.
        max: usize,
    },
}

impl std::fmt::Display for SpawnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpawnError::MaxInstancesReached { app_name, max } => {
                write!(
                    f,
                    "Maximum instances ({}) reached for app '{}'",
                    max, app_name
                )
            }
        }
    }
}

impl std::error::Error for SpawnError {}

/// Error when making a request.
#[derive(Debug, Clone)]
pub enum RequestError {
    /// No instance of the target type found.
    NoInstance,
    /// Instance not found.
    InstanceNotFound,
    /// Target has no handler for this request type.
    NoHandler,
    /// Handler panicked.
    HandlerPanicked,
}

impl std::fmt::Display for RequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestError::NoInstance => write!(f, "No instance of target type found"),
            RequestError::InstanceNotFound => write!(f, "Instance not found"),
            RequestError::NoHandler => write!(f, "No handler for this request type"),
            RequestError::HandlerPanicked => write!(f, "Handler panicked"),
        }
    }
}

impl std::error::Error for RequestError {}
