//! Multi-App Example
//!
//! Demonstrates switching between multiple apps in the rafter runtime:
//! - Spawning new app instances with `cx.spawn_and_focus()`
//! - Different BlurPolicy behaviors (Continue, Sleep, Close)
//! - Instance discovery with `cx.instances()`
//! - Pub/Sub events with `cx.publish()` and `#[event_handler]`
//! - Request/Response with `cx.request()` and `#[request_handler]`

use std::fs::File;

use log::{info, LevelFilter};
use rafter::app::InstanceInfo;
use rafter::prelude::*;
use rafter::request::{Request, RequestError};
use rafter::event::Event;
use simplelog::{Config, WriteLogger};

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
}

#[app_impl]
impl AppA {
    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "q" | "escape" => quit,
            "n" | "enter" => next_app,
            "r" => refresh,
            "p" => check_app_b_status,
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
                info!("[App A] App B has no instances running");
                self.app_b_paused.set(None);
            }
            Err(RequestError::InstanceSleeping(_)) => {
                info!("[App A] App B is sleeping - cannot respond to requests");
                self.app_b_paused.set(Some(true)); // Sleeping means paused
            }
            Err(e) => {
                info!("[App A] Request failed: {:?}", e);
                self.app_b_paused.set(None);
            }
        }
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
            Some(false) => "running".to_string(),
            None => "unknown (no instance?)".to_string(),
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
                text { "" }

                // Instance list
                text(bold) { format!("Running Instances ({}):", instance_count) }
                text { separator.clone() }
                for line in instance_lines {
                    text { line }
                }
                text { separator }
                text { "" }

                text(fg: muted) { "[n] App B  [r] Refresh  [p] Check App B  [q] Quit" }
                text { "" }
                row(gap: 2) {
                    button(id: "next", label: "→ App B", on_click: next_app)
                    button(id: "refresh", label: "↻ Refresh", on_click: refresh)
                    button(id: "check", label: "? Check B", on_click: check_app_b_status)
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
// Main
// ============================================================================

#[tokio::main]
async fn main() {
    // Initialize file logging
    if let Ok(log_file) = File::create("multi_app.log") {
        let _ = WriteLogger::init(LevelFilter::Debug, Config::default(), log_file);
    }

    if let Err(e) = rafter::Runtime::new().initial::<AppA>().run().await {
        eprintln!("Error: {}", e);
    }
}
