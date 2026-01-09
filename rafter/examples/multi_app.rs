//! Multi-App Example
//!
//! Demonstrates switching between multiple apps in the rafter runtime:
//! - Spawning new app instances with `gx.spawn_and_focus()`
//! - Different BlurPolicy behaviors (Continue, Sleep, Close)
//! - Instance discovery with `gx.instances()`
//! - Pub/Sub events with `gx.publish()` and `#[event_handler]`
//! - Request/Response with `gx.request()` and `#[request_handler]`
//! - System keybinds with `#[system]` and `#[system_impl]`
//! - Global data with `Runtime::data()` and `gx.data::<T>()`

use std::fs::File;
use std::sync::atomic::{AtomicU32, Ordering};

use log::{debug, info, LevelFilter};
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Text};
use rafter::{Event, FocusChanged, InstanceClosed, InstanceId, InstanceInfo, InstanceSpawned, Overlay, Request, RequestError};
use simplelog::{Config, WriteLogger};

// ============================================================================
// Global Data (shared across all apps)
// ============================================================================

/// Mock API client demonstrating global data pattern.
pub struct MockApiClient {
    pub base_url: String,
    request_count: AtomicU32,
}

impl MockApiClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            request_count: AtomicU32::new(0),
        }
    }

    pub async fn get(&self, endpoint: &str) -> String {
        self.request_count.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        format!("Response from {}{}", self.base_url, endpoint)
    }

    pub fn request_count(&self) -> u32 {
        self.request_count.load(Ordering::SeqCst)
    }
}

// ============================================================================
// Events and Requests
// ============================================================================

/// Event broadcast when an app becomes active
#[derive(Clone, Debug)]
struct AppActivated {
    app_name: String,
}

impl Event for AppActivated {}

/// Request to check if AppB is paused
#[derive(Request)]
#[response(bool)]
struct IsPaused;

/// Request to get counter value from App D
#[derive(Request)]
#[response(i32)]
struct GetCounter;

/// Request to increment counter
#[derive(Request)]
#[response(i32)]
struct IncrementCounter;

// ============================================================================
// Quit Confirmation Modal
// ============================================================================

#[modal]
struct QuitConfirmModal;

#[modal_impl]
impl QuitConfirmModal {
    #[keybinds]
    fn keys() {
        bind("y", confirm);
        bind("enter", confirm);
        bind("n", cancel);
        bind("escape", cancel);
    }

    #[handler]
    async fn confirm(&self, mx: &ModalContext<bool>) {
        mx.close(true);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<bool>) {
        mx.close(false);
    }

    fn element(&self) -> Element {
        page! {
            column (padding: 2, gap: 1) style (bg: surface) {
                text (content: "Quit Application?") style (bold, fg: error)
                text (content: "Are you sure you want to quit?")
                row (gap: 2) {
                    button (label: "[N]o", id: "no") on_activate: cancel()
                    button (label: "[Y]es", id: "yes") on_activate: confirm()
                }
            }
        }
    }
}

// ============================================================================
// Taskbar Overlay
// ============================================================================

#[system(position = Bottom, height = 1)]
struct Taskbar {
    instances: Vec<InstanceInfo>,
}

#[system_impl]
impl Taskbar {
    fn on_init(&self) {
        info!("[Taskbar] on_init");
    }

    #[event_handler]
    async fn on_instance_spawned(&self, event: InstanceSpawned, gx: &GlobalContext) {
        info!("[Taskbar] on_instance_spawned({})", event.name);
        self.instances.set(gx.instances());
    }

    #[event_handler]
    async fn on_instance_closed(&self, event: InstanceClosed, gx: &GlobalContext) {
        info!("[Taskbar] on_instance_closed({})", event.name);
        self.instances.set(gx.instances());
    }

    #[event_handler]
    async fn on_focus_changed(&self, _event: FocusChanged, gx: &GlobalContext) {
        info!("[Taskbar] on_focus_changed");
        self.instances.set(gx.instances());
    }

    #[handler]
    async fn on_taskbar_click(&self, id: InstanceId, gx: &GlobalContext) {
        gx.focus_instance(id);
    }

