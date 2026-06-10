use std::path::Path;

use calamine::{Data, Reader, Xlsx, open_workbook};

#[derive(Clone, Debug)]
pub struct WorkbookData {
    pub headers: Vec<String>,
    pub rows: Vec<ExcelRow>,
}

#[derive(Clone, Debug)]
pub struct ExcelRow {
    pub source_row: usize,
    pub cells: Vec<String>,
}

pub fn read_deadline_sheet(path: &Path, sheet_name: &str) -> Result<WorkbookData, String> {
    let mut workbook: Xlsx<_> =
        open_workbook(path).map_err(|e: calamine::XlsxError| e.to_string())?;
    let range = workbook
        .worksheet_range(sheet_name)
        .map_err(|e: calamine::XlsxError| e.to_string())?;

    let mut header_row_idx = None;
    let mut headers = Vec::new();

    for (idx, row) in range.rows().enumerate() {
        let row_values: Vec<String> = row.iter().map(cell_to_string).collect();
        let has_domain = row_values.iter().any(|cell| cell.contains("Domein"));
        let has_deadline = row_values.iter().any(|cell| cell.contains("Deadline"));
        if has_domain && has_deadline {
            header_row_idx = Some(idx);
            headers = row_values;
            break;
        }
    }

    let header_row_idx = header_row_idx.ok_or_else(|| {
        "Could not find header row (looking for a row containing Domein and Deadline)".to_string()
    })?;

    let ignore_idx = headers
        .iter()
        .position(|header| header.eq_ignore_ascii_case("IGNORE"));

    let mut rows = Vec::new();
    for (offset, row) in range.rows().skip(header_row_idx + 1).enumerate() {
        let source_row = header_row_idx + 2 + offset;
        let mut cells: Vec<String> = row.iter().map(cell_to_string).collect();
        cells.resize(headers.len(), String::new());

        if cells.iter().all(|cell| cell.trim().is_empty()) {
            continue;
        }

        if let Some(ignore_idx) = ignore_idx {
            if cells
                .get(ignore_idx)
                .is_some_and(|value| !value.trim().is_empty())
            {
                continue;
            }
        }

        rows.push(ExcelRow { source_row, cells });
    }

    Ok(WorkbookData { headers, rows })
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(value) => value.trim().to_string(),
        Data::Int(value) => value.to_string(),
        Data::Float(value) => {
            if value.fract() == 0.0 {
                (*value as i64).to_string()
            } else {
                value.to_string()
            }
        }
        Data::Bool(value) => value.to_string(),
        Data::DateTime(value) => value
            .as_datetime()
            .map(|date| date.format("%Y-%m-%dT%H:%M:%S").to_string())
            .unwrap_or_else(|| value.as_f64().to_string()),
        Data::DateTimeIso(value) => value.to_string(),
        Data::DurationIso(value) => value.to_string(),
        Data::Error(value) => format!("#ERROR:{value:?}"),
    }
}
