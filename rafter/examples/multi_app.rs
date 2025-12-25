//! Multi-App Example
//!
//! Demonstrates switching between multiple apps in the rafter runtime:
//! - Spawning new app instances with `cx.spawn_and_focus()`
//! - Different BlurPolicy behaviors (Continue, Sleep, Close)
//! - Instance discovery with `cx.instances()`
//! - Pub/Sub events with `cx.publish()` and `#[event_handler]`
//! - Request/Response with `cx.request()` and `#[request_handler]`
//! - System keybinds with `#[system]` and `#[system_impl]`
//! - System overlays with `#[system_overlay]` and `#[system_overlay_impl]`
//! - Global data with `Runtime::data()` and `cx.data::<T>()`

use std::fs::File;
use std::sync::atomic::{AtomicU32, Ordering};

use log::{debug, info, LevelFilter};
use rafter::app::InstanceInfo;
use rafter::node::{Layout, Node};
use rafter::prelude::*;
use rafter::request::RequestError;
use rafter::styling::{Style, StyleColor};
use simplelog::{Config, WriteLogger};

// ============================================================================
// Global Data (shared across all apps)
// ============================================================================

/// Mock API client demonstrating global data pattern.
///
/// In a real app, this would be your HTTP client, database pool, etc.
/// Users are responsible for their own synchronization if mutation is needed.
pub struct MockApiClient {
    /// Base URL for the API
    pub base_url: String,
    /// Track number of requests made (using atomic for thread-safe mutation)
    request_count: AtomicU32,
}

impl MockApiClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            request_count: AtomicU32::new(0),
        }
    }

    /// Simulate an API request
    pub async fn get(&self, endpoint: &str) -> String {
        self.request_count.fetch_add(1, Ordering::SeqCst);
        // Simulate network delay
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        format!("Response from {}{}", self.base_url, endpoint)
    }

    /// Get the total number of requests made
    pub fn request_count(&self) -> u32 {
        self.request_count.load(Ordering::SeqCst)
    }
}

// ============================================================================
// System Keybinds (Global, highest priority)
// ============================================================================

/// Confirmation modal for quitting
#[modal]
struct QuitConfirmModal;

#[modal_impl]
impl QuitConfirmModal {
    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "y" | "enter" => confirm,
            "n" | "escape" => cancel,
        }
    }

    #[handler]
    async fn confirm(&self, mx: &ModalContext<bool>) {
        mx.close(true);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<bool>) {
        mx.close(false);
    }

    fn page(&self) -> Node {
        page! {
            column(padding: 2, gap: 1, bg: surface) {
                text(bold, fg: error) { "Quit Application?" }
                text { "" }
                text { "Are you sure you want to quit?" }
                text { "All unsaved data will be lost." }
                text { "" }
                row(gap: 2) {
                    button(id: "no", label: "[N]o", on_click: cancel)
                    button(id: "yes", label: "[Y]es", on_click: confirm)
                }
            }
        }
    }
}

// ============================================================================
// Taskbar Overlay (Bottom bar showing running apps)
// ============================================================================

/// A taskbar overlay that shows all running app instances.
/// Demonstrates the system overlay feature with clickable buttons.
#[system_overlay(position = Bottom, height = 1)]
struct Taskbar {
    /// Cached list of instances (updated via event handler)
    instances: Vec<InstanceInfo>,
}

#[system_overlay_impl]
impl Taskbar {
    /// Called once when the overlay is initialized, before the first render.
    fn on_init(&self, cx: &AppContext) {
        let instances = cx.instances();
        info!("[Taskbar] on_init: {} instances", instances.len());
        self.instances.set(instances);
    }

    /// Listen for app activation events to refresh the instance list
    #[event_handler]
    async fn on_app_activated(&self, event: AppActivated, cx: &AppContext) {
        let instances = cx.instances();
        info!("[Taskbar] on_app_activated({}): {} instances", event.app_name, instances.len());
        for inst in &instances {
            info!("[Taskbar]   - {} (focused={})", inst.app_name, inst.is_focused);
        }
        self.instances.set(instances);
    }

