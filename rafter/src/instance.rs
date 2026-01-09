//! Instance management types.
//!
//! This module contains types for managing app instances in the runtime.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::RwLock;
use std::time::Instant;

use tuidom::Element;
use uuid::Uuid;

use crate::app::{App, AppConfig};
use crate::global_context::InstanceQuery;
use crate::keybinds::KeybindClosures;
use crate::wakeup::WakeupSender;
use crate::{AppContext, GlobalContext, HandlerRegistry};

// =============================================================================
// InstanceId
// =============================================================================

/// Unique identifier for a running app instance.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct InstanceId(Uuid);

impl InstanceId {
    /// Create a new unique instance ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from a raw u64 (for compatibility).
    pub fn from_raw(id: u64) -> Self {
        // Create a deterministic UUID from the u64
        Self(Uuid::from_u64_pair(id, 0))
    }

    /// Get the underlying UUID.
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for InstanceId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for InstanceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// =============================================================================
// InstanceInfo
// =============================================================================

/// Metadata about a running app instance.
///
/// Used for app launchers/switchers to display and select instances.
#[derive(Debug, Clone)]
pub struct InstanceInfo {
    /// Unique identifier for this instance.
    pub id: InstanceId,
    /// App type ID.
    pub type_id: TypeId,
    /// App type name (from config).
    pub name: &'static str,
    /// Instance-specific title (e.g., "Record #123").
    pub title: String,
    /// When this instance was created.
    pub created_at: Instant,
    /// When this instance was last focused.
    pub last_focused_at: Instant,
    /// Whether this instance is currently focused.
    pub is_focused: bool,
    /// Whether this instance is sleeping.
    pub is_sleeping: bool,
}

impl InstanceInfo {
    /// Create new instance info.
    pub fn new(id: InstanceId, type_id: TypeId, name: &'static str, title: String) -> Self {
        let now = Instant::now();
        Self {
            id,
            type_id,
            name,
            title,
            created_at: now,
            last_focused_at: now,
            is_focused: false,
            is_sleeping: false,
        }
    }
}

// =============================================================================
// Errors
// =============================================================================

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

// =============================================================================
// AnyAppInstance
// =============================================================================

/// Type-erased wrapper for app instances in the registry.
///
/// This trait allows the runtime to store and operate on app instances
/// without knowing their concrete types.
pub trait AnyAppInstance: Send + Sync {
    /// Get the unique instance ID.
    fn id(&self) -> InstanceId;

    /// Get the TypeId of the underlying app type.
    fn type_id(&self) -> TypeId;

    /// Get instance metadata.
    fn info(&self) -> InstanceInfo;

    /// Get the app configuration.
    fn config(&self) -> AppConfig;

    /// Get the app's keybinds (closure-based).
    fn keybinds(&self) -> KeybindClosures;

    /// Get the handler registry for widget events.
    fn handlers(&self) -> &HandlerRegistry;

    /// Get the current page identifier for keybind scoping.
    fn current_page(&self) -> Option<String>;

    /// Get the app context for this instance.
    ///
    /// Returns a properly configured AppContext with wakeup sender installed.
    fn app_context(&self) -> AppContext;

    /// Push a modal onto this instance's modal stack.
    fn push_modal(&self, modal: Box<dyn crate::runtime::dispatch::AnyModal>);

    /// Get the instance's modal stack for dispatch.
    fn modals(&self) -> &RwLock<Vec<Box<dyn crate::runtime::dispatch::AnyModal>>>;

    // Rendering

    /// Render the app's element tree.
    fn element(&self) -> Element;

    /// Check if the app needs re-rendering.
    fn is_dirty(&self) -> bool;

    /// Clear dirty flags after render.
    fn clear_dirty(&self);

    /// Install wakeup sender on all State fields and AppContext.
    fn install_wakeup(&self, sender: WakeupSender, gx: &GlobalContext);