    fn overlay(&self) -> Option<Overlay> {
        let instances = self.instances.get();
        debug!("[Taskbar] overlay() with {} instances", instances.len());

        let buttons: Vec<_> = instances
            .iter()
            .map(|info| {
                let label = if info.is_focused {
                    format!(">{}", info.name)
                } else {
                    info.name.to_string()
                };
                let btn_id = format!("taskbar-{}", info.id);
                let instance_id = info.id;
                (label, btn_id, instance_id)
            })
            .collect();

        let element = page! {
            row (gap: 1, width: fill, overflow: hidden) style (bg: surface) {
                text (content: " Taskbar |") style (fg: muted)
                for (label, btn_id, instance_id) in buttons {
                    button (label: {label}, id: {btn_id}) on_activate: on_taskbar_click(instance_id)
                }
            }
        };

        Some(Overlay::bottom(1, element))
    }
}

// ============================================================================
// System Keybinds
// ============================================================================

#[system]
struct GlobalKeys;

#[system_impl]
impl GlobalKeys {
    #[keybinds]
    fn keys() {
        bind("ctrl+q", force_quit);
        bind("ctrl+n", next_instance);
        bind("ctrl+p", prev_instance);
    }

    #[handler]
    async fn force_quit(&self, gx: &GlobalContext) {
        info!("[System] Force quit triggered");
        if gx.modal(QuitConfirmModal::default()).await {
            info!("[System] Quit confirmed");
            gx.shutdown();
        }
    }

    #[handler]
    async fn next_instance(&self, gx: &GlobalContext) {
        let instances = gx.instances();
        if instances.len() <= 1 {
            return;
        }
        if let Some(idx) = instances.iter().position(|i| i.is_focused) {
            let next_idx = (idx + 1) % instances.len();
            gx.focus_instance(instances[next_idx].id);
        }
    }

    #[handler]
    async fn prev_instance(&self, gx: &GlobalContext) {
        let instances = gx.instances();
        if instances.len() <= 1 {
            return;
        }
        if let Some(idx) = instances.iter().position(|i| i.is_focused) {
            let prev_idx = if idx == 0 { instances.len() - 1 } else { idx - 1 };
            gx.focus_instance(instances[prev_idx].id);
        }
    }
}

// ============================================================================
// App A - BlurPolicy::Continue
// ============================================================================

#[app(name = "App A", on_blur = Continue)]
struct AppA {
    instances: Vec<InstanceInfo>,
    last_activated: String,
    app_b_paused: Option<bool>,
    app_d_counter: Option<i32>,
}

#[app_impl]
impl AppA {
    #[keybinds]
    fn keys() {
        bind("q", quit);
        bind("escape", quit);
        bind("n", next_app);
        bind("enter", next_app);
        bind("d", go_to_app_d);
        bind("r", refresh);
        bind("p", check_app_b_status);
        bind("c", get_app_d_counter);
        bind("i", increment_app_d_counter);
        bind("a", test_api);
        bind("e", publish_event);
    }

    #[event_handler]
    async fn on_app_activated(&self, event: AppActivated) {
        info!("[App A] Received AppActivated: {:?}", event);
        self.last_activated.set(event.app_name);
    }

    #[handler]
    async fn publish_event(&self, gx: &GlobalContext) {
        info!("[App A] Publishing AppActivated event");
        gx.publish(AppActivated {
            app_name: "App A".to_string(),
        });
    }

    #[handler]
    async fn quit(&self, gx: &GlobalContext) {
        gx.shutdown();
    }

    #[handler]
    async fn check_app_b_status(&self, gx: &GlobalContext) {
        info!("[App A] Checking App B status");
        match gx.request::<AppB, IsPaused>(IsPaused).await {
            Ok(is_paused) => {
                info!("[App A] App B paused = {}", is_paused);
                self.app_b_paused.set(Some(is_paused));
            }
            Err(RequestError::NoInstance) => {
                info!("[App A] App B has no awake instances");
                self.app_b_paused.set(None);
            }
            Err(e) => {
                info!("[App A] Request failed: {:?}", e);
                self.app_b_paused.set(None);
            }
        }
    }