    /// Handler for clicking any taskbar button
    #[handler]
    async fn on_click(&self, cx: &AppContext) {
        // Get the widget ID that triggered this handler
        if let Some(widget_id) = cx.trigger_widget_id() {
            // Parse the index from the button ID (e.g., "taskbar-btn-2" -> 2)
            if let Some(index_str) = widget_id.strip_prefix("taskbar-btn-") {
                if let Ok(index) = index_str.parse::<usize>() {
                    let instances = self.instances.get();
                    if let Some(info) = instances.get(index) {
                        if !info.is_focused {
                            info!("[Taskbar] Focusing app: {} (from button {})", info.app_name, widget_id);
                            cx.focus_instance(info.id);
                        }
                    }
                }
            }
        }
    }

    fn view(&self) -> Node {
        let instances = self.instances.get();
        debug!("[Taskbar] view() called with {} instances", instances.len());

        if instances.is_empty() {
            return page! {
                row(bg: surface) {
                    text(fg: muted) { " Taskbar | No apps running " }
                }
            };
        }

        // Build buttons for each instance
        let buttons: Vec<Node> = instances
            .iter()
            .enumerate()
            .map(|(i, info)| {
                let marker = if info.is_focused { ">" } else { " " };
                let label = format!("{}[{}]", marker, info.app_name);
                let id = format!("taskbar-btn-{}", i);
                
                page! { button(id: id, label: label, on_click: on_click) }
            })
            .collect();

        // Build the row manually to avoid the `for` loop wrapping in a Column
        let mut children = vec![
            page! { text(fg: muted) { " Taskbar |" } }
        ];
        children.extend(buttons);
        
        Node::Row {
            children,
            style: Style::new().bg(StyleColor::Named("surface".to_string())),
            layout: Layout { gap: 1, ..Default::default() },
            id: None,
        }
    }
}

// ============================================================================
// System Keybinds (Global, highest priority)
// ============================================================================

/// Global system keybinds that work across all apps.
/// These have highest priority and are checked before any app keybinds.
#[system]
struct GlobalKeys;

#[system_impl]
impl GlobalKeys {
    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "Ctrl+q" => force_quit,
            "Ctrl+n" => next_instance,
            "Ctrl+p" => prev_instance,
        }
    }

    /// Force quit the entire application (with confirmation)
    #[handler]
    async fn force_quit(&self, cx: &AppContext) {
        info!("[System] Force quit triggered - showing confirmation");
        if cx.modal(QuitConfirmModal).await {
            info!("[System] Quit confirmed");
            cx.exit();
        } else {
            info!("[System] Quit cancelled");
        }
    }

    /// Cycle to the next app instance
    #[handler]
    fn next_instance(&self, cx: &AppContext) {
        let instances = cx.instances();
        if instances.len() <= 1 {
            info!("[System] Only one instance, can't cycle");
            return;
        }

        // Find the current focused instance and get the next one
        let current = instances.iter().position(|i| i.is_focused);
        if let Some(idx) = current {
            let next_idx = (idx + 1) % instances.len();
            let next_id = instances[next_idx].id;
            info!("[System] Cycling to next instance: {:?}", next_id);
            cx.focus_instance(next_id);
        }
    }

    /// Cycle to the previous app instance
    #[handler]
    fn prev_instance(&self, cx: &AppContext) {
        let instances = cx.instances();
        if instances.len() <= 1 {
            info!("[System] Only one instance, can't cycle");
            return;
        }

        // Find the current focused instance and get the previous one
        let current = instances.iter().position(|i| i.is_focused);
        if let Some(idx) = current {
            let prev_idx = if idx == 0 { instances.len() - 1 } else { idx - 1 };
            let prev_id = instances[prev_idx].id;
            info!("[System] Cycling to previous instance: {:?}", prev_id);
            cx.focus_instance(prev_id);
        }
    }
}

// ============================================================================
// Events and Requests
// ============================================================================

/// Event broadcast when an app becomes active (gains focus)
#[derive(Event, Clone, Debug)]
struct AppActivated {
    app_name: String,
}

/// Request to check if AppB is currently paused (sleeping)
#[derive(Request)]
#[response(bool)]
struct IsPaused;

