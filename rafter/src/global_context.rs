//! Global context for runtime-wide operations.
//!
//! GlobalContext provides access to global operations like:
//! - Shutdown
//! - Instance management (spawn, close, focus)
//! - Toast notifications
//! - Theme changes
//! - Global modals
//! - Inter-app communication (publish, request)
//! - Global data
//! - Keybind management

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use tokio::sync::oneshot;
use tuidom::Theme;

use crate::instance::{InstanceId, InstanceInfo, RequestError, SpawnError};
use crate::keybinds::{KeybindError, KeybindInfo, Keybinds};
use crate::modal::{Modal, ModalContext, ModalEntry};
use crate::registration::CloneableApp;
use crate::wakeup::WakeupSender;
use crate::{App, Event, Request, Toast};

// =============================================================================
// ArcEvent
// =============================================================================

/// Type-erased event wrapped in Arc for cheap cloning.
///
/// Events are stored as `Arc<dyn Any>` so they can be shared across
/// multiple subscribers without copying.
#[derive(Clone)]
pub struct ArcEvent {
    event: Arc<dyn Any + Send + Sync>,
}

impl ArcEvent {
    /// Create a new arc event.
    pub fn new<E: Event>(event: E) -> Self {
        Self {
            event: Arc::new(event),
        }
    }

    /// Get the event type ID.
    pub fn type_id(&self) -> TypeId {
        (*self.event).type_id()
    }

    /// Get a reference to the event, downcasted to type E.
    pub fn downcast_ref<E: Event>(&self) -> Option<&E> {
        self.event.downcast_ref()
    }
}

// =============================================================================
// RequestTarget
// =============================================================================

/// Target for a request.
#[derive(Debug, Clone)]
pub enum RequestTarget {
    /// Target the first (non-sleeping) instance of an app type.
    AppType(TypeId),
    /// Target a specific instance by ID.
    Instance(InstanceId),
}

// =============================================================================
// InstanceCommand
// =============================================================================

/// Command to manage app instances.
///
/// These commands are queued and processed by the runtime event loop.
pub enum InstanceCommand {
    /// Spawn a new app instance.
    Spawn {
        /// The boxed app instance to spawn.
        app: Box<dyn CloneableApp>,
        /// Whether to focus the new instance.
        focus: bool,
    },
    /// Close an instance.
    Close {
        /// The instance to close.
        id: InstanceId,
        /// Whether to force close (skip on_close_request).
        force: bool,
    },
    /// Focus an instance.
    Focus {
        /// The instance to focus.
        id: InstanceId,
    },
    /// Publish an event to all non-sleeping instances.
    PublishEvent {
        /// The event to publish.
        event: ArcEvent,
    },
    /// Send a request to an instance.
    SendRequest {
        /// The target of the request.
        target: RequestTarget,
        /// The request to send.
        request: Box<dyn Any + Send + Sync>,
        /// The TypeId of the request.
        request_type: TypeId,
        /// Channel to send the response back.
        response_tx: oneshot::Sender<Result<Box<dyn Any + Send + Sync>, RequestError>>,
    },
}

// =============================================================================
// ModalRequest
// =============================================================================

/// A request to open a global modal.
pub struct GlobalModalRequest {
    /// The type-erased modal entry.
    pub entry: Box<dyn Any + Send + Sync>,
}

// =============================================================================
// GlobalContextInner
// =============================================================================

/// Inner state for GlobalContext.
struct GlobalContextInner {
    /// Request to shutdown the runtime.
    shutdown_requested: bool,
    /// Pending toasts to show.
    pending_toasts: Vec<Toast>,
    /// Request to change theme.
    theme_request: Option<Arc<dyn Theme>>,
    /// Pending global modal.
    modal_request: Option<GlobalModalRequest>,
    /// Pending instance commands.
    instance_commands: Vec<InstanceCommand>,
}

impl Default for GlobalContextInner {
    fn default() -> Self {
        Self {
            shutdown_requested: false,
            pending_toasts: Vec::new(),
            theme_request: None,
            modal_request: None,
            instance_commands: Vec::new(),
        }
    }
}

// =============================================================================
// InstanceQuery
// =============================================================================

/// Trait for querying instances.
///
/// Implemented by InstanceRegistry. Abstracted to avoid circular dependencies.
pub trait InstanceQuery: Send + Sync {
    fn instances(&self) -> Vec<InstanceInfo>;
    fn instances_of_type(&self, type_id: TypeId) -> Vec<InstanceInfo>;
    fn instance_of_type(&self, type_id: TypeId) -> Option<InstanceId>;
    fn instance_count_of_type(&self, type_id: TypeId) -> usize;
    fn focused_instance_id(&self) -> Option<InstanceId>;
}

// =============================================================================
// DataStore
// =============================================================================