    #[handler]
    async fn go_to_app_d(&self, gx: &GlobalContext) {
        if let Some(id) = gx.instance_of::<AppD>() {
            gx.focus_instance(id);
        } else {
            let _ = gx.spawn_and_focus(AppD::default());
        }
    }

    #[handler]
    async fn get_app_d_counter(&self, gx: &GlobalContext) {
        match gx.request::<AppD, GetCounter>(GetCounter).await {
            Ok(value) => self.app_d_counter.set(Some(value)),
            Err(_) => self.app_d_counter.set(None),
        }
    }

    #[handler]
    async fn increment_app_d_counter(&self, gx: &GlobalContext) {
        match gx.request::<AppD, IncrementCounter>(IncrementCounter).await {
            Ok(new_value) => self.app_d_counter.set(Some(new_value)),
            Err(_) => self.app_d_counter.set(None),
        }
    }

    #[handler]
    async fn test_api(&self, gx: &GlobalContext) {
        let client = gx.data::<MockApiClient>();
        let response = client.get("/users").await;
        let count = client.request_count();
        gx.toast(Toast::success(format!("API: {} (total: {})", response, count)));
    }

    #[handler]
    async fn refresh(&self, gx: &GlobalContext) {
        self.instances.set(gx.instances());
    }

    #[handler]
    async fn next_app(&self, gx: &GlobalContext) {
        if let Some(id) = gx.instance_of::<AppB>() {
            gx.focus_instance(id);
        } else {
            let _ = gx.spawn_and_focus(AppB::default());
        }
    }

