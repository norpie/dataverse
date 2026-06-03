use rafter::prelude::*;

use crate::file_io::{FileIoError, write_excel};
use crate::modals::{FileBrowserModal, LoadingModal};
use crate::paths;

use super::QuestionnaireValidator;
use super::types::ValidationFindingRow;

impl QuestionnaireValidator {
    pub(super) async fn export_bulk_validation_results(&self, gx: &GlobalContext) {
        let Some(result) = self.bulk_result.get() else {
            gx.toast(Toast::error("Run bulk validation first"));
            return;
        };
        if result.rows.is_empty() {
            gx.toast(Toast::info("No validation failures to export"));
            return;
        }

        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let default_filename = format!("questionnaire_validation_{}", timestamp);
        let start_dir = paths::downloads_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        let modal = FileBrowserModal::browse(&start_dir, vec!["xlsx".to_string()])
            .with_filename(default_filename);

        let Some(file) = gx.modal(modal).await else {
            return;
        };

        let headers = export_headers();
        let rows = export_rows(&result.rows);
        let path = file.path.clone();
        let row_count = rows.len();

        let write_result = gx
            .modal(LoadingModal::run_with_default(
                "Exporting validation failures...",
                || Err(FileIoError::Cancelled),
                async move {
                    tokio::task::spawn_blocking(move || {
                        write_excel(&path, &headers, &rows, "Validation Failures")
                    })
                    .await
                    .map_err(|e| {
                        FileIoError::Io(std::io::Error::other(format!("Task join error: {}", e)))
                    })?
                },
            ))
            .await;

        match write_result {
            Ok(()) => gx.toast(Toast::success(format!(
                "Exported {} validation failures to {}",
                row_count,
                file.path.display()
            ))),
            Err(e) if e.is_cancelled() => {}
            Err(e) => gx.toast(Toast::error(format!("Export failed: {}", e))),
        }
    }
}

fn export_headers() -> Vec<String> {
    [
        "Questionnaire Name",
        "Questionnaire ID",
        "Questionnaire Code",
        "Questionnaire State",
        "Questionnaire Status",
        "Entity",
        "Record Name",
        "Record ID",
        "Category",
        "Item",
        "Value",
        "Detail",
    ]
    .iter()
    .map(|header| header.to_string())
    .collect()
}

fn export_rows(rows: &[ValidationFindingRow]) -> Vec<Vec<String>> {
    rows.iter()
        .map(|row| {
            vec![
                row.questionnaire_name.clone(),
                row.questionnaire_id.clone(),
                row.questionnaire_code.clone(),
                row.questionnaire_state.clone(),
                row.questionnaire_status.clone(),
                row.entity.clone(),
                row.record_name.clone(),
                row.record_id.clone(),
                row.category.clone(),
                row.item.clone(),
                row.value.clone(),
                row.detail.clone(),
            ]
        })
        .collect()
}