// ============================================================================
// App A - BlurPolicy::Continue (default)
// Shows a list of all running instances
// ============================================================================

#[app(name = "App A", on_blur = Continue)]
struct AppA {
    /// Cached list of all instances (refreshed via handler)
    instances: Vec<InstanceInfo>,
    /// Last app that became active (received via event)
    last_activated: String,
    /// Status of AppB (from request)
    app_b_paused: Option<bool>,
    /// Counter value from App D (from request)
    app_d_counter: Option<i32>,
}

#[app_impl]
impl AppA {
    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "q" | "escape" => quit,
            "n" | "enter" => next_app,
            "d" => go_to_app_d,
            "r" => refresh,
            "p" => check_app_b_status,
            "c" => get_app_d_counter,
            "i" => increment_app_d_counter,
            "a" => test_api,
        }
    }

    async fn on_start(&self, cx: &AppContext) {
        // Publish that we're now active
        cx.publish(AppActivated {
            app_name: "App A".to_string(),
        });
    }

    /// Handler for AppActivated events (pub/sub)
    #[event_handler]
    async fn on_app_activated(&self, event: AppActivated, _cx: &AppContext) {
        info!("[App A] Received AppActivated event: {:?}", event);
        self.last_activated.set(event.app_name);
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        info!("[App A] Quitting");
        cx.exit();
    }

    /// Query AppB's status using request/response pattern
    #[handler]
    async fn check_app_b_status(&self, cx: &AppContext) {
        info!("[App A] Sending IsPaused request to App B");
        match cx.request::<AppB, IsPaused>(IsPaused).await {
            Ok(is_paused) => {
                info!("[App A] App B responded: paused = {}", is_paused);
                self.app_b_paused.set(Some(is_paused));
            }
            Err(RequestError::NoInstance) => {
                // NoInstance means no *awake* instance exists
                // App B could be sleeping (BlurPolicy::Sleep) or not spawned at all
                info!("[App A] App B has no awake instances (may be sleeping or not started)");
                self.app_b_paused.set(None);
            }
            Err(RequestError::InstanceSleeping(_)) => {
                // This only happens when targeting by instance ID
                info!("[App A] App B is sleeping - cannot respond to requests");
                self.app_b_paused.set(Some(true)); // Sleeping means paused
            }
            Err(e) => {
                info!("[App A] Request failed: {:?}", e);
                self.app_b_paused.set(None);
            }
        }
    }

    /// Go to App D (or spawn it)
    #[handler]
    async fn go_to_app_d(&self, cx: &AppContext) {
        if let Some(id) = cx.instance_of::<AppD>() {
            info!("[App A] Focusing existing App D");
            cx.focus_instance(id);
        } else {
            info!("[App A] Spawning new App D");
            let _ = cx.spawn_and_focus(AppD::default());
        }
    }

    /// Query App D's counter value
    #[handler]
    async fn get_app_d_counter(&self, cx: &AppContext) {
        info!("[App A] Sending GetCounter request to App D");
        match cx.request::<AppD, GetCounter>(GetCounter).await {
            Ok(value) => {
                info!("[App A] App D counter = {}", value);
                self.app_d_counter.set(Some(value));
            }
            Err(RequestError::NoInstance) => {
                info!("[App A] App D has no awake instances");
                self.app_d_counter.set(None);
            }
            Err(e) => {
                info!("[App A] GetCounter request failed: {:?}", e);
                self.app_d_counter.set(None);
            }
        }
    }

    /// Increment App D's counter remotely
    #[handler]
    async fn increment_app_d_counter(&self, cx: &AppContext) {
        info!("[App A] Sending IncrementCounter request to App D");
        match cx.request::<AppD, IncrementCounter>(IncrementCounter).await {
            Ok(new_value) => {
                info!("[App A] App D counter incremented to {}", new_value);
                self.app_d_counter.set(Some(new_value));
            }
            Err(RequestError::NoInstance) => {
                info!("[App A] App D has no awake instances");
                self.app_d_counter.set(None);
            }
            Err(e) => {
                info!("[App A] IncrementCounter request failed: {:?}", e);
            }
        }
    }

    /// Test the global API client (demonstrates cx.data::<T>())
    #[handler]
    async fn test_api(&self, cx: &AppContext) {
        info!("[App A] Testing global API client");

        // Access the global MockApiClient
        let client = cx.data::<MockApiClient>();

        // Make a mock request
        let response = client.get("/users").await;
        let count = client.request_count();

        info!("[App A] API response: {}", response);
        info!("[App A] Total requests made: {}", count);

        cx.toast(Toast::success(format!("API: {} (total: {})", response, count)));
    }

    #[handler]
    async fn refresh(&self, cx: &AppContext) {
        let instances = cx.instances();
        info!("[App A] Refreshing instance list: {} instances", instances.len());
        for inst in &instances {
            info!("  - {} ({}): {}", inst.app_name, &inst.id.to_string()[..8],
                  if inst.is_sleeping { "sleeping" } else if inst.is_focused { "focused" } else { "background" });
        }
        self.instances.set(instances);
    }

    #[handler]
    async fn next_app(&self, cx: &AppContext) {
        self.instances.set(cx.instances());

        // App B is a singleton - use the built-in helper method
        // This automatically gets existing instance or spawns a new one
        info!("[App A] Using AppB::get_or_spawn_and_focus() (singleton helper)");
        match AppB::get_or_spawn_and_focus(cx) {
            Ok(id) => info!("[App A] App B instance: {}", &id.to_string()[..8]),
            Err(e) => info!("[App A] Failed: {}", e),
        }
    }

    fn page(&self) -> Node {
        let instances = self.instances.get();
        let instance_count = instances.len();
        let separator = "─".repeat(50);
        let last_activated = self.last_activated.get();
        let app_b_status = self.app_b_paused.get();
        let app_d_counter = self.app_d_counter.get();

        // Build instance display strings with truncated IDs
        let instance_lines: Vec<String> = instances
            .iter()
            .map(|info| {
                let status = if info.is_focused {
                    "← focused"
                } else if info.is_sleeping {
                    "(sleeping)"
                } else {
                    "(background)"
                };
                let short_id = &info.id.to_string()[..8];
                format!("• [{}] {} - {} {}", short_id, info.app_name, info.title, status)
            })
            .collect();

        // Format AppB status
        let app_b_status_str = match app_b_status {
            Some(true) => "paused/sleeping".to_string(),
            Some(false) => "running (responded to request)".to_string(),
            None => "no awake instance".to_string(),
        };

        // Format AppD counter
        let app_d_counter_str = match app_d_counter {
            Some(v) => v.to_string(),
            None => "unknown".to_string(),
        };

        page! {
            column(padding: 2, gap: 1) {
                text(bold, fg: error) { "═══ APP A ═══" }
                text(fg: muted) { "BlurPolicy: Continue (keeps running in background)" }
                text { "" }

                // Pub/Sub status
                text(bold) { "Pub/Sub Events:" }
                text { format!("Last activated app: {}", if last_activated.is_empty() { "(none)" } else { &last_activated }) }
                text { "" }

                // Request/Response status
                text(bold) { "Request/Response:" }
                text { format!("App B status: {}", app_b_status_str) }
                text { format!("App D counter: {}", app_d_counter_str) }
                text { "" }

                // Instance list
                text(bold) { format!("Running Instances ({}):", instance_count) }
                text { separator.clone() }
                for line in instance_lines {
                    text { line }
                }
                text { separator }
                text { "" }

                text(fg: muted) { "[n] App B  [d] App D  [r] Refresh  [a] API Test  [q] Quit" }
                text(fg: muted) { "[c] Get D counter  [i] Increment D counter" }
                text { "" }
                row(gap: 2) {
                    button(id: "next", label: "→ App B", on_click: next_app)
                    button(id: "appd", label: "→ App D", on_click: go_to_app_d)
                    button(id: "refresh", label: "↻ Refresh", on_click: refresh)
                    button(id: "api", label: "API Test", on_click: test_api)
                }
                row(gap: 2) {
                    button(id: "getc", label: "Get D Counter", on_click: get_app_d_counter)
                    button(id: "incc", label: "Inc D Counter", on_click: increment_app_d_counter)
                }
            }
        }
    }
}

