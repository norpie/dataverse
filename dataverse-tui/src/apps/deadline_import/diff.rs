use chrono::{DateTime, NaiveDate, Utc};
use dataverse_lib::model::Value;
use uuid::Uuid;

use super::operations::{committee_datetime, deadline_datetime, generated_name};
use super::types::{AssociationDiff, DeadlineMode, DeadlineRecord, ExistingAssociations};

pub fn apply_diffs(records: &mut [DeadlineRecord]) {
    for record in records {
        if matches!(record.mode, DeadlineMode::Error(_)) {
            continue;
        }
        let Some(existing) = &record.existing else {
            record.mode = DeadlineMode::Create;
            continue;
        };
        let field_changes = field_change_count(record) > 0;
        let association_changes = diff_associations(record, &existing.associations).has_changes();
        record.mode = if field_changes || association_changes {
            DeadlineMode::Update
        } else {
            DeadlineMode::Unchanged
        };
    }
}

pub fn field_change_count(record: &DeadlineRecord) -> usize {
    let Some(existing) = &record.existing else {
        return 0;
    };
    let mut count = 0;

    for (field, value) in &record.fields.direct {
        if !value_equals_string(existing.fields.get(field), value) {
            count += 1;
        }
    }

    if let Some(datetime) = deadline_datetime(record) {
        if !value_equals_datetime(existing.fields.get("nrq_deadlinedate"), datetime) {
            count += 1;
        }
    }

    if let Some(datetime) = committee_datetime(record) {
        if !value_equals_datetime(existing.fields.get("nrq_committeemeetingdate"), datetime) {
            count += 1;
        }
    }

    if let Some(name) = generated_name(record) {
        if !value_equals_string(existing.fields.get("nrq_name"), &name) {
            count += 1;
        }
    }

    for (field, lookup) in &record.fields.lookups {
        let value_field = format!("_{}_value", field.to_lowercase());
        if !value_equals_guid(existing.fields.get(&value_field), lookup.id) {
            count += 1;
        }
    }

    for (field, value) in &record.fields.picklists {
        if !value_equals_i32(existing.fields.get(field), *value) {
            count += 1;
        }
    }

    for (field, value) in &record.fields.booleans {
        if !value_equals_bool(existing.fields.get(field), *value) {
            count += 1;
        }
    }

    count
}

pub fn diff_associations(
    record: &DeadlineRecord,
    existing: &ExistingAssociations,
) -> AssociationDiff {
    let mut diff = AssociationDiff::default();
    diff.support_to_add = add_pairs(&record.associations.support, &existing.support);
    diff.support_to_remove = existing
        .support
        .iter()
        .filter(|(id, _)| !record.associations.support.contains_key(id))
        .map(|(_, record)| record.clone())
        .collect();
    diff.category_to_add = add_pairs(&record.associations.category, &existing.category);
    diff.category_to_remove = remove_pairs(&record.associations.category, &existing.category);
    diff.subcategory_to_add = add_pairs(&record.associations.subcategory, &existing.subcategory);
    diff.subcategory_to_remove =
        remove_pairs(&record.associations.subcategory, &existing.subcategory);
    diff.flemishshare_to_add = add_pairs(&record.associations.flemishshare, &existing.flemishshare);
    diff.flemishshare_to_remove =
        remove_pairs(&record.associations.flemishshare, &existing.flemishshare);
    diff
}

fn add_pairs<T>(
    new: &std::collections::HashMap<Uuid, String>,
    existing: &std::collections::HashMap<Uuid, T>,
) -> Vec<(Uuid, String)> {
    new.iter()
        .filter(|(id, _)| !existing.contains_key(id))
        .map(|(id, name)| (*id, name.clone()))
        .collect()
}

fn remove_pairs(
    new: &std::collections::HashMap<Uuid, String>,
    existing: &std::collections::HashMap<Uuid, String>,
) -> Vec<(Uuid, String)> {
    existing
        .iter()
        .filter(|(id, _)| !new.contains_key(id))
        .map(|(id, name)| (*id, name.clone()))
        .collect()
}

fn value_equals_string(value: Option<&Value>, expected: &str) -> bool {
    match value {
        Some(Value::String(value)) => value == expected,
        Some(Value::Json(value)) => value.as_str() == Some(expected),
        _ => expected.is_empty() && value.is_none(),
    }
}

fn value_equals_guid(value: Option<&Value>, expected: Uuid) -> bool {
    match value {
        Some(Value::Guid(value)) => *value == expected,
        Some(Value::String(value)) => Uuid::parse_str(value).ok() == Some(expected),
        Some(Value::Json(value)) => {
            value.as_str().and_then(|value| Uuid::parse_str(value).ok()) == Some(expected)
        }
        _ => false,
    }
}

fn value_equals_i32(value: Option<&Value>, expected: i32) -> bool {
    match value {
        Some(Value::Int(value)) => *value == expected,
        Some(Value::Long(value)) => *value == expected as i64,
        Some(Value::OptionSet(value)) => value.value == expected,
        Some(Value::Json(value)) => value.as_i64() == Some(expected as i64),
        _ => false,
    }
}

fn value_equals_bool(value: Option<&Value>, expected: bool) -> bool {
    match value {
        Some(Value::Bool(value)) => *value == expected,
        Some(Value::Json(value)) => value.as_bool() == Some(expected),
        _ => false,
    }
}

fn value_equals_datetime(value: Option<&Value>, expected: DateTime<Utc>) -> bool {
    let expected_date = expected.date_naive();
    match value {
        Some(Value::DateTime(value)) => value.date_naive() == expected_date,
        Some(Value::String(value)) => parse_date(value) == Some(expected_date),
        Some(Value::Json(value)) => value.as_str().and_then(parse_date) == Some(expected_date),
        _ => false,
    }
}

fn parse_date(value: &str) -> Option<NaiveDate> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.date_naive())
        .ok()
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%SZ")
                .ok()
                .map(|dt| dt.date())
        })
        .or_else(|| chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
}
