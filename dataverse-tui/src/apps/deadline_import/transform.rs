use std::collections::HashMap;

use chrono::{NaiveDate, NaiveTime};
use uuid::Uuid;

use super::excel::{ExcelRow, WorkbookData};
use super::fetch::{record_id, record_name};
use super::scope::{self, FieldKind};
use super::types::{
    DeadlineAssociations, DeadlineFields, DeadlineMode, DeadlineRecord, ImportContext, ImportData,
    LookupCache, LookupValue,
};

pub fn transform_workbook(
    workbook: WorkbookData,
    context: ImportContext,
    file_path: std::path::PathBuf,
    sheet_name: String,
) -> ImportData {
    let mut warnings = Vec::new();
    let mut records = Vec::new();
    let header_index = build_header_index(&workbook.headers);
    let board_meetings = build_board_meeting_lookup(&context.cache);

    for row in workbook.rows {
        let mut record = transform_row(
            &row,
            &workbook.headers,
            &header_index,
            &context.cache,
            &board_meetings,
        );
        if !matches!(record.mode, DeadlineMode::Error(_)) {
            if let Some(existing) = context.existing_deadlines.get(&record.id) {
                record.existing = Some(existing.clone());
                record.mode = DeadlineMode::Update;
            }
        }
        for warning in &record.warnings {
            warnings.push(format!("Row {}: {}", record.source_row, warning));
        }
        if let Some(notes) = &record.notes {
            warnings.push(format!("Row {} [OPM]: {}", record.source_row, notes));
        }
        records.push(record);
    }

    ImportData {
        file_path,
        sheet_name,
        records,
        warnings,
    }
}

fn transform_row(
    row: &ExcelRow,
    headers: &[String],
    header_index: &HashMap<String, usize>,
    cache: &LookupCache,
    board_meetings: &HashMap<NaiveDate, (Uuid, String)>,
) -> DeadlineRecord {
    let mut warnings = Vec::new();
    let mut fields = DeadlineFields::default();
    let mut associations = DeadlineAssociations::default();
    let notes = cell_by_header(row, header_index, "OPM").filter(|value| !value.trim().is_empty());

    let (id, id_from_excel) = parse_row_id(row, header_index, &mut warnings);

    for mapping in scope::FIELD_MAPPINGS {
        let Some(raw_value) = cell_by_header(row, header_index, mapping.column) else {
            if mapping.required {
                warnings.push(format!("Required field '{}' is missing", mapping.column));
            }
            continue;
        };
        let value = raw_value.trim();
        if value.is_empty() {
            if mapping.required {
                warnings.push(format!("Required field '{}' is empty", mapping.column));
            }
            continue;
        }

        match &mapping.kind {
            FieldKind::Direct => {
                fields.direct.insert(
                    mapping.field.to_string(),
                    truncate_value(value, mapping.field),
                );
            }
            FieldKind::Lookup { target_entity } => {
                if fields.lookups.contains_key(mapping.field) {
                    continue;
                }
                let resolved =
                    if mapping.column.contains("Raad") && mapping.column.contains("Bestuur") {
                        parse_excel_date(value)
                            .ok()
                            .and_then(|date| board_meetings.get(&date).cloned())
                            .map(|(id, label)| LookupValue {
                                id,
                                target_entity: (*target_entity).to_string(),
                                target_set: entity_set(cache, target_entity),
                                label,
                            })
                    } else {
                        resolve_lookup(cache, target_entity, value)
                    };

                if let Some(lookup) = resolved {
                    fields.lookups.insert(mapping.field.to_string(), lookup);
                } else {
                    warnings.push(format!(
                        "Lookup '{}' not found: '{}'",
                        mapping.column, value
                    ));
                }
            }
            FieldKind::Date => match parse_excel_date(value) {
                Ok(date) if mapping.field == "nrq_deadlinedate" => {
                    fields.deadline_date = Some(date)
                }
                Ok(date) if mapping.field == "nrq_committeemeetingdate" => {
                    fields.committee_date = Some(date)
                }
                Ok(_) => {}
                Err(e) => warnings.push(format!("Invalid date in '{}': {}", mapping.column, e)),
            },
            FieldKind::Time => match parse_time(value) {
                Ok(time) if mapping.field == "nrq_deadlinedate" => {
                    fields.deadline_time = Some(time)
                }
                Ok(time) if mapping.field == "nrq_committeemeetingdate" => {
                    fields.committee_time = Some(time)
                }
                Ok(_) => {}
                Err(e) => warnings.push(format!("Invalid time in '{}': {}", mapping.column, e)),
            },
            FieldKind::Picklist(options) => {
                if let Some((_, option)) = options.iter().find(|(label, _)| *label == value) {
                    fields.picklists.insert(mapping.field.to_string(), *option);
                } else {
                    let expected = options
                        .iter()
                        .map(|(label, _)| *label)
                        .collect::<Vec<_>>()
                        .join(", ");
                    warnings.push(format!(
                        "Invalid value in '{}': '{}'. Expected one of: {}",
                        mapping.column, value, expected
                    ));
                }
            }
            FieldKind::Boolean {
                true_value,
                false_value,
            } => {
                if value.eq_ignore_ascii_case(true_value) {
                    fields.booleans.insert(mapping.field.to_string(), true);
                } else if value.eq_ignore_ascii_case(false_value) {
                    fields.booleans.insert(mapping.field.to_string(), false);
                } else {
                    warnings.push(format!(
                        "Invalid boolean value in '{}': '{}'. Expected '{}' or '{}'",
                        mapping.column, value, true_value, false_value
                    ));
                }
            }
        }
    }

    resolve_checkbox_columns(row, headers, cache, &mut associations, &mut warnings);

    if !fields.direct.contains_key("nrq_deadlinename") {
        warnings.push("Missing required field: Deadline Name".to_string());
    }
    if fields.deadline_date.is_none() {
        warnings.push("Missing required field: Deadline Date".to_string());
    }

    let mode = if warnings.is_empty() {
        DeadlineMode::Create
    } else {
        DeadlineMode::Error("Row has validation warnings".to_string())
    };

    DeadlineRecord {
        source_row: row.source_row,
        id,
        id_from_excel,
        mode,
        fields,
        associations,
        existing: None,
        warnings,
        notes,
    }
}

