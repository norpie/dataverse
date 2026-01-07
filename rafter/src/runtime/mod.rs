//! Runtime for running rafter apps.
//!
//! The runtime manages:
//! - Terminal and rendering
//! - Focus and scroll state
//! - App instances
//! - Systems (global keybinds and overlays)
//! - Global modals and toasts
//! - Theme
//! - Event loop

pub mod dispatch;

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::io;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use tuidom::{Element, FocusState, ScrollState, Terminal};

use crate::global_context::{DataStore, InstanceCommand, InstanceQuery, RequestTarget};
use crate::instance::{AnyAppInstance, AppInstance, InstanceId, InstanceRegistry, RequestError};
use crate::registration::AnySystem;
use crate::system::System;
use crate::toast::Toast;
use crate::wakeup::{channel as wakeup_channel, WakeupReceiver};
use crate::{App, AppContext, GlobalContext};

use dispatch::AnyModal;

// =============================================================================
// RuntimeError
// =============================================================================

/// Errors that can occur during runtime operation.
#[derive(Debug)]
pub enum RuntimeError {
    /// Terminal I/O error.
    Io(io::Error),
    /// No app was started.
    NoApp,
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::Io(e) => write!(f, "I/O error: {}", e),
            RuntimeError::NoApp => write!(f, "No app was started"),
        }
    }
}

impl std::error::Error for RuntimeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RuntimeError::Io(e) => Some(e),
            RuntimeError::NoApp => None,
        }
    }
}

impl From<io::Error> for RuntimeError {
    fn from(e: io::Error) -> Self {
        RuntimeError::Io(e)
    }
}

// =============================================================================
// ActiveToast
// =============================================================================

/// A toast with its expiration time.
struct ActiveToast {
    toast: Toast,
    expires_at: Instant,
}

// =============================================================================
// Runtime
// =============================================================================

/// The rafter runtime.
///
/// Manages the terminal, app instances, systems, and event loop.
///
/// # Example
///
/// ```ignore
/// let runtime = Runtime::new()?
///     .system(MyTaskbar::new())
///     .data(MyGlobalData::new());
///
/// runtime.run(MyApp::new()).await?;
/// ```
pub struct Runtime {
    /// Global data store (set before run).
    data: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
    /// Systems to install.
    systems: Vec<Box<dyn AnySystem>>,
}

impl Runtime {
    /// Create a new runtime.
    pub fn new() -> Result<Self, RuntimeError> {
        Ok(Self {
            data: HashMap::new(),
            systems: Vec::new(),
        })
    }

    /// Add a system to the runtime.
    ///
    /// Systems provide global keybinds and optional overlays (taskbar, status bar, etc.).
    pub fn system<S: System>(mut self, system: S) -> Self {
        self.systems.push(Box::new(system));
        self
    }

