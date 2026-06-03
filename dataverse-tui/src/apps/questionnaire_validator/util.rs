use dataverse_lib::model::Record;
use dataverse_lib::model::Value;
use serde_json::Value as JsonValue;
use uuid::Uuid;

pub(super) fn entity_spec(
    logical_name: &str,
) -> Option<&'static crate::apps::questionnaire_sync::scope::QuestionnaireEntitySpec> {
    crate::apps::questionnaire_sync::scope::QUESTIONNAIRE_ENTITIES
        .iter()
        .find(|spec| spec.logical_name == logical_name)
}

pub(super) fn record_name(record: &Record) -> String {
    string_value(record, "nrq_name").unwrap_or_else(|| {
        record
            .fields()
            .iter()
            .find_map(|(field, value)| {
                if field.ends_with("id") {
                    if let Value::Guid(id) = value {
                        return Some(short_id(&id.to_string()));
                    }
                }
                None
            })
            .unwrap_or_else(|| "unknown name".to_string())
    })
}

pub(super) fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

pub(super) fn string_value(record: &Record, field: &str) -> Option<String> {
    match record.get(field) {
        Some(Value::String(value)) => Some(value.clone()),
        _ => None,
    }
}

pub(super) fn guid_value(record: &Record, field: &str) -> Option<Uuid> {
    match record.get(field) {
        Some(Value::Guid(value)) => Some(*value),
        _ => None,
    }
}

pub(super) fn option_value(record: &Record, field: &str) -> Option<i32> {
    match record.get(field) {
        Some(Value::OptionSet(value)) => Some(value.value),
        Some(Value::Int(value)) => Some(*value),
        _ => None,
    }
}

pub(super) fn bool_value(record: &Record, field: &str) -> Option<bool> {
    match record.get(field) {
        Some(Value::Bool(value)) => Some(*value),
        _ => None,
    }
}

pub(super) fn json_value_to_string(value: &JsonValue) -> String {
    match value {
        JsonValue::String(value) => value.clone(),
        JsonValue::Bool(value) => value.to_string(),
        JsonValue::Number(value) => value.to_string(),
        JsonValue::Null => "null".to_string(),
        _ => value.to_string(),
    }
}
