//! Modal Animation Debug Example
//!
//! Tests whether frame animations work correctly in modals.
//! Expected behavior: both the app spinner and modal spinner should animate.
//! Bug: modal spinners don't animate while app spinners do.

use std::fs::File;
use std::time::Duration;

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Text;
use simplelog::{Config, LevelFilter, WriteLogger};

// ============================================================================
// Simple spinner helper
// ============================================================================

fn braille_spinner(id: &str) -> Element {
    Element::frames(
        vec![
            Element::text("⣷"),
            Element::text("⣯"),
            Element::text("⣟"),
            Element::text("⡿"),
            Element::text("⢿"),
            Element::text("⣻"),
            Element::text("⣽"),
            Element::text("⣾"),
        ],
        Duration::from_millis(80),
    )
    .id(id)
}

// ============================================================================
// Loading Modal with spinner (no on_start)
// ============================================================================

#[modal(default, size = Sm)]
struct LoadingModal;

#[modal_impl]
impl LoadingModal {
    fn default_result(&self) -> () {
        ()
    }

    #[keybinds]
    fn keys() {
        bind("escape", "enter", close);
    }

    #[handler]
    async fn close(&self, mx: &ModalContext<()>) {
        mx.close(());
    }

    fn element(&self) -> Element {
        page! {
            column (padding: 2, gap: 1) style (bg: surface) {
                text (content: "Modal WITHOUT on_start") style (bold, fg: primary)
                row (gap: 2) {
                    {braille_spinner("modal_spinner_basic")}
                    text (content: "Loading in modal...") style (fg: muted)
                }
                text (content: "Press ESC to close") style (dim, fg: muted)
            }
        }
    }
}

// ============================================================================
// Loading Modal with on_start that awaits
// ============================================================================

#[modal(default, size = Sm)]
struct LoadingModalWithOnStart {
    #[state]
    status: String,
}

#[modal_impl]
impl LoadingModalWithOnStart {
    fn default_result(&self) -> () {
        ()
    }

    #[on_start]
    async fn on_start(&self) {
        log::info!("[LoadingModalWithOnStart] on_start called, about to sleep...");
        self.status.set("Starting...".to_string());

        // Simulate async work in on_start
        tokio::time::sleep(Duration::from_secs(2)).await;

        log::info!("[LoadingModalWithOnStart] on_start sleep complete");
        self.status.set("Ready!".to_string());
    }

    #[keybinds]
    fn keys() {
        bind("escape", "enter", close);
    }

    #[handler]
    async fn close(&self, mx: &ModalContext<()>) {
        mx.close(());
    }

    fn element(&self) -> Element {
        let status = self.status.get();
        page! {
            column (padding: 2, gap: 1) style (bg: surface) {
                text (content: "Modal WITH on_start await") style (bold, fg: primary)
                row (gap: 2) {
                    {braille_spinner("modal_spinner_onstart")}
                    text (content: "Loading in modal...") style (fg: muted)
                }
                row (gap: 1) {
                    text (content: "Status:") style (fg: muted)
                    text (content: {status}) style (fg: warning)
                }
                text (content: "Press ESC to close") style (dim, fg: muted)
            }
        }
    }
}

// ============================================================================
// Main App with spinner
// ============================================================================

#[app(default)]
struct AnimationTestApp {}

#[app_impl]
impl AnimationTestApp {
    #[keybinds]
    fn keys() {
        bind("m", open_modal_basic);
        bind("o", open_modal_onstart);
        bind("q", "escape", quit);
    }

    #[handler]
    async fn open_modal_basic(&self, cx: &AppContext) {
        log::info!("Opening basic modal (no on_start)...");
        cx.modal(LoadingModal::default()).await;
        log::info!("Basic modal closed.");
    }

    #[handler]
    async fn open_modal_onstart(&self, cx: &AppContext) {
        log::info!("Opening modal with on_start await...");
        cx.modal(LoadingModalWithOnStart::default()).await;
        log::info!("Modal with on_start closed.");
    }

    #[handler]
    async fn quit(&self, gx: &GlobalContext) {
        gx.shutdown();
    }

    fn element(&self) -> Element {
        page! {
            column (padding: 2, gap: 2) style (bg: background) {
                text (content: "Modal Animation Test") style (bold, fg: primary)
                text (content: "This app tests if frame animations work in modals.") style (fg: muted)

                column (gap: 1) style (bg: surface, padding: 1) {
                    text (content: "App Spinner (should always animate):") style (fg: secondary)
                    row (gap: 2) {
                        {braille_spinner("app_spinner")}
                        text (content: "Background loading...") style (fg: muted)
                    }
                }

                text (content: "") // spacer

                column (gap: 1) {
                    text (content: "Instructions:") style (bold, fg: interact)
                    text (content: "1. Watch the spinner above - it should animate") style (fg: muted)
                    text (content: "2. Press 'm' for basic modal (no on_start)") style (fg: muted)
                    text (content: "3. Press 'o' for modal with on_start await") style (fg: muted)
                    text (content: "4. Compare: does spinner animate in both?") style (fg: muted)
                    text (content: "5. Press ESC to close modals") style (fg: muted)
                }

                text (content: "m basic modal  o on_start modal  q quit") style (dim, fg: muted)
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Set up file logging
    let log_file = File::create("modal_animation.log").expect("Failed to create log file");
    WriteLogger::init(LevelFilter::Debug, Config::default(), log_file)
        .expect("Failed to initialize logger");

    log::info!("Starting modal animation test...");

    if let Err(e) = Runtime::new()
        .expect("Failed to create runtime")
        .run(AnimationTestApp::default())
        .await
    {
        eprintln!("Error: {}", e);
    }
}