    /// Register global data.
    ///
    /// Global data can be accessed from any handler via `gx.data::<T>()`.
    pub fn data<T: Send + Sync + 'static>(mut self, data: T) -> Self {
        self.data.insert(TypeId::of::<T>(), Arc::new(data));
        self
    }

    /// Run the runtime with an initial app.
    ///
    /// This is the main entry point. The runtime will:
    /// 1. Initialize the terminal
    /// 2. Spawn the initial app
    /// 3. Run the event loop until shutdown
    pub async fn run<A: App>(mut self, app: A) -> Result<(), RuntimeError> {
        // Initialize terminal
        let mut terminal = Terminal::new()?;

        // Create focus and scroll state
        let mut focus = FocusState::new();
        let mut scroll = ScrollState::new();

        // Create instance registry (wrapped in Arc for GlobalContext access)
        let registry = Arc::new(RwLock::new(InstanceRegistry::new()));

        // Create wakeup channel
        let (wakeup_tx, wakeup_rx) = wakeup_channel();

        // Create global context
        let data_store: Arc<DataStore> = Arc::new(std::mem::take(&mut self.data));
        let mut gx = GlobalContext::new(Arc::clone(&data_store));
        gx.set_wakeup_sender(wakeup_tx.clone());

        // Create registry query wrapper
        let registry_query = RegistryQuery(Arc::clone(&registry));
        gx.set_registry(Arc::new(registry_query));

        // Initialize systems (take ownership from self)
        let mut systems: Vec<Box<dyn AnySystem>> = std::mem::take(&mut self.systems);
        for system in &systems {
            system.on_init();
            system.install_wakeup(wakeup_tx.clone());
        }

        // Spawn initial app
        let instance = AppInstance::new(app, gx.clone());
        let instance_id = instance.id();
        instance.install_wakeup(wakeup_tx.clone(), &gx);

        {
            let mut reg = registry.write().unwrap();
            reg.insert(Box::new(instance));
            reg.focus(instance_id);
        }

        // Active toasts
        let mut active_toasts: Vec<ActiveToast> = Vec::new();

        // Global modals
        let mut global_modals: Vec<Box<dyn AnyModal>> = Vec::new();

        // Run event loop
        let mut wakeup_rx = wakeup_rx;
        self.event_loop(
            &mut terminal,
            &mut focus,
            &mut scroll,
            &registry,
            &mut systems,
            &gx,
            &mut wakeup_rx,
            &mut active_toasts,
            &mut global_modals,
        )
        .await
    }

    /// The main event loop.
    #[allow(clippy::too_many_arguments)]
    async fn event_loop(
        &self,
        terminal: &mut Terminal,
        focus: &mut FocusState,
        scroll: &mut ScrollState,
        registry: &Arc<RwLock<InstanceRegistry>>,
        systems: &mut Vec<Box<dyn AnySystem>>,
        gx: &GlobalContext,
        wakeup_rx: &mut WakeupReceiver,
        active_toasts: &mut Vec<ActiveToast>,
        global_modals: &mut Vec<Box<dyn AnyModal>>,
    ) -> Result<(), RuntimeError> {
        // Default poll timeout (16ms for ~60fps when animations active)
        let animation_timeout = Duration::from_millis(16);
        let idle_timeout = Duration::from_millis(100);

        loop {
            // 1. Check shutdown
            if gx.is_shutdown_requested() {
                break;
            }

            // Check if any instances remain
            {
                let reg = registry.read().unwrap();
                if reg.is_empty() {
                    break;
                }
            }

            // 2. Process pending commands
            self.process_commands(registry, gx)?;

            // 3. Process modal requests from focused app
            {
                let reg = registry.read().unwrap();
                if let Some(instance) = reg.focused_instance() {
                    let cx = instance.app_context();
                    if let Some(request) = cx.take_modal_request() {
                        instance.push_modal(request.entry);
                    }
                }
            }

            // 4. Collect new toasts
            for toast in gx.take_toasts() {
                let duration = toast.duration;
                active_toasts.push(ActiveToast {
                    toast,
                    expires_at: Instant::now() + duration,
                });
            }

            // Remove expired toasts
            let now = Instant::now();
            active_toasts.retain(|t| t.expires_at > now);

            // 4. Apply theme changes
            if let Some(theme) = gx.take_theme_request() {
                // We need to extract the theme from Arc<dyn Theme>
                // Terminal::set_theme takes impl Theme + 'static
                // We'll need a different approach - store theme in runtime state
                // For now, skip theme changes (TODO: fix this)
                let _ = theme;
            }

            // 5. Build UI
            let root = self.build_root_element(registry, systems, active_toasts);

            // 6. Render (stores layout internally)
            terminal.render(&root)?;

            // 7. Determine poll timeout
            let timeout = if terminal.has_active_animations() {
                animation_timeout
            } else {
                // Check if any toast will expire soon
                let next_toast_expiry = active_toasts
                    .iter()
                    .map(|t| t.expires_at)
                    .min()
                    .map(|exp| exp.saturating_duration_since(now));

                match next_toast_expiry {
                    Some(dur) if dur < idle_timeout => dur,
                    _ => idle_timeout,
                }
            };

            // 8. Poll events
            let raw_events = terminal.poll(Some(timeout))?;

            // 9. Get layout for event processing (stored from render)
            let layout = terminal.layout();

            // 10. Process focus events (Tab navigation, focus-follows-mouse)
            let events = focus.process_events(&raw_events, &root, layout);

            // 11. Process scroll events
            let _consumed_scroll = scroll.process_events(&events, &root, layout);

            // 12. Dispatch events to keybinds and apps
            for event in &events {
                dispatch::dispatch_event(event, global_modals, systems, registry, gx, layout);
            }

            // 13. Check wakeups (state changes from async tasks)
            while wakeup_rx.try_recv() {
                // Just drain the wakeup queue - we'll re-render on next iteration
            }
        }

        Ok(())
    }

    /// Build the root element tree.
    fn build_root_element(
        &self,
        registry: &Arc<RwLock<InstanceRegistry>>,
        systems: &[Box<dyn AnySystem>],
        active_toasts: &[ActiveToast],
    ) -> Element {
        use tuidom::{Position, Size};

        let mut root = Element::col().width(Size::Fill).height(Size::Fill);

        // Collect edge overlays (top, bottom, left, right)
        let mut top_overlays: Vec<Element> = Vec::new();
        let mut bottom_overlays: Vec<Element> = Vec::new();

        for system in systems {
            if let Some(overlay) = system.overlay() {
                match overlay.position {
                    crate::system::OverlayPosition::Top { .. } => {
                        top_overlays.push(overlay.element);
                    }
                    crate::system::OverlayPosition::Bottom { .. } => {
                        bottom_overlays.push(overlay.element);
                    }
                    crate::system::OverlayPosition::Left { .. }
                    | crate::system::OverlayPosition::Right { .. } => {
                        // TODO: Handle left/right overlays
                    }
                    crate::system::OverlayPosition::Absolute { x, y, .. } => {
                        // Absolute overlays rendered later
                        root = root.child(
                            overlay
                                .element
                                .position(Position::Absolute)
                                .left(x as i16)
                                .top(y as i16),
                        );
                    }
                }
            }
        }

        // Add top overlays
        for overlay in top_overlays {
            root = root.child(overlay);
        }

        // Add focused app content
        {
            let reg = registry.read().unwrap();
            if let Some(instance) = reg.focused_instance() {
                let app_element = instance.element();
                root = root.child(app_element.width(Size::Fill).height(Size::Fill));
            }
        }

        // Add bottom overlays
        for overlay in bottom_overlays {
            root = root.child(overlay);
        }

        // Add app modals (centered overlay)
        {
            let reg = registry.read().unwrap();
            if let Some(instance) = reg.focused_instance() {
                let modals = instance.modals().read().unwrap();
                if let Some(modal) = modals.last() {
                    use tuidom::{Align, Justify, Position, Size};
                    // Center the modal on screen
                    let modal_wrapper = Element::col()
                        .id("__modal__")
                        .position(Position::Absolute)
                        .left(0)
                        .top(0)
                        .width(Size::Fill)
                        .height(Size::Fill)
                        .justify(Justify::Center)
                        .align(Align::Center)
                        .child(modal.element());
                    root = root.child(modal_wrapper);
                }
            }
        }

        // Add toasts (absolute positioned, stacked from bottom-right)
        if !active_toasts.is_empty() {
            let toast_container = self.build_toast_container(active_toasts);
            root = root.child(toast_container);
        }

        root
    }

    /// Build the toast container element.
    fn build_toast_container(&self, active_toasts: &[ActiveToast]) -> Element {
        use tuidom::{Align, Position, Size};

        let mut container = Element::col()
            .id("__toasts__")
            .position(Position::Absolute)
            .right(1)
            .bottom(1)
            .width(Size::Fixed(40))
            .gap(1)
            .align(Align::End);

        for active in active_toasts.iter().rev().take(5) {
            container = container.child(active.toast.element());
        }

        container
    }

    /// Process pending commands from GlobalContext.
    fn process_commands(
        &self,
        registry: &Arc<RwLock<InstanceRegistry>>,
        gx: &GlobalContext,
    ) -> Result<(), RuntimeError> {
        let commands = gx.take_instance_commands();

        for command in commands {
            match command {
                InstanceCommand::Spawn { app, focus } => {
                    let instance = app.into_instance(gx.clone());
                    let id = instance.id();

                    // Install wakeup sender
                    if let Some(sender) = gx.wakeup_sender() {
                        instance.install_wakeup(sender, gx);
                    }

                    let mut reg = registry.write().unwrap();
                    reg.insert(instance);
                    if focus {
                        reg.focus(id);
                    }
                }

                InstanceCommand::Close { id, force: _ } => {
                    // TODO: Handle on_close_request for non-forced closes
                    let mut reg = registry.write().unwrap();
                    reg.close(id);
                }

                InstanceCommand::Focus { id } => {
                    let mut reg = registry.write().unwrap();
                    reg.focus(id);
                }

                InstanceCommand::PublishEvent { event } => {
                    let reg = registry.read().unwrap();
                    let event_type = event.type_id();

                    for instance in reg.iter() {
                        if !instance.is_sleeping() && instance.has_event_handler(event_type) {
                            // Create AppContext for this instance
                            let cx = AppContext::new(instance.id(), gx.clone(), instance.config().name);
                            instance.dispatch_event(event_type, event.as_ref(), &cx, gx);
                        }
                    }
                }

                InstanceCommand::SendRequest {
                    target,
                    request,
                    request_type,
                    response_tx,
                } => {
                    let result = self.handle_request(registry, gx, target, request, request_type);
                    let _ = response_tx.send(result);
                }
            }
        }

        Ok(())
    }

    /// Handle a request command.
    fn handle_request(
        &self,
        registry: &Arc<RwLock<InstanceRegistry>>,
        gx: &GlobalContext,
        target: RequestTarget,
        request: Box<dyn Any + Send + Sync>,
        request_type: TypeId,
    ) -> Result<Box<dyn Any + Send + Sync>, RequestError> {
        let reg = registry.read().unwrap();

        // Find target instance
        let instance_id = match target {
            RequestTarget::AppType(target_type_id) => {
                let mut found_id = None;
                for instance in reg.iter() {
                    if instance.type_id() == target_type_id && !instance.is_sleeping() {
                        found_id = Some(instance.id());
                        break;
                    }
                }
                found_id.ok_or(RequestError::NoInstance)?
            }
            RequestTarget::Instance(id) => id,
        };

        let instance = reg.get(instance_id).ok_or(RequestError::InstanceNotFound)?;

        if !instance.has_request_handler(request_type) {
            return Err(RequestError::NoHandler);
        }

        // Create AppContext for target instance
        let cx = AppContext::new(instance_id, gx.clone(), instance.config().name);

        // Dispatch request - for now we don't support async request handlers
        // This would need to be spawned as a task
        let future = instance.dispatch_request(request_type, request, &cx, gx);

        match future {
            Some(_future) => {
                // TODO: Spawn the future and wait for result
                // For now, return NoHandler since we can't block here
                Err(RequestError::NoHandler)
            }
            None => Err(RequestError::NoHandler),
        }
    }

}

