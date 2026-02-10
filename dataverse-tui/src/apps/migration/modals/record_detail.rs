//! Record detail modal — shows full comparison detail for a preview row.

use rafter::element;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::List;
use rafter::widgets::ListItem;
use rafter::widgets::ListState;
use rafter::widgets::Text;
use tuidom::Color;
use tuidom::Element;

use crate::apps::migration::comparison::OperationType;
use crate::apps::migration::comparison::OrphanRecord;
use crate::apps::migration::comparison::RecordComparison;
use crate::apps::migration::editor::preview::op_label_color;
use crate::formatting::format_value;

// =============================================================================
// Input types
// =============================================================================

/// Input for the record detail modal.
#[derive(Clone)]
pub enum RecordDetail {
    /// A source record comparison.
    Record(RecordComparison),
    /// An orphaned target record.
    Orphan(OrphanRecord),
}

impl Default for RecordDetail {
    fn default() -> Self {
        Self::Record(RecordComparison {
            operation: OperationType::Skip,
            source_record: dataverse_lib::model::Record::new(""),
            target_record: None,
            transformed: Default::default(),
            diffs: Vec::new(),
            errors: Vec::new(),
        })
    }
}

// =============================================================================
// Detail line — a single line in the list
// =============================================================================

/// A styled line of text in the detail view.
#[derive(Debug, Clone)]
pub struct DetailLine {
    key: usize,
    text: String,
    color: String,
    bold: bool,
}

impl ListItem for DetailLine {
    type Key = usize;

    fn key(&self) -> usize {
        self.key
    }

    fn render(&self) -> Element {
        let color = Color::var(&self.color);
        if self.bold {
            element! {
                text (content: {self.text.clone()}) style (bold, fg: {color})
            }
        } else {
            element! {
                text (content: {self.text.clone()}) style (fg: {color})
            }
        }
    }
}

// =============================================================================
// Modal
// =============================================================================

/// Page enum for the different detail views.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Record,
    Orphan,
}

/// Modal that displays full detail for a single preview row.
#[modal(default, size = Lg, pages)]
pub struct RecordDetailModal {
    #[state(skip)]
    detail: RecordDetail,
    #[state(skip)]
    op_label: String,
    #[state(skip)]
    op_color: String,
    #[state(skip)]
    subtitle: String,
    lines: ListState<DetailLine>,
}

impl RecordDetailModal {
    pub fn with_detail(detail: RecordDetail) -> Self {
        let (op_label, op_color, subtitle, lines) = match &detail {
            RecordDetail::Record(record) => {
                let (label, color) = op_label_color(&record.operation);
                let id = record
                    .source_record
                    .id()
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "(no id)".to_string());
                let lines = build_record_lines(record);
                (label.to_string(), color.to_string(), id, lines)
            }
            RecordDetail::Orphan(orphan) => {
                let (label, color) = op_label_color(&orphan.operation);
                let title = format!("Orphan {}", label);
                let id = orphan
                    .record
                    .id()
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "(no id)".to_string());
                let lines = build_orphan_lines(orphan);
                (title, color.to_string(), id, lines)
            }
        };

        Self::new(
            detail,
            op_label,
            op_color,
            subtitle,
            ListState::new(lines).with_selection(rafter::widgets::SelectionMode::None),
        )
    }
}

#[modal_impl]
impl RecordDetailModal {
    fn default_result(&self) {}

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<()>) {
        match &self.detail {
            RecordDetail::Record(_) => self.navigate(Page::Record),
            RecordDetail::Orphan(_) => self.navigate(Page::Orphan),
        }
        mx.focus("detail-list");
    }

    #[keybinds]
    fn keys() {
        bind("escape", close);
    }

    #[handler]
    async fn close(&self, mx: &ModalContext<()>) {
        mx.close(());
    }

    #[page(Record)]
    fn record_page(&self) -> Element {
        let op_label = self.op_label.clone();
        let op_color = Color::var(&self.op_color);
        let subtitle = self.subtitle.clone();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                row (gap: 1) {
                    text (content: {op_label}) style (bold, fg: {op_color})
                    text (content: {subtitle}) style (fg: muted)
                }
                list (state: self.lines, id: "detail-list", height: fill)
                row (width: fill, justify: end) {
                    button (label: "Close", hint: "esc", id: "close") on_activate: close()
                }
            }
        }
    }

    #[page(Orphan)]
    fn orphan_page(&self) -> Element {
        let op_label = self.op_label.clone();
        let op_color = Color::var(&self.op_color);
        let subtitle = self.subtitle.clone();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                row (gap: 1) {
                    text (content: {op_label}) style (bold, fg: {op_color})
                    text (content: {subtitle}) style (fg: muted)
                }
                list (state: self.lines, id: "detail-list", height: fill)
                row (width: fill, justify: end) {
                    button (label: "Close", hint: "esc", id: "close") on_activate: close()
                }
            }
        }
    }
}