    fn element(&self) -> Element {
        let instances = self.instances.get();
        let last_activated = self.last_activated.get();
        let app_b_status = self.app_b_paused.get();
        let app_d_counter = self.app_d_counter.get();

        let instances_text = if instances.is_empty() {
            "(press [r] to refresh)".to_string()
        } else {
            instances
                .iter()
                .map(|i| {
                    let status = if i.is_focused { "focused" } else if i.is_sleeping { "sleeping" } else { "bg" };
                    format!("  {} - {}", i.name, status)
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        let last_str = if last_activated.is_empty() { "(none)".to_string() } else { last_activated };
        let b_status = match app_b_status { Some(true) => "sleeping", Some(false) => "awake", None => "unknown" };
        let d_counter = app_d_counter.map(|v| v.to_string()).unwrap_or_else(|| "unknown".to_string());

        page! {
            column (padding: 2, gap: 1) style (bg: background) {
                text (content: "=== APP A ===") style (bold, fg: error)
                text (content: "BlurPolicy: Continue") style (fg: muted)
                text (content: {format!("Last event: {}", last_str)})
                text (content: {format!("App B: {}", b_status)})
                text (content: {format!("App D counter: {}", d_counter)})
                text (content: {format!("Instances:\n{}", instances_text)})
                text (content: "[n]AppB [d]AppD [r]Refresh [e]Event [p]CheckB [c/i]D counter [a]API [q]Quit") style (fg: muted)
            }
        }
    }
}

// ============================================================================
// App B - BlurPolicy::Sleep + Singleton
// ============================================================================

#[app(name = "App B", on_blur = Sleep, singleton)]
struct AppB {}

#[app_impl]
impl AppB {
    #[keybinds]
    fn keys() {
        bind("q", quit);
        bind("escape", quit);
        bind("n", next_app);
        bind("enter", next_app);
    }

    #[request_handler]
    async fn handle_is_paused(&self, _request: IsPaused) -> bool {
        info!("[App B] IsPaused request, responding: false");
        false
    }

    #[event_handler]
    async fn on_app_activated(&self, event: AppActivated) {
        info!("[App B] Received AppActivated: {:?}", event);
    }

    #[handler]
    async fn quit(&self, gx: &GlobalContext) {
        gx.shutdown();
    }

    #[handler]
    async fn next_app(&self, gx: &GlobalContext) {
        if let Some(id) = gx.instance_of::<AppC>() {
            gx.focus_instance(id);
        } else {
            let _ = gx.spawn_and_focus(AppC::default());
        }
    }

    fn element(&self) -> Element {
        page! {
            column (padding: 2, gap: 1) style (bg: background) {
                text (content: "=== APP B (SINGLETON) ===") style (bold, fg: success)
                text (content: "BlurPolicy: Sleep") style (fg: muted)
                text (content: "This app sleeps when unfocused.")
                text (content: "Same instance resumes when you return!")
                text (content: "[n] Go to App C  [q] Quit") style (fg: muted)
            }
        }
    }
}

// ============================================================================
// App C - BlurPolicy::Close
// ============================================================================

#[app(name = "App C", on_blur = Close)]
struct AppC {}

#[app_impl]
impl AppC {
    #[keybinds]
    fn keys() {
        bind("q", quit);
        bind("escape", quit);
        bind("n", next_app);
        bind("enter", next_app);
    }

    #[handler]
    async fn quit(&self, gx: &GlobalContext) {
        gx.shutdown();
    }

    #[handler]
    async fn next_app(&self, gx: &GlobalContext) {
        info!("[App C] Switching to App A (this will close)");
        if let Some(id) = gx.instance_of::<AppA>() {
            gx.focus_instance(id);
        } else {
            let _ = gx.spawn_and_focus(AppA::default());
        }
    }

    fn element(&self) -> Element {
        page! {
            column (padding: 2, gap: 1) style (bg: background) {
                text (content: "=== APP C ===") style (bold, fg: info)
                text (content: "BlurPolicy: Close") style (fg: muted)
                text (content: "This app closes when you switch away!")
                text (content: "[n] Go to App A  [q] Quit") style (fg: muted)
            }
        }
    }
}

// ============================================================================
// App D - BlurPolicy::Continue + Request Handler
// ============================================================================

#[app(name = "App D", on_blur = Continue)]
struct AppD {
    counter: i32,
}

#[app_impl]
impl AppD {
    #[keybinds]
    fn keys() {
        bind("q", quit);
        bind("escape", quit);
        bind("n", go_to_app_a);
        bind("enter", go_to_app_a);
        bind("+", increment);
        bind("=", increment);
    }

    #[request_handler]
    async fn handle_get_counter(&self, _request: GetCounter) -> i32 {
        self.counter.get()
    }

    #[request_handler]
    async fn handle_increment(&self, _request: IncrementCounter) -> i32 {
        self.counter.update(|v| *v += 1);
        self.counter.get()
    }

    #[event_handler]
    async fn on_app_activated(&self, event: AppActivated) {
        info!("[App D] Received AppActivated: {:?}", event);
    }

    #[handler]
    async fn quit(&self, gx: &GlobalContext) {
        gx.shutdown();
    }

    #[handler]
    async fn increment(&self) {
        self.counter.update(|v| *v += 1);
    }

    #[handler]
    async fn go_to_app_a(&self, gx: &GlobalContext) {
        if let Some(id) = gx.instance_of::<AppA>() {
            gx.focus_instance(id);
        } else {
            let _ = gx.spawn_and_focus(AppA::default());
        }
    }

    fn element(&self) -> Element {
        let counter_str = self.counter.get().to_string();

        page! {
            column (padding: 2, gap: 1) style (bg: background) {
                text (content: "=== APP D ===") style (bold, fg: warning)
                text (content: "BlurPolicy: Continue (responds to requests)") style (fg: muted)
                text (content: {format!("Counter: {}", counter_str)}) style (bold)
                text (content: "[+] Increment  [n] Go to App A  [q] Quit") style (fg: muted)
            }
        }
    }
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() {
    if let Ok(log_file) = File::create("multi_app.log") {
        let _ = WriteLogger::init(LevelFilter::Debug, Config::default(), log_file);
    }

    let api_client = MockApiClient::new("https://api.example.com");

    if let Err(e) = Runtime::new()
        .expect("Failed to create runtime")
        .data(api_client)
        .system(Taskbar::default())
        .system(GlobalKeys::default())
        .run(AppA::default())
        .await
    {
        eprintln!("Error: {}", e);
    }
}