    /// Call the app's on_start lifecycle method.
    fn on_start(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;

    /// Call the app's on_foreground lifecycle method.
    fn on_foreground(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;

    /// Call the app's on_background lifecycle method.
    fn on_background(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;

    // Event/Request Dispatch

    /// Check if this instance has a handler for the given event type.
    fn has_event_handler(&self, event_type: TypeId) -> bool;

    /// Check if this instance has a handler for the given request type.
    fn has_request_handler(&self, request_type: TypeId) -> bool;

    /// Dispatch an event to this instance's handlers.
    fn dispatch_event(
        &self,
        event_type: TypeId,
        event: &(dyn Any + Send + Sync),
        cx: &AppContext,
        gx: &GlobalContext,
    ) -> bool;

    /// Dispatch a request to this instance's handlers.
    fn dispatch_request(
        &self,
        request_type: TypeId,
        request: Box<dyn Any + Send + Sync>,
        cx: &AppContext,
        gx: &GlobalContext,
    ) -> Option<Pin<Box<dyn Future<Output = Box<dyn Any + Send + Sync>> + Send>>>;

    // State management

    /// Clone the instance into a new box.
    fn clone_box(&self) -> Box<dyn AnyAppInstance>;

    /// Update the last focused time.
    fn mark_focused(&self);

    /// Update focused state.
    fn set_focused(&self, focused: bool);

    /// Check if the instance is sleeping.
    fn is_sleeping(&self) -> bool;

    /// Set the sleeping state.
    fn set_sleeping(&self, sleeping: bool);
}

// =============================================================================
// AppInstance
// =============================================================================

/// Wrapper that implements AnyAppInstance for any App.
pub struct AppInstance<A: App> {
    /// The app instance.
    app: A,
    /// Unique instance ID.
    id: InstanceId,
    /// Instance info (interior mutability for focused/sleeping state).
    info: RwLock<InstanceInfo>,
    /// App context for this instance (interior mutability for wakeup sender).
    context: RwLock<AppContext>,
    /// Modal stack for this instance.
    modals: RwLock<Vec<Box<dyn crate::runtime::dispatch::AnyModal>>>,
}

impl<A: App> AppInstance<A> {
    /// Create a new app instance.
    pub fn new(app: A, gx: GlobalContext) -> Self {
        let id = InstanceId::new();
        let config = A::config();
        let title = app.title();
        let type_id = TypeId::of::<A>();
        log::debug!(
            "[AppInstance::new] name={} type_id={:?}",
            config.name, type_id
        );
        let info = InstanceInfo::new(id, type_id, config.name, title);
        let context = AppContext::new(id, gx, config.name);

        Self {
            app,
            id,
            info: RwLock::new(info),
            context: RwLock::new(context),
            modals: RwLock::new(Vec::new()),
        }
    }

    /// Get a reference to the underlying app.
    pub fn app(&self) -> &A {
        &self.app
    }
}

impl<A: App> AnyAppInstance for AppInstance<A> {
    fn id(&self) -> InstanceId {
        self.id
    }

    fn type_id(&self) -> TypeId {
        let tid = TypeId::of::<A>();
        log::debug!("[AnyAppInstance::type_id] A={} type_id={:?}", std::any::type_name::<A>(), tid);
        tid
    }

    fn info(&self) -> InstanceInfo {
        let mut info = self.info.read().unwrap_or_else(|e| e.into_inner()).clone();
        // Update title in case it changed
        info.title = self.app.title();
        info
    }

    fn config(&self) -> AppConfig {
        A::config()
    }

    fn keybinds(&self) -> KeybindClosures {
        self.app.keybinds()
    }

    fn handlers(&self) -> &HandlerRegistry {
        self.app.handlers()
    }

    fn current_page(&self) -> Option<String> {
        self.app.current_page()
    }

    fn app_context(&self) -> AppContext {
        self.context.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn push_modal(&self, modal: Box<dyn crate::runtime::dispatch::AnyModal>) {
        if let Ok(mut modals) = self.modals.write() {
            modals.push(modal);
        }
    }

    fn modals(&self) -> &RwLock<Vec<Box<dyn crate::runtime::dispatch::AnyModal>>> {
        &self.modals
    }

    fn element(&self) -> Element {
        self.app.element()
    }

    fn is_dirty(&self) -> bool {
        self.app.is_dirty()
    }

    fn clear_dirty(&self) {
        self.app.clear_dirty();
    }

    fn install_wakeup(&self, sender: WakeupSender, gx: &GlobalContext) {
        // Install on State fields
        self.app.install_wakeup(sender.clone());
        // Install on AppContext
        if let Ok(mut ctx) = self.context.write() {
            ctx.set_wakeup_sender(sender);
            // Update GlobalContext reference in case it changed
            ctx.set_global(gx.clone());
        }
    }

    fn on_start(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(self.app.on_start())
    }

    fn on_foreground(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(self.app.on_foreground())
    }

    fn on_background(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(self.app.on_background())
    }

    fn has_event_handler(&self, event_type: TypeId) -> bool {
        self.app.has_event_handler(event_type)
    }

    fn has_request_handler(&self, request_type: TypeId) -> bool {
        self.app.has_request_handler(request_type)
    }

    fn dispatch_event(
        &self,
        event_type: TypeId,
        event: &(dyn Any + Send + Sync),
        cx: &AppContext,
        gx: &GlobalContext,
    ) -> bool {
        self.app.dispatch_event(event_type, event, cx, gx)
    }

    fn dispatch_request(
        &self,
        request_type: TypeId,
        request: Box<dyn Any + Send + Sync>,
        cx: &AppContext,
        gx: &GlobalContext,
    ) -> Option<Pin<Box<dyn Future<Output = Box<dyn Any + Send + Sync>> + Send>>> {
        self.app.dispatch_request(request_type, request, cx, gx)
    }

    fn clone_box(&self) -> Box<dyn AnyAppInstance> {
        Box::new(Self {
            app: self.app.clone(),
            id: self.id,
            info: RwLock::new(self.info.read().unwrap_or_else(|e| e.into_inner()).clone()),
            context: RwLock::new(self.context.read().unwrap_or_else(|e| e.into_inner()).clone()),
            modals: RwLock::new(Vec::new()), // Modals don't transfer on clone
        })
    }

    fn mark_focused(&self) {
        if let Ok(mut info) = self.info.write() {
            info.last_focused_at = Instant::now();
        }
    }

    fn set_focused(&self, focused: bool) {
        if let Ok(mut info) = self.info.write() {
            info.is_focused = focused;
        }
    }

    fn is_sleeping(&self) -> bool {
        self.info
            .read()
            .map(|i| i.is_sleeping)
            .unwrap_or(false)
    }

    fn set_sleeping(&self, sleeping: bool) {
        if let Ok(mut info) = self.info.write() {
            info.is_sleeping = sleeping;
        }
    }
}

// =============================================================================
// InstanceRegistry
// =============================================================================

/// Registry managing all running app instances.
///
/// Tracks all running instances, focus state, and MRU order.
pub struct InstanceRegistry {
    /// All running instances.
    instances: HashMap<InstanceId, Box<dyn AnyAppInstance>>,
    /// Currently focused instance.
    focused: Option<InstanceId>,
    /// Most recently used order (most recent first).
    mru_order: Vec<InstanceId>,
    /// Count of instances per app type.
    instance_counts: HashMap<TypeId, usize>,
}

impl InstanceRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            instances: HashMap::new(),
            focused: None,
            mru_order: Vec::new(),
            instance_counts: HashMap::new(),
        }
    }

    /// Insert an instance into the registry.
    pub fn insert(&mut self, instance: Box<dyn AnyAppInstance>) {
        let id = instance.id();
        let type_id = AnyAppInstance::type_id(instance.as_ref());

        log::debug!(
            "[registry.insert] id={:?} type_id={:?}",
            id, type_id
        );

        self.instances.insert(id, instance);
        *self.instance_counts.entry(type_id).or_insert(0) += 1;
        self.mru_order.insert(0, id);
    }

    /// Close an instance.
    ///
    /// Returns true if the instance was removed.
    pub fn close(&mut self, id: InstanceId) -> bool {
        let Some(instance) = self.instances.remove(&id) else {
            return false;
        };

        let type_id = AnyAppInstance::type_id(instance.as_ref());

        // Update instance count
        if let Some(count) = self.instance_counts.get_mut(&type_id) {
            *count = count.saturating_sub(1);
        }

        // Remove from MRU order
        self.mru_order.retain(|&i| i != id);

        // If this was focused, focus MRU
        if self.focused == Some(id) {
            self.focused = None;
            self.focus_mru();
        }

        true
    }

    /// Focus an instance.
    pub fn focus(&mut self, id: InstanceId) -> bool {
        if !self.instances.contains_key(&id) {
            return false;
        }

        // Update old focused instance
        if let Some(old_id) = self.focused {
            if old_id != id {
                if let Some(old) = self.instances.get(&old_id) {
                    old.set_focused(false);
                }
            }
        }

        // Update new focused instance
        if let Some(instance) = self.instances.get(&id) {
            instance.set_focused(true);
            instance.mark_focused();
        }

        self.focused = Some(id);

        // Move to front of MRU
        self.mru_order.retain(|&i| i != id);
        self.mru_order.insert(0, id);

        true
    }

    /// Focus the most recently used instance.
    pub fn focus_mru(&mut self) -> bool {
        for &id in &self.mru_order.clone() {
            if self.instances.contains_key(&id) {
                return self.focus(id);
            }
        }
        false
    }

    /// Get the currently focused instance ID.
    pub fn focused(&self) -> Option<InstanceId> {
        self.focused
    }

    /// Get a reference to an instance.
    pub fn get(&self, id: InstanceId) -> Option<&dyn AnyAppInstance> {
        self.instances.get(&id).map(|b| b.as_ref())
    }

    /// Get a mutable reference to an instance.
    pub fn get_mut(&mut self, id: InstanceId) -> Option<&mut (dyn AnyAppInstance + '_)> {
        match self.instances.get_mut(&id) {
            Some(b) => Some(b.as_mut()),
            None => None,
        }
    }

    /// Get a reference to the focused instance.
    pub fn focused_instance(&self) -> Option<&dyn AnyAppInstance> {
        self.focused.and_then(|id| self.get(id))
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }

    /// Get total count.
    pub fn len(&self) -> usize {
        self.instances.len()
    }

    /// Iterate over all instances.
    pub fn iter(&self) -> impl Iterator<Item = &dyn AnyAppInstance> {
        self.instances.values().map(|b| b.as_ref())
    }
}

impl Default for InstanceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl InstanceQuery for InstanceRegistry {
    fn instances(&self) -> Vec<InstanceInfo> {
        self.instances.values().map(|i| i.info()).collect()
    }

    fn instances_of_type(&self, target_type_id: TypeId) -> Vec<InstanceInfo> {
        self.instances
            .values()
            .filter_map(|i| {
                if AnyAppInstance::type_id(i.as_ref()) == target_type_id {
                    Some(i.info())
                } else {
                    None
                }
            })
            .collect()
    }

    fn instance_of_type(&self, target_type_id: TypeId) -> Option<InstanceId> {
        log::debug!(
            "[registry.instance_of_type] looking for {:?}, have {} instances",
            target_type_id, self.instances.len()
        );
        for instance in self.instances.values() {
            let inst_type_id = AnyAppInstance::type_id(instance.as_ref());
            log::debug!(
                "[registry.instance_of_type] checking {:?} (type={:?})",
                instance.id(), inst_type_id
            );
            if inst_type_id == target_type_id {
                return Some(instance.id());
            }
        }
        None
    }

    fn instance_count_of_type(&self, type_id: TypeId) -> usize {
        self.instance_counts.get(&type_id).copied().unwrap_or(0)
    }

    fn focused_instance_id(&self) -> Option<InstanceId> {
        self.focused
    }
}