// =============================================================================
// Line builders
// =============================================================================

fn build_record_lines(record: &RecordComparison) -> Vec<DetailLine> {
    let mut lines = Vec::new();
    let mut key = 0;

    // Errors section
    if !record.errors.is_empty() {
        lines.push(DetailLine {
            key,
            text: "Errors".to_string(),
            color: "error".to_string(),
            bold: true,
        });
        key += 1;
        for (field, error) in &record.errors {
            lines.push(DetailLine {
                key,
                text: format!("  {}: {}", field, error),
                color: "error".to_string(),
                bold: false,
            });
            key += 1;
        }
        lines.push(DetailLine {
            key,
            text: String::new(),
            color: "muted".to_string(),
            bold: false,
        });
        key += 1;
    }

    match &record.operation {
        OperationType::Error(msg) => {
            lines.push(DetailLine {
                key,
                text: "Error".to_string(),
                color: "error".to_string(),
                bold: true,
            });
            key += 1;
            lines.push(DetailLine {
                key,
                text: format!("  {}", msg),
                color: "error".to_string(),
                bold: false,
            });
        }
        OperationType::Update => {
            lines.push(DetailLine {
                key,
                text: "Diffs".to_string(),
                color: "info".to_string(),
                bold: true,
            });
            key += 1;
            for diff in &record.diffs {
                let old = format_value(&diff.old_value).display;
                let new = format_value(&diff.new_value).display;
                lines.push(DetailLine {
                    key,
                    text: format!("  {}: {} → {}", diff.field, old, new),
                    color: "primary".to_string(),
                    bold: false,
                });
                key += 1;
            }
            if record.diffs.is_empty() {
                lines.push(DetailLine {
                    key,
                    text: "  (no diffs)".to_string(),
                    color: "muted".to_string(),
                    bold: false,
                });
            }
        }
        OperationType::Create => {
            lines.push(DetailLine {
                key,
                text: "Fields".to_string(),
                color: "success".to_string(),
                bold: true,
            });
            key += 1;
            let mut fields: Vec<_> = record.transformed.iter().collect();
            fields.sort_by_key(|(k, _)| *k);
            for (field, value) in fields {
                let display = format_value(value).display;
                lines.push(DetailLine {
                    key,
                    text: format!("  {}: {}", field, display),
                    color: "primary".to_string(),
                    bold: false,
                });
                key += 1;
            }
        }
        OperationType::Skip => {
            lines.push(DetailLine {
                key,
                text: "No changes".to_string(),
                color: "muted".to_string(),
                bold: false,
            });
        }
        _ => {
            if !record.transformed.is_empty() {
                lines.push(DetailLine {
                    key,
                    text: "Fields".to_string(),
                    color: "primary".to_string(),
                    bold: true,
                });
                key += 1;
                let mut fields: Vec<_> = record.transformed.iter().collect();
                fields.sort_by_key(|(k, _)| *k);
                for (field, value) in fields {
                    let display = format_value(value).display;
                    lines.push(DetailLine {
                        key,
                        text: format!("  {}: {}", field, display),
                        color: "primary".to_string(),
                        bold: false,
                    });
                    key += 1;
                }
            }
        }
    }

    lines
}

fn build_orphan_lines(orphan: &OrphanRecord) -> Vec<DetailLine> {
    let mut lines = Vec::new();
    let mut key = 0;

    lines.push(DetailLine {
        key,
        text: "Target Record".to_string(),
        color: "primary".to_string(),
        bold: true,
    });
    key += 1;

    let mut fields: Vec<(&String, &dataverse_lib::model::Value)> =
        orphan.record.fields().into_iter().collect();
    fields.sort_by_key(|(k, _)| k.to_string());
    for (field, value) in fields {
        let display = format_value(value).display;
        lines.push(DetailLine {
            key,
            text: format!("  {}: {}", field, display),
            color: "primary".to_string(),
            bold: false,
        });
        key += 1;
    }

    lines
}