// ============================================================================
// App B - BlurPolicy::Sleep + Singleton
// Pauses when losing focus, resumes when focused again
// Only one instance can exist (uses built-in get_or_spawn_and_focus)
// ============================================================================

#[app(name = "App B", on_blur = Sleep, singleton)]
struct AppB {}

#[app_impl]
impl AppB {
    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "q" | "escape" => quit,
            "n" | "enter" => next_app,
        }
    }

    #[allow(dead_code)]
    async fn on_foreground(&self, cx: &AppContext) {
        // Publish event when we become active
        info!("[App B] Publishing AppActivated event");
        cx.publish(AppActivated {
            app_name: "App B".to_string(),
        });
    }

    /// Handle IsPaused requests - App B is never paused when it can respond
    /// (if it's sleeping, the request won't reach this handler)
    #[request_handler]
    async fn handle_is_paused(&self, _request: IsPaused, _cx: &AppContext) -> bool {
        info!("[App B] Received IsPaused request, responding: false");
        false // We're running if we can respond
    }

    /// Listen for activation events from other apps
    #[event_handler]
    async fn on_app_activated(&self, event: AppActivated, _cx: &AppContext) {
        info!("[App B] Received AppActivated event: {:?}", event);
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        info!("[App B] Quitting");
        cx.exit();
    }

    #[handler]
    async fn next_app(&self, cx: &AppContext) {
        // Check if an App C instance already exists
        if let Some(id) = cx.instance_of::<AppC>() {
            info!("[App B] Focusing existing App C instance: {}", &id.to_string()[..8]);
            cx.focus_instance(id);
        } else {
            info!("[App B] No App C exists, spawning new instance");
            let result = cx.spawn_and_focus(AppC::default());
            match result {
                Ok(id) => info!("[App B] Spawned App C with id: {}", &id.to_string()[..8]),
                Err(e) => info!("[App B] Failed to spawn App C: {}", e),
            }
        }
    }

    fn page(&self) -> Node {
        page! {
            column(padding: 2, gap: 1) {
                text(bold, fg: success) { "═══ APP B (SINGLETON) ═══" }
                text(fg: muted) { "BlurPolicy: Sleep + Singleton (max 1 instance)" }
                text { "" }
                text { "This app is a singleton - only one instance can exist." }
                text { "Uses AppB::get_or_spawn_and_focus() helper." }
                text { "" }
                text { "When you switch away, this instance sleeps." }
                text { "When you come back, the SAME instance resumes!" }
                text { "" }
                text(fg: muted) { "[n] Go to App C  [q] Quit" }
                text { "" }
                button(id: "next", label: "→ Go to App C", on_click: next_app)
            }
        }
    }
}

