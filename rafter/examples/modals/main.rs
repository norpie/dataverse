//! Modal Showcase Example
//!
//! Demonstrates all modal features in rafter:
//! - Size presets (Auto, Sm, Md, Lg, Fixed, Proportional)
//! - Positioning (Centered, At { x, y })
//! - Lifecycle hooks (on_start)
//! - Result types ((), bool, String, enum)
//! - Nested modals
//! - Keybinds within modals

mod choice;
mod input;
mod nested;
mod position;
mod sizes;

use std::fs::File;

use choice::{ChoiceModal, ConfirmModal};
use input::{InputModal, NameModal, NameResult};
use nested::{Level1Modal, OuterModal};
use position::{CornerModal, PositionedModal};
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Text};
use simplelog::{Config, LevelFilter, WriteLogger};
use sizes::{AutoModal, FixedModal, LgModal, MdModal, ProportionalModal, SmModal};

// ============================================================================
// Modal Showcase App
// ============================================================================

#[app]
struct ModalShowcase {
    last_result: String,
}

#[app_impl]
impl ModalShowcase {
    #[keybinds]
    fn keys() {
        bind("1", show_auto);
        bind("2", show_sm);
        bind("3", show_md);
        bind("4", show_lg);
        bind("5", show_fixed);
        bind("6", show_proportional);
        bind("7", show_positioned);
        bind("8", show_corner);
        bind("i", show_input);
        bind("n", show_name);
        bind("c", show_choice);
        bind("y", show_confirm);
        bind("o", show_outer);
        bind("d", show_deep);
        bind("q", quit);
    }

    // Size modals
    #[handler]
    async fn show_auto(&self, cx: &AppContext) {
        cx.modal(AutoModal::default()).await;
        self.last_result.set("Auto modal closed".to_string());
    }

    #[handler]
    async fn show_sm(&self, cx: &AppContext) {
        cx.modal(SmModal::default()).await;
        self.last_result.set("Small modal closed".to_string());
    }

    #[handler]
    async fn show_md(&self, cx: &AppContext) {
        cx.modal(MdModal::default()).await;
        self.last_result.set("Medium modal closed".to_string());
    }

    #[handler]
    async fn show_lg(&self, cx: &AppContext) {
        cx.modal(LgModal::default()).await;
        self.last_result.set("Large modal closed".to_string());
    }

    #[handler]
    async fn show_fixed(&self, cx: &AppContext) {
        cx.modal(FixedModal::default()).await;
        self.last_result.set("Fixed size modal closed".to_string());
    }

    #[handler]
    async fn show_proportional(&self, cx: &AppContext) {
        cx.modal(ProportionalModal::default()).await;
        self.last_result
            .set("Proportional modal closed".to_string());
    }

    // Position modals
    #[handler]
    async fn show_positioned(&self, cx: &AppContext) {
        cx.modal(PositionedModal::default()).await;
        self.last_result
            .set("Positioned modal closed (check logs for on_start)".to_string());
    }

    #[handler]
    async fn show_corner(&self, cx: &AppContext) {
        cx.modal(CornerModal::default()).await;
        self.last_result.set("Corner modal closed".to_string());
    }

    // Input modals
    #[handler]
    async fn show_input(&self, cx: &AppContext) {
        let result = cx
            .modal(InputModal::with_prompt("Enter some text:"))
            .await;
        match result {
            Some(text) => self.last_result.set(format!("Input: \"{}\"", text)),
            None => self.last_result.set("Input cancelled".to_string()),
        }
    }

    #[handler]
    async fn show_name(&self, cx: &AppContext) {
        let result = cx.modal(NameModal::default()).await;
        match result {
            Some(NameResult {
                first_name,
                last_name,
            }) => self
                .last_result
                .set(format!("Name: {} {}", first_name, last_name)),
            None => self.last_result.set("Name input cancelled".to_string()),
        }
    }

    // Choice modals
    #[handler]
    async fn show_choice(&self, cx: &AppContext) {
        let result = cx.modal(ChoiceModal::new("Make a choice")).await;
        match result {
            Some(choice) => self.last_result.set(format!("Choice: {}", choice)),
            None => self.last_result.set("Choice cancelled".to_string()),
        }
    }

    #[handler]
    async fn show_confirm(&self, cx: &AppContext) {
        let result = cx.modal(ConfirmModal::new("Do you want to proceed?")).await;
        self.last_result.set(format!("Confirmed: {}", result));
    }

