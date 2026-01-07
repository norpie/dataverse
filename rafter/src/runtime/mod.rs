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
// Toast Animation Constants
// =============================================================================

const SLIDE_DURATION: Duration = Duration::from_millis(400);
const TOAST_WIDTH: u16 = 40;
const TOP_OFFSET: u16 = 1;
const RIGHT_MARGIN: u16 = 1;
const TOAST_GAP: u16 = 1;
const MAX_VISIBLE_TOASTS: usize = 5;

// =============================================================================
// Toast Phase
// =============================================================================

/// Animation phase for a toast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToastPhase {
    /// First frame: render off-screen to establish animation snapshot
    PendingSlideIn,
    /// Animating from off-screen to on-screen
    SlidingIn,
    /// Static display
    Visible,
    /// Animating from on-screen to off-screen
    SlidingOut,
}

// =============================================================================
// ActiveToast
// =============================================================================

/// A toast with its animation state.
struct ActiveToast {
    toast: Toast,
    id: usize,
    phase: ToastPhase,
    phase_started: Instant,
    duration: Duration,
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

        // Set default theme
        terminal.set_theme(Arc::new(crate::theme::default_theme()));

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
        let mut next_toast_id: usize = 0;

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
            &mut next_toast_id,
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
        next_toast_id: &mut usize,
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

            // 3. Process modal requests from focused app and clean up closed modals
            {
                let reg = registry.read().unwrap();
                if let Some(instance) = reg.focused_instance() {
                    // Push any new modal requests
                    let cx = instance.app_context();
                    if let Some(request) = cx.take_modal_request() {
                        instance.push_modal(request.entry);
                    }
                    // Pop closed modals so they don't render
                    if let Ok(mut modals) = instance.modals().write() {
                        while modals.last().map(|m| m.is_closed()).unwrap_or(false) {
                            modals.pop();
                        }
                    }
                }
            }

            // 4. Collect new toasts
            for toast in gx.take_toasts() {
                let duration = toast.duration;
                active_toasts.push(ActiveToast {
                    toast,
                    id: *next_toast_id,
                    phase: ToastPhase::PendingSlideIn,
                    phase_started: Instant::now(),
                    duration,
                });
                *next_toast_id += 1;
            }

            // 5. Apply theme changes
            if let Some(theme) = gx.take_theme_request() {
                terminal.set_theme(theme);
            }

            // 6. Build UI
            let root = self.build_root_element(registry, systems, active_toasts);

            // 7. Render (stores layout internally)
            log::trace!("[runtime] === FRAME START ===");
            terminal.render(&root)?;

            // 8. Update toast phases (AFTER render so animation captures off-screen position first)
            let now = Instant::now();
            for toast in active_toasts.iter_mut() {
                let elapsed = now.duration_since(toast.phase_started);

                match toast.phase {
                    ToastPhase::PendingSlideIn => {
                        // Transition to SlidingIn now that off-screen position is captured
                        log::debug!("Toast {} transitioning PendingSlideIn -> SlidingIn", toast.id);
                        toast.phase = ToastPhase::SlidingIn;
                        toast.phase_started = now;
                    }
                    ToastPhase::SlidingIn if elapsed >= SLIDE_DURATION => {
                        toast.phase = ToastPhase::Visible;
                        toast.phase_started = now;
                    }
                    ToastPhase::Visible if elapsed >= toast.duration => {
                        toast.phase = ToastPhase::SlidingOut;
                        toast.phase_started = now;
                    }
                    _ => {}
                }
            }

            // Remove toasts that have finished sliding out
            active_toasts.retain(|t| {
                if t.phase == ToastPhase::SlidingOut {
                    now.duration_since(t.phase_started) < SLIDE_DURATION
                } else {
                    true
                }
            });

            // 9. Determine poll timeout
            let toast_animating = active_toasts.iter().any(|t| {
                matches!(
                    t.phase,
                    ToastPhase::PendingSlideIn | ToastPhase::SlidingIn | ToastPhase::SlidingOut
                )
            });