// ============================================================================
// App C - BlurPolicy::Close
// Automatically closes when losing focus
// ============================================================================

#[app(name = "App C", on_blur = Close)]
struct AppC {}

#[app_impl]
impl AppC {
    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "q" | "escape" => quit,
            "n" | "enter" => next_app,
        }
    }

    #[allow(dead_code)]
    async fn on_foreground(&self, cx: &AppContext) {
        // Publish event when we become active
        info!("[App C] Publishing AppActivated event");
        cx.publish(AppActivated {
            app_name: "App C".to_string(),
        });
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        info!("[App C] Quitting");
        cx.exit();
    }

    #[handler]
    async fn next_app(&self, cx: &AppContext) {
        info!("[App C] Switching to App A (this App C will close due to BlurPolicy::Close)");

        // Check if an App A instance already exists
        if let Some(id) = cx.instance_of::<AppA>() {
            info!("[App C] Focusing existing App A instance: {}", &id.to_string()[..8]);
            cx.focus_instance(id);
        } else {
            info!("[App C] No App A exists, spawning new instance");
            let result = cx.spawn_and_focus(AppA::default());
            match result {
                Ok(id) => info!("[App C] Spawned App A with id: {}", &id.to_string()[..8]),
                Err(e) => info!("[App C] Failed to spawn App A: {}", e),
            }
        }
    }

    fn page(&self) -> Node {
        page! {
            column(padding: 2, gap: 1) {
                text(bold, fg: info) { "═══ APP C ═══" }
                text(fg: muted) { "BlurPolicy: Close (closes when losing focus)" }
                text { "" }
                text { "This app will automatically close when you" }
                text { "switch to another app. It won't appear in" }
                text { "the instance list after switching!" }
                text { "" }
                text(fg: muted) { "[n] Go to App A  [q] Quit" }
                text { "" }
                button(id: "next", label: "→ Go to App A", on_click: next_app)
            }
        }
    }
}