    // Nested modals
    #[handler]
    async fn show_outer(&self, cx: &AppContext) {
        let count = cx.modal(OuterModal::default()).await;
        self.last_result
            .set(format!("Nested modal: {} confirmations", count));
    }

    #[handler]
    async fn show_deep(&self, cx: &AppContext) {
        let result = cx.modal(Level1Modal::default()).await;
        self.last_result
            .set(format!("Deep nesting complete: {}", result));
    }

    #[handler]
    async fn quit(&self, gx: &GlobalContext) {
        gx.shutdown();
    }

    fn element(&self) -> Element {
        let last_result = self.last_result.get();

        page! {
            column (padding: 1, gap: 1) style (bg: background) {
                // Header
                column {
                    text (content: "Modal Showcase") style (bold, fg: primary)
                    text (content: "Press keys to open different modals") style (fg: muted)
                }

                // Last result
                row (padding: 1) style (bg: surface) {
                    text (content: "Last result:") style (fg: muted)
                    text (content: {last_result}) style (fg: success)
                }

                // Size section
                column (gap: 1) {
                    text (content: "Size Presets") style (bold, fg: secondary)
                    row (gap: 1) {
                        button (label: "[1] Auto", id: "auto") on_activate: show_auto()
                        button (label: "[2] Small", id: "sm") on_activate: show_sm()
                        button (label: "[3] Medium", id: "md") on_activate: show_md()
                        button (label: "[4] Large", id: "lg") on_activate: show_lg()
                    }
                    row (gap: 1) {
                        button (label: "[5] Fixed", id: "fixed") on_activate: show_fixed()
                        button (label: "[6] Proportional", id: "prop") on_activate: show_proportional()
                    }
                }

                // Position section
                column (gap: 1) {
                    text (content: "Positioning") style (bold, fg: secondary)
                    row (gap: 1) {
                        button (label: "[7] Positioned (5,3)", id: "pos") on_activate: show_positioned()
                        button (label: "[8] Corner (2,2)", id: "corner") on_activate: show_corner()
                    }
                }

                // Input section
                column (gap: 1) {
                    text (content: "Input Modals") style (bold, fg: secondary)
                    row (gap: 1) {
                        button (label: "[I] Text Input", id: "input_btn") on_activate: show_input()
                        button (label: "[N] Name Form", id: "name") on_activate: show_name()
                    }
                }

                // Choice section
                column (gap: 1) {
                    text (content: "Choice Modals") style (bold, fg: secondary)
                    row (gap: 1) {
                        button (label: "[C] Multi-choice", id: "choice") on_activate: show_choice()
                        button (label: "[Y] Yes/No", id: "confirm") on_activate: show_confirm()
                    }
                }

                // Nested section
                column (gap: 1) {
                    text (content: "Nested Modals") style (bold, fg: secondary)
                    row (gap: 1) {
                        button (label: "[O] Outer (2 levels)", id: "outer") on_activate: show_outer()
                        button (label: "[D] Deep (3 levels)", id: "deep") on_activate: show_deep()
                    }
                }

                // Footer
                column (gap: 1) {
                    text (content: "Press Q to quit") style (fg: muted)
                    text (content: "Global modals (via System): Ctrl+G = Global Confirm, Ctrl+T = Global Input") style (fg: muted)
                }
            }
        }
    }
}

// ============================================================================
// Global Modal System
// ============================================================================

use rafter::system_impl;

#[rafter::system]
struct GlobalModalSystem;

#[system_impl]
impl GlobalModalSystem {
    #[keybinds]
    fn keys() {
        bind("ctrl+g", show_global_confirm);
        bind("ctrl+t", show_global_input);
    }

    #[handler]
    async fn show_global_confirm(&self, gx: &GlobalContext) {
        let result = gx.modal(ConfirmModal::new("This is a GLOBAL modal!")).await;
        log::info!("Global confirm result: {}", result);
    }

    #[handler]
    async fn show_global_input(&self, gx: &GlobalContext) {
        let result = gx
            .modal(InputModal::with_prompt("Global input (works from anywhere):"))
            .await;
        log::info!("Global input result: {:?}", result);
    }
}

#[tokio::main]
async fn main() {
    // Set up file logging
    let log_file = File::create("modal.log").expect("Failed to create log file");
    WriteLogger::init(LevelFilter::Debug, Config::default(), log_file)
        .expect("Failed to initialize logger");

    // GlobalModalSystem is auto-registered via inventory from #[system] macro
    if let Err(e) = Runtime::new()
        .expect("Failed to create runtime")
        .run(ModalShowcase::default())
        .await
    {
        eprintln!("Error: {}", e);
    }
}