/// Type-erased storage for global data.
pub type DataStore = HashMap<TypeId, Arc<dyn Any + Send + Sync>>;

// =============================================================================
// GlobalContext
// =============================================================================

/// Global context for runtime-wide operations.
///
/// Passed to handlers that need global access. Systems receive this directly.
/// Apps receive it alongside AppContext when declared in handler signature.
///
/// Uses interior mutability with command queue pattern - operations are queued
/// and processed by the event loop.
#[derive(Clone)]
pub struct GlobalContext {
    inner: Arc<RwLock<GlobalContextInner>>,
    /// Shared keybinds (can be modified at runtime).
    keybinds: Arc<RwLock<Keybinds>>,
    /// Instance query interface.
    registry: Option<Arc<dyn InstanceQuery>>,
    /// Global data store (read-only, set at runtime startup).
    data: Arc<DataStore>,
    /// Wakeup sender for notifying the event loop.
    wakeup_sender: Option<WakeupSender>,
}

impl GlobalContext {
    /// Create a new global context.
    pub fn new(keybinds: Arc<RwLock<Keybinds>>, data: Arc<DataStore>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(GlobalContextInner::default())),
            keybinds,
            registry: None,
            data,
            wakeup_sender: None,
        }
    }

    // =========================================================================
    // Setup (runtime use only)
    // =========================================================================

    /// Set the wakeup sender (called by runtime).
    pub(crate) fn set_wakeup_sender(&mut self, sender: WakeupSender) {
        self.wakeup_sender = Some(sender);
    }

    /// Set the instance registry (called by runtime).
    pub(crate) fn set_registry(&mut self, registry: Arc<dyn InstanceQuery>) {
        self.registry = Some(registry);
    }

    /// Send a wakeup signal to the event loop.
    fn send_wakeup(&self) {
        if let Some(sender) = &self.wakeup_sender {
            sender.send();
        }
    }

    // =========================================================================
    // Shutdown
    // =========================================================================

    /// Request to shutdown the runtime.
    ///
    /// Closes all apps and exits the event loop.
    pub fn shutdown(&self) {
        if let Ok(mut inner) = self.inner.write() {
            inner.shutdown_requested = true;
            self.send_wakeup();
        }
    }

    // =========================================================================
    // Instance Management
    // =========================================================================

    /// Spawn a new app instance.
    ///
    /// The instance is added to the registry but not focused.
    /// Use `spawn_and_focus` to spawn and immediately focus.
    pub fn spawn<A: App>(&self, app: A) -> Result<InstanceId, SpawnError> {
        let config = A::config();

        // Check max instances using the registry
        if let Some(max) = config.max_instances {
            let current = self.instance_count::<A>();
            if current >= max {
                return Err(SpawnError::MaxInstancesReached {
                    app_name: config.name,
                    max,
                });
            }
        }

        // Queue the spawn command - ID will be assigned by registry
        if let Ok(mut inner) = self.inner.write() {
            inner.instance_commands.push(InstanceCommand::Spawn {
                app: Box::new(app),
                focus: false,
            });
            self.send_wakeup();
        }

        // Return a placeholder - actual ID assigned by registry when processed
        Ok(InstanceId::new())
    }

    /// Spawn a new app instance and immediately focus it.
    pub fn spawn_and_focus<A: App>(&self, app: A) -> Result<InstanceId, SpawnError> {
        let config = A::config();

        // Check max instances using the registry
        if let Some(max) = config.max_instances {
            let current = self.instance_count::<A>();
            if current >= max {
                return Err(SpawnError::MaxInstancesReached {
                    app_name: config.name,
                    max,
                });
            }
        }

        // Queue the spawn command - ID will be assigned by registry
        if let Ok(mut inner) = self.inner.write() {
            inner.instance_commands.push(InstanceCommand::Spawn {
                app: Box::new(app),
                focus: true,
            });
            self.send_wakeup();
        }

        // Return a placeholder - actual ID assigned by registry when processed
        Ok(InstanceId::new())
    }

    /// Close an instance.
    ///
    /// Respects `on_close_request` - if it returns false, the close is cancelled.
    /// Use `force_close` to skip this check.
    pub fn close(&self, id: InstanceId) {
        if let Ok(mut inner) = self.inner.write() {
            inner
                .instance_commands
                .push(InstanceCommand::Close { id, force: false });
            self.send_wakeup();
        }
    }

    /// Force close an instance.
    ///
    /// Skips `on_close_request` but respects the `persistent` flag.
    pub fn force_close(&self, id: InstanceId) {
        if let Ok(mut inner) = self.inner.write() {
            inner
                .instance_commands
                .push(InstanceCommand::Close { id, force: true });
            self.send_wakeup();
        }
    }

    /// Focus an instance.
    ///
    /// Makes the instance the foreground app, receiving input and rendering.
    pub fn focus_instance(&self, id: InstanceId) {
        if let Ok(mut inner) = self.inner.write() {
            inner.instance_commands.push(InstanceCommand::Focus { id });
            self.send_wakeup();
        }
    }

    // =========================================================================
    // Instance Discovery
    // =========================================================================

    /// List all running instances.
    pub fn instances(&self) -> Vec<InstanceInfo> {
        self.registry
            .as_ref()
            .map(|r| r.instances())
            .unwrap_or_default()
    }

    /// List instances of a specific app type.
    pub fn instances_of<A: App>(&self) -> Vec<InstanceInfo> {
        self.registry
            .as_ref()
            .map(|r| r.instances_of_type(TypeId::of::<A>()))
            .unwrap_or_default()
    }

    /// Find the first instance of a specific app type.
    pub fn instance_of<A: App>(&self) -> Option<InstanceId> {
        self.registry
            .as_ref()
            .and_then(|r| r.instance_of_type(TypeId::of::<A>()))
    }

    /// Get the number of instances of a specific app type.
    pub fn instance_count<A: App>(&self) -> usize {
        self.registry
            .as_ref()
            .map(|r| r.instance_count_of_type(TypeId::of::<A>()))
            .unwrap_or(0)
    }

    /// Get the currently focused instance ID.
    pub fn focused_instance_id(&self) -> Option<InstanceId> {
        self.registry.as_ref().and_then(|r| r.focused_instance_id())
    }

    // =========================================================================
    // Toast
    // =========================================================================

    /// Show a toast notification.
    ///
    /// Accepts either a string (creates an info toast) or a Toast directly.
    pub fn toast(&self, toast: impl Into<Toast>) {
        if let Ok(mut inner) = self.inner.write() {
            inner.pending_toasts.push(toast.into());
            self.send_wakeup();
        }
    }

    // =========================================================================
    // Theme
    // =========================================================================

    /// Set the current theme.
    ///
    /// The theme change will take effect on the next render.
    pub fn set_theme(&self, theme: impl Theme + 'static) {
        if let Ok(mut inner) = self.inner.write() {
            inner.theme_request = Some(Arc::new(theme));
            self.send_wakeup();
        }
    }

    // =========================================================================
    // Global Modal
    // =========================================================================

    /// Open a global modal and wait for it to return a result.
    ///
    /// Global modals overlay everything and are not tied to a specific app.
    pub async fn modal<M: Modal>(&self, modal: M) -> M::Result {
        let (tx, rx) = oneshot::channel();

        // Create the modal context with the result sender
        let mx = ModalContext::new(tx);

        // Create the type-erased entry
        let entry = ModalEntry::new(modal, mx);

        // Request the runtime to push this modal
        if let Ok(mut inner) = self.inner.write() {
            inner.modal_request = Some(GlobalModalRequest {
                entry: Box::new(entry),
            });
            self.send_wakeup();
        }

        // Wait for the modal to close and return its result
        rx.await.expect("modal closed without sending result")
    }

    // =========================================================================
    // Inter-App Communication
    // =========================================================================

    /// Publish an event to all non-sleeping app instances.
    ///
    /// Events are delivered asynchronously to all instances that have
    /// an `#[event_handler]` for the event type.
    pub fn publish<E: Event>(&self, event: E) {
        if let Ok(mut inner) = self.inner.write() {
            inner.instance_commands.push(InstanceCommand::PublishEvent {
                event: ArcEvent::new(event),
            });
            self.send_wakeup();
        }
    }

    /// Send a request to the first non-sleeping instance of an app type.
    ///
    /// Returns the response from the handler, or an error if:
    /// - No instance of the app type is running (`NoInstance`)
    /// - The target has no handler for this request type (`NoHandler`)
    /// - The handler panicked (`HandlerPanicked`)
    pub async fn request<A: App, R: Request>(
        &self,
        request: R,
    ) -> Result<R::Response, RequestError> {
        let (tx, rx) = oneshot::channel();

        if let Ok(mut inner) = self.inner.write() {
            inner.instance_commands.push(InstanceCommand::SendRequest {
                target: RequestTarget::AppType(TypeId::of::<A>()),
                request: Box::new(request),
                request_type: TypeId::of::<R>(),
                response_tx: tx,
            });
            self.send_wakeup();
        }

        let response = rx.await.map_err(|_| RequestError::HandlerPanicked)??;
        let response: Box<R::Response> = response
            .downcast()
            .map_err(|_| RequestError::HandlerPanicked)?;
        Ok(*response)
    }

    /// Send a request to a specific instance by ID.
    ///
    /// Returns the response from the handler, or an error if:
    /// - The instance does not exist (`InstanceNotFound`)
    /// - The target has no handler for this request type (`NoHandler`)
    /// - The handler panicked (`HandlerPanicked`)
    pub async fn request_to<R: Request>(
        &self,
        instance_id: InstanceId,
        request: R,
    ) -> Result<R::Response, RequestError> {
        let (tx, rx) = oneshot::channel();

        if let Ok(mut inner) = self.inner.write() {
            inner.instance_commands.push(InstanceCommand::SendRequest {
                target: RequestTarget::Instance(instance_id),
                request: Box::new(request),
                request_type: TypeId::of::<R>(),
                response_tx: tx,
            });
            self.send_wakeup();
        }

        let response = rx.await.map_err(|_| RequestError::HandlerPanicked)??;
        let response: Box<R::Response> = response
            .downcast()
            .map_err(|_| RequestError::HandlerPanicked)?;
        Ok(*response)
    }

    // =========================================================================
    // Global Data
    // =========================================================================

    /// Get a reference to global data of type `T`.
    ///
    /// # Panics
    ///
    /// Panics if no data of this type was registered with `Runtime::data()`.
    pub fn data<T: Send + Sync + 'static>(&self) -> &T {
        self.try_data::<T>().unwrap_or_else(|| {
            panic!(
                "No data of type {} registered. Use Runtime::data() to register it.",
                std::any::type_name::<T>()
            )
        })
    }

    /// Get a reference to global data of type `T`.
    ///
    /// Returns `None` if no data of this type was registered.
    pub fn try_data<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.data
            .get(&TypeId::of::<T>())
            .and_then(|arc| arc.downcast_ref::<T>())
    }

    // =========================================================================
    // Keybinds
    // =========================================================================

    /// Get display info for all keybinds.
    pub fn keybinds(&self) -> Vec<KeybindInfo> {
        self.keybinds
            .read()
            .map(|kb| kb.infos())
            .unwrap_or_default()
    }

    /// Override a keybind's key combination.
    pub fn override_keybind(&self, id: &str, keys: &str) -> Result<(), KeybindError> {
        let mut keybinds = self
            .keybinds
            .write()
            .map_err(|_| KeybindError::ParseError("Failed to acquire keybinds lock".to_string()))?;
        keybinds.override_keybind(id, keys)
    }

    /// Disable a keybind.
    pub fn disable_keybind(&self, id: &str) -> Result<(), KeybindError> {
        let mut keybinds = self
            .keybinds
            .write()
            .map_err(|_| KeybindError::ParseError("Failed to acquire keybinds lock".to_string()))?;
        keybinds.disable_keybind(id)
    }

    /// Reset a keybind to its default key combination.
    pub fn reset_keybind(&self, id: &str) -> Result<(), KeybindError> {
        let mut keybinds = self
            .keybinds
            .write()
            .map_err(|_| KeybindError::ParseError("Failed to acquire keybinds lock".to_string()))?;
        keybinds.reset_keybind(id)
    }

    /// Reset all keybinds to their defaults.
    pub fn reset_all_keybinds(&self) {
        if let Ok(mut keybinds) = self.keybinds.write() {
            keybinds.reset_all();
        }
    }

    // =========================================================================
    // Internal (runtime use)
    // =========================================================================

    /// Check if shutdown was requested (runtime use).
    pub(crate) fn is_shutdown_requested(&self) -> bool {
        self.inner
            .read()
            .map(|inner| inner.shutdown_requested)
            .unwrap_or(false)
    }

    /// Take pending toasts (runtime use).
    pub(crate) fn take_toasts(&self) -> Vec<Toast> {
        self.inner
            .write()
            .ok()
            .map(|mut inner| std::mem::take(&mut inner.pending_toasts))
            .unwrap_or_default()
    }

    /// Take the theme change request (runtime use).
    pub(crate) fn take_theme_request(&self) -> Option<Arc<dyn Theme>> {
        self.inner
            .write()
            .ok()
            .and_then(|mut inner| inner.theme_request.take())
    }

    /// Take the global modal request (runtime use).
    pub(crate) fn take_modal_request(&self) -> Option<GlobalModalRequest> {
        self.inner
            .write()
            .ok()
            .and_then(|mut inner| inner.modal_request.take())
    }

    /// Take pending instance commands (runtime use).
    pub(crate) fn take_instance_commands(&self) -> Vec<InstanceCommand> {
        self.inner
            .write()
            .ok()
            .map(|mut inner| std::mem::take(&mut inner.instance_commands))
            .unwrap_or_default()
    }
}

impl Default for GlobalContext {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(GlobalContextInner::default())),
            keybinds: Arc::new(RwLock::new(Keybinds::new())),
            registry: None,
            data: Arc::new(HashMap::new()),
            wakeup_sender: None,
        }
    }
}

