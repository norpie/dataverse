//! Scheduled Toast Example
//!
//! Demonstrates the scheduled jobs feature by showing a toast notification
//! every 5 seconds with an incrementing counter.
//!
//! Features demonstrated:
//! - `gx.schedule_every()` for recurring jobs
//! - `gx.cancel_job()` to stop a scheduled job
//! - Toast notifications triggered by scheduled handlers

use std::fs::File;
use std::sync::Arc;
use std::time::Duration;

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Text;
use rafter::Handler;
use simplelog::{Config, LevelFilter, WriteLogger};

// ============================================================================
// Scheduled Toast App
// ============================================================================

#[app(default)]
struct ScheduledToastApp {
    /// The counter that increments every 5 seconds.
    counter: i32,
    /// Whether the scheduled job is running.
    running: bool,
    /// The job ID for the scheduled toast (stored to allow cancellation).
    job_id: Option<JobId>,
}

#[app_impl]
impl ScheduledToastApp {
    #[on_start]
    async fn on_start(&self, gx: &GlobalContext) {
        // Start the scheduled job immediately
        self.start_job_impl(gx);
    }

    #[keybinds]
    fn keys() {
        bind("space", toggle_job);
        bind("r", reset_counter);
        bind("q", quit);
    }

    /// Create a handler closure for the scheduled job.
    /// 
    /// This creates a Handler (Arc<dyn Fn(&HandlerContext) + Send + Sync>)
    /// that can be passed to schedule_every/schedule_after.
    fn toast_handler(&self) -> Handler {
        let app = self.clone();
        Arc::new(move |hx| {
            // Clone app state and spawn async task
            let counter = app.counter.clone();
            let gx = hx.gx().clone();
            tokio::spawn(async move {
                let count = counter.get() + 1;
                counter.set(count);
                gx.toast(Toast::success(format!("Scheduled toast #{}", count)));
            });
        })
    }

    fn start_job_impl(&self, gx: &GlobalContext) {
        if self.running.get() {
            return; // Already running
        }

        // Schedule a recurring job every 5 seconds
        let handler = self.toast_handler();
        let job_id = gx.schedule_every(Duration::from_secs(5), handler);
        self.job_id.set(Some(job_id));
        self.running.set(true);
        gx.toast(Toast::info("Started scheduled toasts (every 5 seconds)"));
    }

    fn stop_job_impl(&self, gx: &GlobalContext) {
        if !self.running.get() {
            return; // Not running
        }

        // Cancel the scheduled job
        if let Some(job_id) = self.job_id.get() {
            gx.cancel_job(job_id);
        }
        self.job_id.set(None);
        self.running.set(false);
        gx.toast(Toast::warning("Stopped scheduled toasts"));
    }

    #[handler]
    async fn toggle_job(&self, gx: &GlobalContext) {
        if self.running.get() {
            self.stop_job_impl(gx);
        } else {
            self.start_job_impl(gx);
        }
    }

    #[handler]
    async fn reset_counter(&self, gx: &GlobalContext) {
        self.counter.set(0);
        gx.toast(Toast::info("Counter reset to 0"));
    }

    #[handler]
    async fn quit(&self, gx: &GlobalContext) {
        gx.shutdown();
    }

    fn element(&self) -> Element {
        let counter_str = self.counter.get().to_string();
        let status = if self.running.get() {
            "Running"
        } else {
            "Stopped"
        };

        page! {
            column (padding: 1, gap: 1) style (bg: background) {
                column {
                    text (content: "Scheduled Toast Demo") style (bold, fg: primary)
                    text (content: "Shows a toast every 5 seconds with incrementing counter") style (fg: muted)
                }

                column (padding: 1) style (bg: surface) {
                    row (gap: 2) {
                        text (content: "Counter:") style (fg: muted)
                        text (content: {counter_str}) style (bold, fg: interact)
                    }
                    row (gap: 2) {
                        text (content: "Status:") style (fg: muted)
                        if self.running.get() {
                            text (content: {status}) style (bold, fg: success)
                        } else {
                            text (content: {status}) style (bold, fg: warning)
                        }
                    }
                }

                column (gap: 0) style (fg: muted) {
                    text (content: "space  toggle scheduled job")
                    text (content: "r      reset counter")
                    text (content: "q      quit")
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Set up file logging
    let log_file = File::create("scheduled_toast.log").expect("Failed to create log file");
    WriteLogger::init(LevelFilter::Debug, Config::default(), log_file)
        .expect("Failed to initialize logger");

    if let Err(e) = Runtime::new()
        .expect("Failed to create runtime")
        .run(ScheduledToastApp::default())
        .await
    {
        eprintln!("Error: {}", e);
    }
}