fn parse_row_id(
    row: &ExcelRow,
    header_index: &HashMap<String, usize>,
    warnings: &mut Vec<String>,
) -> (Uuid, Option<Uuid>) {
    let Some(value) = cell_by_header(row, header_index, "id") else {
        return (Uuid::new_v4(), None);
    };
    let value = value.trim();
    if value.is_empty() {
        return (Uuid::new_v4(), None);
    }
    match Uuid::parse_str(value) {
        Ok(id) => (id, Some(id)),
        Err(_) => {
            warnings.push(format!("Invalid id GUID: '{}'", value));
            (Uuid::new_v4(), None)
        }
    }
}

fn resolve_checkbox_columns(
    row: &ExcelRow,
    headers: &[String],
    cache: &LookupCache,
    associations: &mut DeadlineAssociations,
    warnings: &mut Vec<String>,
) {
    let start_idx = headers
        .iter()
        .position(|header| {
            header.to_lowercase().contains("raad") && header.to_lowercase().contains("bestuur")
        })
        .or_else(|| headers.iter().position(|header| header == "Type"));
    let Some(start_idx) = start_idx else {
        return;
    };

    for (idx, header) in headers.iter().enumerate().skip(start_idx + 1) {
        let upper = header.to_uppercase();
        if header.trim().is_empty() || upper == "OPM" || upper == "IGNORE" {
            continue;
        }
        let Some(value) = row.cells.get(idx) else {
            continue;
        };
        if !is_checked(value) {
            continue;
        }

        if let Some((id, name)) = find_named_record(cache, scope::ENTITY_SUPPORT, header) {
            associations.support.insert(id, name);
        } else if let Some((id, name)) = find_named_record(cache, scope::ENTITY_CATEGORY, header) {
            associations.category.insert(id, name);
        } else if let Some((id, name)) = find_named_record(cache, scope::ENTITY_SUBCATEGORY, header)
        {
            associations.subcategory.insert(id, name);
        } else if let Some((id, name)) =
            find_named_record(cache, scope::ENTITY_FLEMISHSHARE, header)
        {
            associations.flemishshare.insert(id, name);
        } else {
            warnings.push(format!(
                "Checkbox column '{}' not found in NRQ lookup data",
                header
            ));
        }
    }
}

fn resolve_lookup(cache: &LookupCache, entity: &str, value: &str) -> Option<LookupValue> {
    find_named_record(cache, entity, value).map(|(id, label)| LookupValue {
        id,
        target_entity: entity.to_string(),
        target_set: entity_set(cache, entity),
        label,
    })
}

fn find_named_record(cache: &LookupCache, entity: &str, value: &str) -> Option<(Uuid, String)> {
    let records = cache.records.get(entity)?;
    for record in records {
        let Some(name) = record_name(record, entity) else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case(value.trim()) {
            let Some(id) = record_id(record, entity) else {
                continue;
            };
            return Some((id, name));
        }
    }
    None
}

