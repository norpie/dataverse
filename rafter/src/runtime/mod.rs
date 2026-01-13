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
mod enrich;

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::io;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use tuidom::{scroll::find_scrollable_ancestor, Content, Element, FocusState, LayoutResult, ScrollState, Terminal, TextInputState};

use enrich::enrich_elements;

use crate::global_context::{DataStore, InstanceCommand, InstanceQuery, RequestTarget};
use crate::instance::{AnyAppInstance, AppInstance, InstanceId, InstanceRegistry, RequestError};
use crate::registration::{registered_systems, AnySystem};
use crate::system::System;
use crate::toast::Toast;
use crate::wakeup::{channel as wakeup_channel, WakeupReceiver};
use crate::event::{FocusChanged, InstanceClosed, InstanceSpawned};
use crate::{App, AppContext, BlurPolicy, GlobalContext, HandlerContext};

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

        // Create focus, scroll, and text input state
        let mut focus = FocusState::new();
        let mut scroll = ScrollState::new();
        let mut text_inputs = TextInputState::new();

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

        // Initialize systems (merge manually added with auto-registered)
        let mut systems: Vec<Box<dyn AnySystem>> = std::mem::take(&mut self.systems);

        // Add auto-registered systems from inventory
        for reg in registered_systems() {
            systems.push((reg.factory)());
        }

        for system in &systems {
            let hx = HandlerContext::for_system(&gx);
            system.lifecycle_hooks().call_on_start(&hx);
            system.install_wakeup(wakeup_tx.clone());
        }

        // Spawn initial app
        let instance = AppInstance::new(app, gx.clone());
        let instance_id = instance.id();
        let instance_name = instance.config().name;
        instance.install_wakeup(wakeup_tx.clone(), &gx);

        // Call on_start lifecycle method
        let cx = instance.app_context();
        let hx = HandlerContext::for_app(&cx, &gx);
        instance.lifecycle_hooks().call_on_start(&hx);

        {
            let mut reg = registry.write().unwrap();
            reg.insert(Box::new(instance));
            reg.focus(instance_id);
        }

        // Publish events for initial app (after systems are initialized)
        gx.publish(InstanceSpawned { id: instance_id, name: instance_name });
        gx.publish(FocusChanged { old: None, new: instance_id });

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
            &mut text_inputs,
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
        text_inputs: &mut TextInputState,
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

        // Track drag target for mouse capture (send all drag events to click target until release)
        let mut drag_target: Option<String> = None;

        loop {
            // 1. Check shutdown
            if gx.is_shutdown_requested() {
                log::debug!("[runtime] Exiting: shutdown requested");
                break;
            }

            // Check if any instances remain
            {
                let reg = registry.read().unwrap();
                if reg.is_empty() {
                    log::debug!("[runtime] Exiting: no instances remaining");
                    break;
                }
            }

            // 2. Process pending commands
            self.process_commands(registry, systems, gx).await?;

            // 3. Process modal requests from focused app and clean up closed modals
            // Take modal request outside the lock so we can await on_start
            let modal_request = {
                let reg = registry.read().unwrap();
                reg.focused_instance()
                    .and_then(|i| i.app_context().take_modal_request())
            };
            if let Some(request) = modal_request {
                // Call on_start before pushing
                // Use appropriate context based on modal kind
                match request.entry.kind() {
                    crate::ModalKind::App => {
                        let reg = registry.read().unwrap();
                        if let Some(instance) = reg.focused_instance() {
                            let cx = instance.app_context();
                            let hx = HandlerContext::for_modal_any(&cx, gx, request.entry.modal_context());
                            request.entry.lifecycle_hooks().call_on_start(&hx);
                        }
                    }
                    crate::ModalKind::System => {
                        // System modals don't get AppContext even when spawned from an app
                        let hx = HandlerContext::for_system_modal(gx, request.entry.modal_context());
                        request.entry.lifecycle_hooks().call_on_start(&hx);
                    }
                }
                // Re-acquire lock to push
                let reg = registry.read().unwrap();
                if let Some(instance) = reg.focused_instance() {
                    instance.push_modal(request.entry);
                }
            }
            // Clean up closed modals
            {
                let reg = registry.read().unwrap();
                if let Some(instance) = reg.focused_instance() {
                    if let Ok(mut modals) = instance.modals().write() {
                        while modals.last().map(|m| m.is_closed()).unwrap_or(false) {
                            modals.pop();
                        }
                    }
                }
            }

            // 3b. Process global modal requests and clean up closed global modals
            if let Some(request) = gx.take_modal_request() {
                // Global modals don't have app context, use system modal context
                let hx = HandlerContext::for_system_modal(gx, request.entry.modal_context());
                request.entry.lifecycle_hooks().call_on_start(&hx);
                global_modals.push(request.entry);
            }
            while global_modals.last().map(|m| m.is_closed()).unwrap_or(false) {
                global_modals.pop();
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
            log::debug!("[runtime] Building root element...");
            let mut root = self.build_root_element(registry, systems, active_toasts, global_modals);
            log::debug!("[runtime] Root element built successfully");

            // 7. Process cursor position requests (before enrichment uses TextInputState)
            {
                let reg = registry.read().unwrap();
                if let Some(instance) = reg.focused_instance() {
                    let cx = instance.app_context();
                    for (element_id, position) in cx.take_cursor_requests() {
                        // get_data_mut creates entry if not exists, so check if input exists first
                        if text_inputs.get_data(&element_id).is_some() {
                            let data = text_inputs.get_data_mut(&element_id);
                            data.cursor = position.min(data.text.chars().count());
                            data.clear_selection();
                        }
                    }
                }
            }

            // 8. Enrich elements with runtime state (BEFORE render)
            enrich_elements(&mut root, focus, text_inputs, scroll);

            // 9. Render (stores layout internally)
            log::trace!("[runtime] === FRAME START ===");
            terminal.render(&root)?;

            // 9b. Dispatch on_layout handlers for elements that have them
            {
                let layout = terminal.layout();
                let reg = registry.read().unwrap();
                if let Some(instance) = reg.focused_instance() {
                    let cx = instance.app_context();
                    let handlers = instance.handlers();
                    let app_name = instance.config().name;
                    let instance_id = instance.id();
                    for (id, rect) in layout.iter_rects() {
                        if let Some(handler) = handlers.get(id, "on_layout") {
                            let hx = crate::HandlerContext::for_app_with_event(
                                &cx,
                                gx,
                                crate::handler_context::EventData::Layout {
                                    x: rect.x,
                                    y: rect.y,
                                    width: rect.width,
                                    height: rect.height,
                                },
                            );
                            let _ = crate::handler_context::call_handler_for_app(&handler, &hx, app_name, instance_id);
                        }
                    }
                }
            }

            // 10. Update toast phases (AFTER render so animation captures off-screen position first)
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

            // 11. Determine poll timeout
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

            // 12. Poll events
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

            // 13. Get layout for event processing (stored from render)
            let layout = terminal.layout();

            // 14. Process scroll requests (needs layout)
            let mut focus_scroll_changes: Vec<tuidom::ScrollChange> = Vec::new();
            {
                let reg = registry.read().unwrap();
                if let Some(instance) = reg.focused_instance() {
                    let cx = instance.app_context();
                    // Process scroll requests
                    for target_id in cx.take_scroll_requests() {
                        if let Some(change) = scroll_to_element(&root, layout, scroll, &target_id) {
                            focus_scroll_changes.push(change);
                        }
                    }
                    // Process focus requests from widgets/app
                    if let Some(target_id) = cx.take_focus_request() {
                        log::debug!("[runtime] Processing focus request: {}", target_id);
                        if focus.focus(&target_id) {
                            log::debug!("[runtime] Focus changed to: {}", target_id);
                            // Scroll the newly focused element into view
                            if let Some(change) = scroll_to_element(&root, layout, scroll, &target_id) {
                                focus_scroll_changes.push(change);
                            }
                        }
                    }
                }
            }

            // 15. Sync text input values to TextInputState
            sync_text_inputs(&root, text_inputs);

            // 16. Process scrollbar drag events (before focus so clicks on scrollbar don't propagate)
            let (raw_events, scrollbar_changes) = scroll.process_raw_events(&raw_events, &root, layout);

            // 17. Process focus events (Tab navigation, focus-follows-mouse)
            let events = focus.process_events(&raw_events, &root, layout);

            // 17b. Scroll focused element into view
            for event in &events {
                if let tuidom::Event::Focus { target } = event {
                    if let Some(change) = scroll_to_element(&root, layout, scroll, target) {
                        focus_scroll_changes.push(change);
                    }
                }
            }

            // 18. Process text input events (keyboard → Change/Submit events)
            let events = text_inputs.process_events(&events, &root, layout);

            // 19. Process scroll events (wheel scrolling)
            let mut scroll_changes = scroll.process_events(&events, &root, layout);
            // Include focus-triggered scroll changes and scrollbar drag changes
            scroll_changes.extend(focus_scroll_changes);
            scroll_changes.extend(scrollbar_changes);

            // 19b. Dispatch on_scroll handlers for elements that scrolled
            if !scroll_changes.is_empty() {
                let reg = registry.read().unwrap();
                if let Some(instance) = reg.focused_instance() {
                    let cx = instance.app_context();
                    let handlers = instance.handlers();
                    let app_name = instance.config().name;
                    let instance_id = instance.id();
                    for change in &scroll_changes {
                        if let Some(handler) = handlers.get(&change.element_id, "on_scroll") {
                            let hx = crate::HandlerContext::for_app_with_event(
                                &cx,
                                gx,
                                crate::handler_context::EventData::Scroll {
                                    offset_x: change.offset_x,
                                    offset_y: change.offset_y,
                                    content_width: change.content_width,
                                    content_height: change.content_height,
                                    viewport_width: change.viewport_width,
                                    viewport_height: change.viewport_height,
                                },
                            );
                            let _ = crate::handler_context::call_handler_for_app(&handler, &hx, app_name, instance_id);
                        }
                    }
                }
            }

            // 19c. Apply drag capture - redirect drag events to the click target
            let events: Vec<tuidom::Event> = events.into_iter().map(|event| {
                match &event {
                    tuidom::Event::Click { target, .. } => {
                        // Store click target for drag capture
                        drag_target = target.clone();
                        event
                    }
                    tuidom::Event::Drag { target: _, x, y, button } => {
                        // Redirect drag to the captured target
                        if let Some(ref captured) = drag_target {
                            tuidom::Event::Drag {
                                target: Some(captured.clone()),
                                x: *x,
                                y: *y,
                                button: *button,
                            }
                        } else {
                            event
                        }
                    }
                    tuidom::Event::Release { target: _, x, y, button } => {
                        // Redirect release to the captured target, then clear it
                        let result = if let Some(ref captured) = drag_target {
                            tuidom::Event::Release {
                                target: Some(captured.clone()),
                                x: *x,
                                y: *y,
                                button: *button,
                            }
                        } else {
                            event
                        };
                        drag_target = None;
                        result
                    }
                    _ => event,
                }
            }).collect();

            // 20. Dispatch events to keybinds and apps
            for event in &events {
                let result = dispatch::dispatch_event(event, global_modals, systems, registry, gx, layout);

                // Handle panics according to PanicBehavior
                if let dispatch::DispatchResult::HandlerPanicked { message } = result {
                    let reg = registry.read().unwrap();
                    if let Some(instance) = reg.focused_instance() {
                        let behavior = instance.config().panic_behavior;
                        let app_name = instance.config().name;
                        let instance_id = instance.id();
                        drop(reg); // Release lock before taking action

                        match behavior {
                            crate::PanicBehavior::Close => {
                                log::warn!(
                                    "[{}:{}] Closing instance due to handler panic (PanicBehavior::Close): {}",
                                    app_name, instance_id, message
                                );
                                let mut reg = registry.write().unwrap();
                                reg.close(instance_id);
                            }
                            crate::PanicBehavior::Restart => {
                                log::warn!(
                                    "[{}:{}] Restart not yet implemented for handler panics (PanicBehavior::Restart): {}",
                                    app_name, instance_id, message
                                );
                                // TODO: Implement restart - requires creating new instance
                            }
                            crate::PanicBehavior::Ignore => {
                                log::warn!(
                                    "[{}:{}] Ignoring handler panic (PanicBehavior::Ignore): {}",
                                    app_name, instance_id, message
                                );
                            }
                        }
                    }
                }
            }

            // 20b. Process focus requests from handlers (e.g., list boundary scrolling)
            // This runs AFTER dispatch so handler focus requests take effect immediately
            {
                let reg = registry.read().unwrap();
                if let Some(instance) = reg.focused_instance() {
                    let cx = instance.app_context();
                    if let Some(target_id) = cx.take_focus_request() {
                        log::debug!("[runtime] Processing post-dispatch focus request: {}", target_id);
                        if focus.focus(&target_id) {
                            log::debug!("[runtime] Focus changed to: {}", target_id);
                        }
                    }
                }
            }

            // 21. Check wakeups (state changes from async tasks)
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
        global_modals: &[Box<dyn AnyModal>],
    ) -> Element {
        use tuidom::{Position, Size};

        let mut root = Element::col().width(Size::Fill).height(Size::Fill);

        // Collect edge overlays (top, bottom, left, right)
        let mut top_overlays: Vec<Element> = Vec::new();
        let mut bottom_overlays: Vec<Element> = Vec::new();
        let mut left_overlays: Vec<Element> = Vec::new();
        let mut right_overlays: Vec<Element> = Vec::new();

        for system in systems {
            if let Some(overlay) = system.overlay() {
                match overlay.position {
                    crate::system::OverlayPosition::Top { height } => {
                        top_overlays.push(overlay.element.height(Size::Fixed(height)));
                    }
                    crate::system::OverlayPosition::Bottom { height } => {
                        bottom_overlays.push(overlay.element.height(Size::Fixed(height)));
                    }
                    crate::system::OverlayPosition::Left { width } => {
                        left_overlays.push(overlay.element.width(Size::Fixed(width)));
                    }
                    crate::system::OverlayPosition::Right { width } => {
                        right_overlays.push(overlay.element.width(Size::Fixed(width)));
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

        // Build middle section: left overlays + app content + right overlays
        let mut middle_row = Element::row().width(Size::Fill).height(Size::Fill);

        // Add left overlays
        for overlay in left_overlays {
            middle_row = middle_row.child(overlay.height(Size::Fill));
        }

        // Add focused app content
        {
            let reg = registry.read().unwrap();
            if let Some(instance) = reg.focused_instance() {
                log::debug!("[runtime] Calling instance.element() for {}", instance.config().name);
                let app_element = instance.element();
                log::debug!("[runtime] instance.element() returned");
                middle_row = middle_row.child(app_element.width(Size::Fill).height(Size::Fill));
            }
        }

        // Add right overlays
        for overlay in right_overlays {
            middle_row = middle_row.child(overlay.height(Size::Fill));
        }

        root = root.child(middle_row);

        // Add bottom overlays
        for overlay in bottom_overlays {
            root = root.child(overlay);
        }

        // Add app modals (overlay with dim backdrop)
        {
            let reg = registry.read().unwrap();
            if let Some(instance) = reg.focused_instance() {
                let modals = instance.modals().read().unwrap();
                if let Some(modal) = modals.last() {
                    let modal_wrapper = Self::build_modal_wrapper(
                        "__modal__",
                        modal.as_ref(),
                    );
                    root = root.child(modal_wrapper);
                }
            }
        }

        // Add global modals (highest z-order, overlays everything including app modals)
        if let Some(modal) = global_modals.last() {
            let modal_wrapper = Self::build_modal_wrapper(
                "__global_modal__",
                modal.as_ref(),
            );
            root = root.child(modal_wrapper);
        }

        // Add toasts (absolute positioned, stacked from bottom-right)
        if !active_toasts.is_empty() {
            let toast_container = self.build_toast_container(active_toasts);
            root = root.child(toast_container);
        }

        root
    }

    /// Build a modal wrapper element with proper size and position.
    fn build_modal_wrapper(id: &str, modal: &dyn AnyModal) -> Element {
        use tuidom::{Align, Backdrop, Justify, Position, Size};

        let modal_size = modal.size();
        let modal_position = modal.position();

        // Convert ModalSize to tuidom Size
        let (width, height) = match modal_size {
            crate::ModalSize::Auto => (Size::Auto, Size::Auto),
            crate::ModalSize::Sm => (Size::Percent(0.3), Size::Percent(0.3)),
            crate::ModalSize::Md => (Size::Percent(0.5), Size::Percent(0.5)),
            crate::ModalSize::Lg => (Size::Percent(0.8), Size::Percent(0.8)),
            crate::ModalSize::Fixed { width, height } => {
                (Size::Fixed(width), Size::Fixed(height))
            }
            crate::ModalSize::Proportional { width, height } => {
                (Size::Percent(width), Size::Percent(height))
            }
        };

        // Get the modal content element and apply size
        let modal_content = modal.element().width(width).height(height);

        // Build wrapper based on position
        match modal_position {
            crate::ModalPosition::Centered => {
                Element::col()
                    .id(id)
                    .position(Position::Absolute)
                    .left(0)
                    .top(0)
                    .width(Size::Fill)
                    .height(Size::Fill)
                    .z_index(1000)
                    .backdrop(Backdrop::Dim(0.5))
                    .justify(Justify::Center)
                    .align(Align::Center)
                    .child(modal_content)
            }
            crate::ModalPosition::At { x, y } => {
                // For absolute positioning, we still need the full-screen backdrop
                // but position the modal content at specific coordinates
                Element::col()
                    .id(id)
                    .position(Position::Absolute)
                    .left(0)
                    .top(0)
                    .width(Size::Fill)
                    .height(Size::Fill)
                    .z_index(1000)
                    .backdrop(Backdrop::Dim(0.5))
                    .child(
                        modal_content
                            .position(Position::Absolute)
                            .left(x as i16)
                            .top(y as i16),
                    )
            }
        }
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
    async fn process_commands(
        &self,
        registry: &Arc<RwLock<InstanceRegistry>>,
        systems: &[Box<dyn AnySystem>],
        gx: &GlobalContext,
    ) -> Result<(), RuntimeError> {
        let commands = gx.take_instance_commands();

        for command in commands {
            match command {
                InstanceCommand::Spawn { app, focus } => {
                    let instance = app.into_instance(gx.clone());
                    let id = instance.id();
                    let name = instance.config().name;

                    // Install wakeup sender
                    if let Some(sender) = gx.wakeup_sender() {
                        instance.install_wakeup(sender, gx);
                    }

                    // Call on_start lifecycle method
                    {
                        let cx = instance.app_context();
                        let hx = HandlerContext::for_app(&cx, gx);
                        instance.lifecycle_hooks().call_on_start(&hx);
                    }

                    {
                        let mut reg = registry.write().unwrap();
                        reg.insert(instance);
                        if focus {
                            reg.focus(id);
                        }
                    }

                    // Publish InstanceSpawned event
                    gx.publish(InstanceSpawned { id, name });
                }

                InstanceCommand::Close { id, force: _ } => {
                    // TODO: Handle on_close_request for non-forced closes
                    let name = {
                        let reg = registry.read().unwrap();
                        reg.get(id).map(|i| i.config().name)
                    };

                    {
                        let mut reg = registry.write().unwrap();
                        reg.close(id);
                    }

                    // Publish InstanceClosed event
                    if let Some(name) = name {
                        gx.publish(InstanceClosed { id, name });
                    }
                }

                InstanceCommand::Focus { id } => {
                    // Get old focused instance before changing focus
                    let old_focused = {
                        let reg = registry.read().unwrap();
                        reg.focused()
                    };

                    // Change focus
                    {
                        let mut reg = registry.write().unwrap();
                        reg.focus(id);
                    }

                    // Apply blur policy to old instance
                    let mut to_close = None;
                    if let Some(old_id) = old_focused {
                        if old_id != id {
                            // Get blur policy
                            let blur_policy = {
                                let reg = registry.read().unwrap();
                                reg.get(old_id).map(|i| i.config().on_blur)
                            };

                            // Call on_background lifecycle hook
                            {
                                let reg = registry.read().unwrap();
                                if let Some(instance) = reg.get(old_id) {
                                    let cx = instance.app_context();
                                    let hx = HandlerContext::for_app(&cx, gx);
                                    instance.lifecycle_hooks().call_on_background(&hx);
                                }
                            }

                            // Apply blur policy
                            match blur_policy {
                                Some(BlurPolicy::Sleep) => {
                                    let reg = registry.read().unwrap();
                                    if let Some(instance) = reg.get(old_id) {
                                        instance.set_sleeping(true);
                                    }
                                }
                                Some(BlurPolicy::Close) => {
                                    to_close = Some(old_id);
                                }
                                _ => {}
                            }
                        }
                    }

                    // Wake new instance if it was sleeping and call on_foreground
                    {
                        let reg = registry.read().unwrap();
                        if let Some(instance) = reg.get(id) {
                            if instance.is_sleeping() {
                                instance.set_sleeping(false);
                            }
                            let cx = instance.app_context();
                            let hx = HandlerContext::for_app(&cx, gx);
                            instance.lifecycle_hooks().call_on_foreground(&hx);
                        }
                    }

                    // Close old instance if BlurPolicy::Close
                    if let Some(close_id) = to_close {
                        let mut reg = registry.write().unwrap();
                        reg.close(close_id);
                    }

                    // Publish FocusChanged event
                    gx.publish(FocusChanged { old: old_focused, new: id });
                }

                InstanceCommand::PublishEvent { event } => {
                    let event_type = event.type_id();

                    // Dispatch to app instances
                    {
                        let reg = registry.read().unwrap();
                        log::debug!("[PublishEvent] event_type={:?}, instances={}", event_type, reg.len());

                        for instance in reg.iter() {
                            let has_handler = instance.has_event_handler(event_type);
                            log::debug!(
                                "[PublishEvent] instance={} sleeping={} has_handler={}",
                                instance.config().name, instance.is_sleeping(), has_handler
                            );
                            if !instance.is_sleeping() && has_handler {
                                // Create AppContext for this instance
                                let cx = AppContext::new(instance.id(), gx.clone(), instance.config().name);
                                let handled = instance.dispatch_event(event_type, event.as_ref(), &cx, gx);
                                log::debug!("[PublishEvent] dispatched to {}, handled={}", instance.config().name, handled);
                            }
                        }
                    }

                    // Dispatch to systems
                    for system in systems.iter() {
                        let has_handler = system.has_event_handler(event_type);
                        log::debug!(
                            "[PublishEvent] system={} has_handler={}",
                            system.name(), has_handler
                        );
                        if has_handler {
                            let handled = system.dispatch_event(event_type, event.as_ref(), gx);
                            log::debug!("[PublishEvent] dispatched to system {}, handled={}", system.name(), handled);
                        }
                    }
                }

                InstanceCommand::SendRequest {
                    target,
                    request,
                    request_type,
                    response_tx,
                } => {
                    let result = self.handle_request(registry, gx, target, request, request_type).await;
                    let _ = response_tx.send(result);
                }
            }
        }

        Ok(())
    }

    /// Handle a request command.
    async fn handle_request(
        &self,
        registry: &Arc<RwLock<InstanceRegistry>>,
        gx: &GlobalContext,
        target: RequestTarget,
        request: Box<dyn Any + Send + Sync>,
        request_type: TypeId,
    ) -> Result<Box<dyn Any + Send + Sync>, RequestError> {
        // Find target instance and get the future while holding the lock briefly
        let future = {
            let reg = registry.read().unwrap();

            // Find target instance
            let instance_id = match target {
                RequestTarget::AppType(target_type_id) => {
                    let mut found_id = None;
                    for instance in reg.iter() {
                        if AnyAppInstance::type_id(instance) == target_type_id && !instance.is_sleeping() {
                            found_id = Some(instance.id());
                            break;
                        }
                    }
                    found_id.ok_or(RequestError::NoInstance)?
                }
                RequestTarget::Instance(id) => id,
            };

            let instance = reg.get(instance_id).ok_or(RequestError::InstanceNotFound)?;

            // Check if instance is sleeping (only for direct instance targeting)
            if instance.is_sleeping() {
                return Err(RequestError::InstanceSleeping(instance_id));
            }

            if !instance.has_request_handler(request_type) {
                return Err(RequestError::NoHandler);
            }

            // Create AppContext for target instance
            let cx = AppContext::new(instance_id, gx.clone(), instance.config().name);

            // Dispatch request - get the future
            instance.dispatch_request(request_type, request, &cx, gx)
        };

        match future {
            Some(fut) => Ok(fut.await),
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

// =============================================================================
// Text Input Sync
// =============================================================================

/// Sync text input values from element tree to TextInputState.
///
/// Only initializes or updates TextInputState when:
/// - The input doesn't exist yet (new element)
/// - The value changed externally (app set different value via State<String>)
///
/// This preserves cursor position when user is typing.
fn sync_text_inputs(element: &Element, text_inputs: &mut TextInputState) {
    // Check if this element is a text input
    if let Content::TextInput { value, .. } = &element.content {
        let should_init = match text_inputs.get_data(&element.id) {
            None => true,                        // New input
            Some(data) => data.text != *value,   // Value changed externally
        };

        if should_init {
            text_inputs.set(&element.id, value);
        }
    }

    // Recurse into children
    match &element.content {
        Content::Children(children) => {
            for child in children {
                sync_text_inputs(child, text_inputs);
            }
        }
        Content::Frames { children, .. } => {
            for child in children {
                sync_text_inputs(child, text_inputs);
            }
        }
        _ => {}
    }
}

// =============================================================================
// Scroll To Element
// =============================================================================

/// Scroll to bring an element into view (both horizontally and vertically).
/// Returns a ScrollChange if scrolling occurred.
fn scroll_to_element(
    root: &Element,
    layout: &LayoutResult,
    scroll: &mut ScrollState,
    target_id: &str,
) -> Option<tuidom::ScrollChange> {
    // Find scrollable ancestor
    let scrollable_id = find_scrollable_ancestor(root, target_id)?;

    // Get target rect and scrollable viewport
    let target_rect = layout.get(target_id)?;
    let viewport_rect = layout.get(&scrollable_id)?;
    let (viewport_width, viewport_height) = layout.viewport_size(&scrollable_id)?;
    let (content_width, content_height) = layout.content_size(&scrollable_id)?;

    let current = scroll.get(&scrollable_id);

    // Calculate target position relative to scrollable content (vertical)
    let target_top = target_rect.y.saturating_sub(viewport_rect.y) + current.y;
    let target_bottom = target_top + target_rect.height;

    // Calculate target position relative to scrollable content (horizontal)
    // target_rect.x is already relative to scroll content for horizontal scroll containers
    let target_left = target_rect.x;
    let target_right = target_left + target_rect.width;

    // Compute new vertical scroll offset to bring target into view
    let new_y = if target_top < current.y {
        // Target is above viewport - scroll up
        target_top
    } else if target_bottom > current.y + viewport_height {
        // Target is below viewport - scroll down
        target_bottom.saturating_sub(viewport_height)
    } else {
        // Already visible vertically
        current.y
    };

    // Compute new horizontal scroll offset to bring target into view
    let new_x = if target_left < current.x {
        // Target is left of viewport - scroll left
        target_left
    } else if target_right > current.x + viewport_width {
        // Target is right of viewport - scroll right
        target_right.saturating_sub(viewport_width)
    } else {
        // Already visible horizontally
        current.x
    };

    if new_x != current.x || new_y != current.y {
        scroll.set(&scrollable_id, new_x, new_y);
        Some(tuidom::ScrollChange {
            element_id: scrollable_id,
            offset_x: new_x,
            offset_y: new_y,
            content_width,
            content_height,
            viewport_width,
            viewport_height,
        })
    } else {
        None
    }
}