            let has_tuidom_anims = terminal.has_active_animations();
            let timeout = if has_tuidom_anims || toast_animating {
                log::trace!(
                    "[runtime] using animation_timeout: tuidom_anims={}, toast_animating={}",
                    has_tuidom_anims,
                    toast_animating
                );
                animation_timeout
            } else {
                // Check when the next toast phase change is due
                let next_phase_change = active_toasts
                    .iter()
                    .filter_map(|t| {
                        match t.phase {
                            ToastPhase::PendingSlideIn => Some(Duration::ZERO), // Immediate
                            ToastPhase::SlidingIn => {
                                Some(SLIDE_DURATION.saturating_sub(now.duration_since(t.phase_started)))
                            }
                            ToastPhase::Visible => {
                                Some(t.duration.saturating_sub(now.duration_since(t.phase_started)))
                            }
                            ToastPhase::SlidingOut => {
                                Some(SLIDE_DURATION.saturating_sub(now.duration_since(t.phase_started)))
                            }
                        }
                    })
                    .min();

                match next_phase_change {
                    Some(dur) if dur < idle_timeout => dur,
                    _ => idle_timeout,
                }
            };

            // 10. Poll events
            let poll_start = Instant::now();
            let raw_events = terminal.poll(Some(timeout))?;
            let poll_duration = poll_start.elapsed();
            if has_tuidom_anims || toast_animating {
                log::trace!(
                    "[runtime] poll returned {} events after {:?} (timeout was {:?})",
                    raw_events.len(),
                    poll_duration,
                    timeout
                );
            }

            // 11. Get layout for event processing (stored from render)
            let layout = terminal.layout();

            // 12. Process focus events (Tab navigation, focus-follows-mouse)
            let events = focus.process_events(&raw_events, &root, layout);

            // 13. Process scroll events
            let _consumed_scroll = scroll.process_events(&events, &root, layout);

            // 14. Dispatch events to keybinds and apps
            for event in &events {
                dispatch::dispatch_event(event, global_modals, systems, registry, gx, layout);
            }

            // 15. Check wakeups (state changes from async tasks)
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

        // Add app modals (centered overlay with dim backdrop)
        {
            let reg = registry.read().unwrap();
            if let Some(instance) = reg.focused_instance() {
                let modals = instance.modals().read().unwrap();
                if let Some(modal) = modals.last() {
                    use tuidom::{Align, Backdrop, Justify, Position, Size};
                    // Center the modal on screen with dimmed backdrop
                    let modal_wrapper = Element::col()
                        .id("__modal__")
                        .position(Position::Absolute)
                        .left(0)
                        .top(0)
                        .width(Size::Fill)
                        .height(Size::Fill)
                        .backdrop(Backdrop::Dim(0.5))
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
        use tuidom::{Easing, Position, Size, Transitions};

        let mut container = Element::col()
            .id("__toasts__")
            .position(Position::Absolute)
            .right(RIGHT_MARGIN as i16)
            .top(TOP_OFFSET as i16)
            .width(Size::Fixed(TOAST_WIDTH))
            .gap(TOAST_GAP);

        // Oldest toasts first (at top), newest at bottom, limit to MAX_VISIBLE_TOASTS
        for active in active_toasts.iter().take(MAX_VISIBLE_TOASTS) {
            // Right position based on phase
            let right: i16 = match active.phase {
                ToastPhase::PendingSlideIn | ToastPhase::SlidingOut => {
                    -(TOAST_WIDTH as i16 + 2)
                }
                ToastPhase::SlidingIn | ToastPhase::Visible => 0,
            };

            log::debug!(
                "Building toast {} with phase {:?}, right={}",
                active.id,
                active.phase,
                right
            );

            let toast_element = Element::box_()
                .id(format!("__toast_{}__", active.id))
                .position(Position::Relative)
                .right(right)
                .width(Size::Fill)
                .child(active.toast.element())
                .transitions(
                    Transitions::new()
                        .x(SLIDE_DURATION, Easing::EaseOut)
                        .y(SLIDE_DURATION, Easing::EaseOut),
                );

            container = container.child(toast_element);
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