fn build_board_meeting_lookup(cache: &LookupCache) -> HashMap<NaiveDate, (Uuid, String)> {
    let mut lookup = HashMap::new();
    let Some(records) = cache.records.get("nrq_boardofdirectorsmeeting") else {
        return lookup;
    };

    for record in records {
        let Some(id) = record_id(record, "nrq_boardofdirectorsmeeting") else {
            continue;
        };
        let Some(name) = record_name(record, "nrq_boardofdirectorsmeeting") else {
            continue;
        };
        let normalized = normalize_board_meeting_name(&name);
        let prefix = if normalized.starts_with("bestuur - ") {
            "bestuur - "
        } else if normalized.starts_with("bestuur + algemene vergadering - ") {
            "bestuur + algemene vergadering - "
        } else if normalized.starts_with("raad van bestuur - ") {
            "raad van bestuur - "
        } else {
            continue;
        };
        let date_part = normalized[prefix.len()..]
            .split_whitespace()
            .next()
            .unwrap_or("");
        if let Ok(date) = parse_board_meeting_date(date_part) {
            lookup.insert(date, (id, name));
        }
    }
    lookup
}

fn normalize_board_meeting_name(name: &str) -> String {
    name.to_lowercase()
        .replace('\u{00A0}', " ")
        .replace('\u{2009}', " ")
        .replace('\u{202F}', " ")
        .trim()
        .to_string()
}

fn parse_board_meeting_date(value: &str) -> Result<NaiveDate, String> {
    for format in [
        "%-d/%-m/%Y",
        "%-d/%m/%Y",
        "%d/%-m/%Y",
        "%d/%m/%Y",
        "%-d-%-m-%Y",
        "%d-%m-%Y",
        "%Y-%m-%d",
    ] {
        if let Ok(date) = NaiveDate::parse_from_str(value, format) {
            return Ok(date);
        }
    }

    parse_excel_date(value)
}

fn cell_by_header(
    row: &ExcelRow,
    header_index: &HashMap<String, usize>,
    header: &str,
) -> Option<String> {
    let idx = header_index.get(&header.to_lowercase())?;
    row.cells.get(*idx).cloned()
}

fn build_header_index(headers: &[String]) -> HashMap<String, usize> {
    headers
        .iter()
        .enumerate()
        .map(|(idx, header)| (header.to_lowercase(), idx))
        .collect()
}

fn entity_set(cache: &LookupCache, entity: &str) -> String {
    cache
        .entity_sets
        .get(entity)
        .cloned()
        .unwrap_or_else(|| format!("{}s", entity))
}

fn truncate_value(value: &str, field: &str) -> String {
    let max_len = match field {
        "nrq_deadlinename" | "nrq_name" => 200,
        "nrq_description" => 2000,
        _ => usize::MAX,
    };
    value.chars().take(max_len).collect()
}

fn is_checked(value: &str) -> bool {
    matches!(
        value.trim().to_lowercase().as_str(),
        "x" | "1" | "true" | "yes"
    )
}

pub fn parse_excel_date(value: &str) -> Result<NaiveDate, String> {
    if let Ok(serial) = value.parse::<f64>() {
        if !(1.0..=100000.0).contains(&serial) {
            return Err(format!("Invalid Excel date serial: {serial}"));
        }
        let base = NaiveDate::from_ymd_opt(1899, 12, 30).unwrap();
        return base
            .checked_add_days(chrono::Days::new(serial as u64))
            .ok_or_else(|| format!("Date calculation overflow for serial: {serial}"));
    }

    if let Ok(datetime) = chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S") {
        return Ok(datetime.date());
    }

    for format in [
        "%Y-%m-%d", "%d/%m/%Y", "%m/%d/%Y", "%d-%m-%Y", "%m-%d-%Y", "%Y/%m/%d",
    ] {
        if let Ok(date) = NaiveDate::parse_from_str(value, format) {
            return Ok(date);
        }
    }

    Err(format!("Could not parse as date: {value}"))
}

fn parse_time(value: &str) -> Result<NaiveTime, String> {
    if let Ok(fraction) = value.parse::<f64>() {
        if (0.0..=1.0).contains(&fraction) {
            let total_seconds = (fraction * 86400.0) as u32;
            return NaiveTime::from_hms_opt(
                total_seconds / 3600,
                (total_seconds % 3600) / 60,
                total_seconds % 60,
            )
            .ok_or_else(|| "Invalid time components".to_string());
        }
    }

    for format in ["%H:%M", "%H:%M:%S", "%I:%M %p", "%I:%M:%S %p"] {
        if let Ok(time) = NaiveTime::parse_from_str(value, format) {
            return Ok(time);
        }
    }

    Err(format!("Could not parse as time: {value}"))
}