// ============================================================================
// App D - BlurPolicy::Continue + Request Handler
// Stays running in background and can respond to requests
// ============================================================================

#[app(name = "App D", on_blur = Continue)]
struct AppD {
    /// Counter to demonstrate state
    counter: i32,
}

/// Request to get the current counter value from App D
#[derive(Request)]
#[response(i32)]
struct GetCounter;

/// Request to increment the counter and return new value
#[derive(Request)]
#[response(i32)]
struct IncrementCounter;

#[app_impl]
impl AppD {
    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "q" | "escape" => quit,
            "n" | "enter" => go_to_app_a,
            "+" | "=" => increment,
        }
    }

    #[allow(dead_code)]
    async fn on_foreground(&self, cx: &AppContext) {
        info!("[App D] Publishing AppActivated event");
        cx.publish(AppActivated {
            app_name: "App D".to_string(),
        });
    }

    /// Handle GetCounter requests
    #[request_handler]
    async fn handle_get_counter(&self, _request: GetCounter, _cx: &AppContext) -> i32 {
        let value = self.counter.get();
        info!("[App D] GetCounter request received, returning {}", value);
        value
    }

    /// Handle IncrementCounter requests
    #[request_handler]
    async fn handle_increment(&self, _request: IncrementCounter, _cx: &AppContext) -> i32 {
        self.counter.update(|v| *v += 1);
        let new_value = self.counter.get();
        info!("[App D] IncrementCounter request received, new value: {}", new_value);
        new_value
    }

    /// Listen for activation events
    #[event_handler]
    async fn on_app_activated(&self, event: AppActivated, _cx: &AppContext) {
        info!("[App D] Received AppActivated event: {:?}", event);
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        info!("[App D] Quitting");
        cx.exit();
    }

    #[handler]
    async fn increment(&self) {
        self.counter.update(|v| *v += 1);
        let new_val = self.counter.get();
        info!("[App D] Counter incremented to {}", new_val);
    }

    #[handler]
    async fn go_to_app_a(&self, cx: &AppContext) {
        if let Some(id) = cx.instance_of::<AppA>() {
            info!("[App D] Focusing existing App A");
            cx.focus_instance(id);
        } else {
            info!("[App D] Spawning new App A");
            let _ = cx.spawn_and_focus(AppA::default());
        }
    }

    fn page(&self) -> Node {
        let counter = self.counter.get();
        page! {
            column(padding: 2, gap: 1) {
                text(bold, fg: warning) { "═══ APP D ═══" }
                text(fg: muted) { "BlurPolicy: Continue (stays running, can respond to requests)" }
                text { "" }
                text { "This app stays awake in the background!" }
                text { "Other apps can query its counter via requests." }
                text { "" }
                text(bold) { format!("Counter: {}", counter) }
                text { "" }
                text(fg: muted) { "[+] Increment  [n] Go to App A  [q] Quit" }
                text { "" }
                row(gap: 2) {
                    button(id: "inc", label: "+ Increment", on_click: increment)
                    button(id: "next", label: "→ App A", on_click: go_to_app_a)
                }
            }
        }
    }
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() {
    // Initialize file logging
    if let Ok(log_file) = File::create("multi_app.log") {
        let _ = WriteLogger::init(LevelFilter::Debug, Config::default(), log_file);
    }

    // Create shared API client
    let api_client = MockApiClient::new("https://api.example.com");

    if let Err(e) = rafter::Runtime::new()
        .data(api_client) // Register global data
        .initial::<AppA>()
        .run()
        .await
    {
        eprintln!("Error: {}", e);
    }
}
