//! Preview types and helpers for the migration editor.

use std::collections::HashMap;

use rafter::widgets::{Column, TableRow};
use tuidom::{Color, Element, Style};

use crate::apps::migration::comparison::MappingComparison;
use crate::apps::migration::comparison::OperationType;
use crate::apps::migration::comparison::OperationTypeCounts;
use crate::formatting::format_value;

// =============================================================================
// PreviewRow
// =============================================================================

/// A single row in the preview table.
#[derive(Debug, Clone)]
pub struct PreviewRow {
    /// Unique row index.
    pub key: usize,
    /// The determined operation.
    pub op: OperationType,
    /// Source record ID (GUID string) or "(orphan)".
    pub source_id: String,
    /// Info string: diff count, error message, or empty.
    pub info: String,
    /// Field name → display value for scrollable data columns.
    pub fields: HashMap<String, String>,
}

impl TableRow for PreviewRow {
    type Key = usize;

    fn key(&self) -> usize {
        self.key
    }

    fn cell(&self, column_id: &str) -> Element {
        match column_id {
            "op" => {
                let (label, color) = op_label_color(&self.op);
                Element::text(label).style(Style::new().foreground(Color::var(color)))
            }
            "source_id" => Element::text(&self.source_id),
            "info" => Element::text(&self.info),
            _ => {
                let text = self.fields.get(column_id).map(|s| s.as_str()).unwrap_or("");
                Element::text(text)
            }
        }
    }
}

// =============================================================================
// Building rows and columns from MappingComparison
// =============================================================================

/// Build preview rows and dynamic columns from a MappingComparison.
///
/// Returns (rows, all_columns) where all_columns includes frozen meta columns
/// and dynamic data columns.
pub fn build_preview_table(comparison: &MappingComparison) -> (Vec<PreviewRow>, Vec<Column>) {
    let mut rows = Vec::new();
    let mut field_names: Vec<String> = Vec::new();

    // Collect all unique field names from transformed records and orphans
    for record in &comparison.records {
        for field in record.transformed.keys() {
            if !field_names.contains(field) {
                field_names.push(field.clone());
            }
        }
    }
    for orphan in &comparison.orphans {
        for field in orphan.fields.keys() {
            if !field_names.contains(field) {
                field_names.push(field.clone());
            }
        }
    }
    field_names.sort();

    // Build rows from source record comparisons
    for (i, record) in comparison.records.iter().enumerate() {
        let source_id = record
            .source_id
            .map(|id| format!("{}", id))
            .unwrap_or_else(|| "(no id)".to_string());

        let info = match &record.operation {
            OperationType::Update => {
                let n = record.diffs.len();
                if n == 1 {
                    "1 diff".to_string()
                } else {
                    format!("{} diffs", n)
                }
            }
            OperationType::Error(msg) => msg.clone(),
            _ => {
                if !record.errors.is_empty() {
                    format!("{} errors", record.errors.len())
                } else {
                    String::new()
                }
            }
        };

        let mut fields = HashMap::new();
        for (field, value) in &record.transformed {
            fields.insert(field.clone(), format_value(value).display);
        }

        rows.push(PreviewRow {
            key: i,
            op: record.operation.clone(),
            source_id,
            info,
            fields,
        });
    }

    // Build rows from orphaned target records
    let orphan_offset = comparison.records.len();
    for (i, orphan) in comparison.orphans.iter().enumerate() {
        let target_id = orphan
            .record_id
            .map(|id| format!("{}", id))
            .unwrap_or_else(|| "(no id)".to_string());

        let mut fields = HashMap::new();
        for (field, value) in &orphan.fields {
            fields.insert(field.clone(), format_value(value).display);
        }

        rows.push(PreviewRow {
            key: orphan_offset + i,
            op: orphan.operation.clone(),
            source_id: format!("(orphan) {}", target_id),
            info: String::new(),
            fields,
        });
    }

    // Build columns: frozen meta + dynamic data
    let mut columns = vec![
        Column::new("op", "Op").fixed(12),
        Column::new("source_id", "Source ID").fixed(38),
        Column::new("info", "Info").fixed(14),
    ];
    for name in &field_names {
        columns.push(Column::new(name, name).fixed(24));
    }

    (rows, columns)
}

// =============================================================================
// Helpers
// =============================================================================

/// Get display label and color variable name for an operation type.
pub fn op_label_color(op: &OperationType) -> (&'static str, &'static str) {
    match op {
        OperationType::Create => ("CREATE", "success"),
        OperationType::Update => ("UPDATE", "info"),
        OperationType::Skip => ("SKIP", "muted"),
        OperationType::Delete => ("DELETE", "error"),
        OperationType::Deactivate => ("DEACTIVATE", "warning"),
        OperationType::Associate => ("ASSOCIATE", "success"),
        OperationType::Disassociate => ("DISASSOC", "error"),
        OperationType::IgnoreSource => ("IGN. SOURCE", "muted"),
        OperationType::IgnoreTarget => ("IGN. TARGET", "muted"),
        OperationType::Error(_) => ("ERROR", "error"),
    }
}

/// Get entity display names from comparison results.
pub fn entity_names(results: &[MappingComparison]) -> Vec<String> {
    results
        .iter()
        .map(|mc| {
            let total = mc.records.len() + mc.orphans.len();
            format!("{} ({})", mc.target_entity, total)
        })
        .collect()
}

/// Get operation counts for the current entity.
pub fn entity_counts(results: &[MappingComparison], index: usize) -> OperationTypeCounts {
    results
        .get(index)
        .map(|mc| mc.count_operations())
        .unwrap_or_default()
}