impl Default for Runtime {
    fn default() -> Self {
        Self::new().expect("Failed to create runtime")
    }
}

// =============================================================================
// RegistryQuery
// =============================================================================

/// Wrapper to implement InstanceQuery for Arc<RwLock<InstanceRegistry>>.
struct RegistryQuery(Arc<RwLock<InstanceRegistry>>);

impl InstanceQuery for RegistryQuery {
    fn instances(&self) -> Vec<crate::instance::InstanceInfo> {
        self.0
            .read()
            .map(|r| r.instances())
            .unwrap_or_default()
    }

    fn instances_of_type(&self, type_id: TypeId) -> Vec<crate::instance::InstanceInfo> {
        self.0
            .read()
            .map(|r| r.instances_of_type(type_id))
            .unwrap_or_default()
    }

    fn instance_of_type(&self, type_id: TypeId) -> Option<InstanceId> {
        self.0.read().ok().and_then(|r| r.instance_of_type(type_id))
    }

    fn instance_count_of_type(&self, type_id: TypeId) -> usize {
        self.0
            .read()
            .map(|r| r.instance_count_of_type(type_id))
            .unwrap_or(0)
    }

    fn focused_instance_id(&self) -> Option<InstanceId> {
        self.0.read().ok().and_then(|r| r.focused_instance_id())
    }
}
