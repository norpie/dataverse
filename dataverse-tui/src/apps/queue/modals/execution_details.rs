//! Execution details modal - shows execution history for a queue item.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Text};

use crate::apps::queue::types::ExecutionRecord;

/// Modal that displays execution history records for a queue item.
#[modal(size = Lg)]
pub struct ExecutionDetailsModal {
    #[state(skip)]
    executions: Vec<ExecutionRecord>,
}

impl ExecutionDetailsModal {
    pub fn new(executions: Vec<ExecutionRecord>) -> Self {
        Self {
            executions,
            ..Default::default()
        }
    }
}

#[modal_impl]
impl ExecutionDetailsModal {
    fn default_result(&self) {}

    #[keybinds]
    fn keys() {
        bind("escape", close);
    }

    #[handler]
    async fn close(&self, mx: &ModalContext<()>) {
        mx.close(());
    }

    fn element(&self) -> Element {
        let content = self.format_executions();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Execution History") style (bold, fg: interact)
                column (height: fill, width: fill) style (overflow: scroll) {
                    text (content: {content})
                }
                row (width: fill, justify: end) {
                    button (label: "Close", hint: "esc", id: "close") on_activate: close()
                }
            }
        }
    }

    fn format_executions(&self) -> String {
        if self.executions.is_empty() {
            return "No execution history.".to_string();
        }

        let mut lines = Vec::new();
        for (i, exec) in self.executions.iter().enumerate() {
            if i > 0 {
                lines.push("---".to_string());
            }
            lines.push(format!("Status: {:?}", exec.status));
            lines.push(format!(
                "Started: {}",
                exec.started_at.format("%Y-%m-%d %H:%M:%S")
            ));
            lines.push(format!("Duration: {}ms", exec.duration_ms));
            lines.push(format!(
                "Results: {} ok, {} failed",
                exec.success_count, exec.failure_count
            ));
            if let Some(error) = &exec.error {
                lines.push(format!("Error: {}", error));
            }
        }

        lines.join("\n")
    }
}
