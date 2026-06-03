use std::collections::{HashMap, HashSet};

use dataverse_lib::model::Record;
use dataverse_lib::model::Value;
use dataverse_lib::model::metadata::EntityMetadata;
use serde_json::Value as JsonValue;

use super::types::{
    ConditionActionValidation, ConditionQuestionValidation, ConditionValidation, EntityRecordSet,
    EntityValidation, QuestionnaireGraph, QuestionnaireSummary, RecordValidation, ValidatedField,
    ValidationReport,
};
use super::util::{
    bool_value, entity_spec, guid_value, json_value_to_string, record_name, string_value,
};

pub(super) fn build_validation_report(
    questionnaire: QuestionnaireSummary,
    graph: QuestionnaireGraph,
    metadata: HashMap<String, EntityMetadata>,
) -> ValidationReport {
    let option_values = build_option_value_map(&metadata);
    let question_ids = collect_question_ids(&graph.records_by_entity);
    let condition_actions = build_condition_action_map(&graph.records_by_entity);
    let mut entities = Vec::new();
    let mut record_count = 0;
    let mut finding_count = 0;

    for set in graph.records_by_entity {
        if set.entity == "nrq_questionconditionaction" {
            continue;
        }

        let mut records = Vec::new();
        let Some(spec) = entity_spec(&set.entity) else {
            continue;
        };

        for record in &set.records {
            record_count += 1;
            let record_id = guid_value(record, spec.primary_key)
                .map(|id| id.to_string())
                .unwrap_or_else(|| "unknown-id".to_string());
            let name = record_name(record);
            let fields = validate_record_options(&set.entity, record, &option_values);
            let condition = if set.entity == "nrq_questioncondition" {
                Some(build_condition_validation(
                    record,
                    &question_ids,
                    condition_actions
                        .get(&record_id)
                        .map(|actions| actions.as_slice())
                        .unwrap_or(&[]),
                ))
            } else {
                None
            };
            let record_findings = fields.iter().filter(|field| !field.valid).count()
                + condition
                    .as_ref()
                    .map(ConditionValidation::finding_count)
                    .unwrap_or(0);
            finding_count += record_findings;
            records.push(RecordValidation {
                entity: set.entity.clone(),
                record_id,
                name,
                fields,
                condition,
            });
        }

        let entity_findings = records.iter().map(RecordValidation::finding_count).sum();
        entities.push(EntityValidation {
            entity: set.entity,
            record_count: set.records.len(),
            findings_count: entity_findings,
            records,
        });
    }

    ValidationReport {
        questionnaire,
        record_count,
        finding_count,
        entities,
    }
}

fn build_option_value_map(
    metadata: &HashMap<String, EntityMetadata>,
) -> HashMap<(String, String), Vec<i32>> {
    let mut map = HashMap::new();
    for (entity, entity_metadata) in metadata {
        for attribute in &entity_metadata.picklist_attributes {
            if !attribute.logical_name.starts_with("nrq_") {
                continue;
            }
            map.insert(
                (entity.clone(), attribute.logical_name.clone()),
                attribute
                    .option_set
                    .options
                    .iter()
                    .map(|option| option.value)
                    .collect(),
            );
        }
        for attribute in &entity_metadata.multi_select_picklist_attributes {
            if !attribute.logical_name.starts_with("nrq_") {
                continue;
            }
            map.insert(
                (entity.clone(), attribute.logical_name.clone()),
                attribute
                    .option_set
                    .options
                    .iter()
                    .map(|option| option.value)
                    .collect(),
            );
        }
        for attribute in &entity_metadata.state_attributes {
            if !attribute.logical_name.starts_with("nrq_") {
                continue;
            }
            map.insert(
                (entity.clone(), attribute.logical_name.clone()),
                attribute
                    .option_set
                    .options
                    .iter()
                    .map(|option| option.value)
                    .collect(),
            );
        }
        for attribute in &entity_metadata.status_attributes {
            if !attribute.logical_name.starts_with("nrq_") {
                continue;
            }
            map.insert(
                (entity.clone(), attribute.logical_name.clone()),
                attribute
                    .option_set
                    .options
                    .iter()
                    .map(|option| option.value)
                    .collect(),
            );
        }
    }
    log::debug!(
        "Questionnaire validator option-set metadata fields: {}",
        map.len()
    );
    map
}

fn collect_question_ids(records_by_entity: &[EntityRecordSet]) -> HashSet<String> {
    let mut ids = HashSet::new();
    for set in records_by_entity {
        if set.entity != "nrq_question" {
            continue;
        }
        for record in &set.records {
            if let Some(id) = guid_value(record, "nrq_questionid") {
                ids.insert(id.to_string());
            }
        }
    }
    ids
}

fn build_condition_action_map(
    records_by_entity: &[EntityRecordSet],
) -> HashMap<String, Vec<ConditionActionValidation>> {
    let mut map: HashMap<String, Vec<ConditionActionValidation>> = HashMap::new();
    for set in records_by_entity {
        if set.entity != "nrq_questionconditionaction" {
            continue;
        }
        for record in &set.records {
            let Some(condition_id) = guid_value(record, "nrq_questionconditionid") else {
                continue;
            };
            let action = ConditionActionValidation {
                id: guid_value(record, "nrq_questionconditionactionid")
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "unknown-id".to_string()),
                name: record_name(record),
                visible: bool_value(record, "nrq_visible"),
                required: bool_value(record, "nrq_required"),
                valid: bool_value(record, "nrq_visible").is_some()
                    && bool_value(record, "nrq_required").is_some(),
            };
            map.entry(condition_id.to_string())
                .or_default()
                .push(action);
        }
    }
    map
}

fn build_condition_validation(
    record: &Record,
    question_ids: &HashSet<String>,
    actions: &[ConditionActionValidation],
) -> ConditionValidation {
    let raw_json = string_value(record, "nrq_conditionjson");
    let mut valid = true;
    let mut mode = None;
    let mut trigger_question_id = None;
    let mut operator = None;
    let mut value = None;
    let mut parameter_type = None;
    let mut parameter_values = Vec::new();
    let mut questions = Vec::new();

    let Some(raw_json_str) = raw_json.as_deref() else {
        return ConditionValidation {
            mode,
            trigger_question_id,
            operator,
            value,
            parameter_type,
            parameter_values,
            questions,
            actions: actions.to_vec(),
            valid: false,
        };
    };

    let Ok(json) = serde_json::from_str::<JsonValue>(raw_json_str) else {
        return ConditionValidation {
            mode,
            trigger_question_id,
            operator,
            value,
            parameter_type,
            parameter_values,
            questions,
            actions: actions.to_vec(),
            valid: false,
        };
    };

    mode = json
        .get("type")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    trigger_question_id = json
        .get("questionId")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    operator = json
        .get("condition")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    value = json.get("value").map(json_value_to_string);
    parameter_type = json
        .get("parameterType")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    parameter_values = json
        .get("parameterValues")
        .and_then(|value| value.as_array())
        .map(|values| values.iter().map(json_value_to_string).collect())
        .unwrap_or_default();

    if mode.is_none() {
        mode = if parameter_type.is_some() || !parameter_values.is_empty() {
            Some("parameter".to_string())
        } else if trigger_question_id.is_some() {
            Some("question".to_string())
        } else {
            None
        };
    }

    match mode.as_deref() {
        Some("question") => {
            if trigger_question_id
                .as_ref()
                .map(|id| question_ids.contains(id))
                .unwrap_or(false)
            {
                // ok
            } else {
                valid = false;
            }
        }
        Some("parameter") => {
            if parameter_type.as_deref().unwrap_or("").is_empty() {
                valid = false;
            }
            if parameter_values.is_empty() {
                valid = false;
            }
        }
        Some(_) | None => {
            valid = false;
        }
    }

    if operator.as_deref().unwrap_or("").is_empty() {
        valid = false;
    }

    if let Some(items) = json.get("questions").and_then(|value| value.as_array()) {
        for item in items {
            let question_id = item
                .get("questionId")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown")
                .to_string();
            let visible = item
                .get("visible")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let required = item
                .get("required")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let question_valid = question_ids.contains(&question_id);
            if !question_valid {
                valid = false;
            }
            questions.push(ConditionQuestionValidation {
                question_id,
                visible,
                required,
                valid: question_valid,
            });
        }
    } else {
        valid = false;
    }

    ConditionValidation {
        mode,
        trigger_question_id,
        operator,
        value,
        parameter_type,
        parameter_values,
        questions,
        actions: actions.to_vec(),
        valid,
    }
}

fn validate_record_options(
    entity: &str,
    record: &Record,
    option_values: &HashMap<(String, String), Vec<i32>>,
) -> Vec<ValidatedField> {
    let mut fields = Vec::new();
    let mut option_fields = option_values
        .keys()
        .filter_map(|(option_entity, field)| {
            if option_entity == entity {
                Some(field.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    option_fields.sort();

    for field in option_fields {
        let Some(value) = record.get(&field) else {
            add_missing_option_field(entity, &field, option_values, &mut fields);
            continue;
        };

        match value {
            Value::OptionSet(option) => {
                validate_option_value(entity, &field, option.value, option_values, &mut fields);
            }
            Value::MultiOptionSet(options) => {
                if options.values.is_empty() {
                    add_missing_option_field(entity, &field, option_values, &mut fields);
                }
                for value in &options.values {
                    validate_option_value(entity, &field, *value, option_values, &mut fields);
                }
            }
            Value::Int(value) => {
                validate_option_value(entity, &field, *value, option_values, &mut fields);
            }
            Value::Long(value) => {
                if let Ok(value) = i32::try_from(*value) {
                    validate_option_value(entity, &field, value, option_values, &mut fields);
                }
            }
            Value::String(value) => {
                let parsed = parse_option_string(value);
                if parsed.is_empty() {
                    add_missing_option_field(entity, &field, option_values, &mut fields);
                }
                for value in parsed {
                    validate_option_value(entity, &field, value, option_values, &mut fields);
                }
            }
            Value::Null => {
                add_missing_option_field(entity, &field, option_values, &mut fields);
            }
            _ => {
                add_missing_option_field(entity, &field, option_values, &mut fields);
            }
        }
    }
    fields.sort_by(|a, b| a.field.cmp(&b.field).then(a.value.cmp(&b.value)));
    fields
}

fn parse_option_string(value: &str) -> Vec<i32> {
    value
        .split(',')
        .filter_map(|part| part.trim().parse::<i32>().ok())
        .collect()
}

fn add_missing_option_field(
    entity: &str,
    field: &str,
    option_values: &HashMap<(String, String), Vec<i32>>,
    fields: &mut Vec<ValidatedField>,
) {
    let key = (entity.to_string(), field.to_string());
    let Some(accepted_values) = option_values.get(&key) else {
        return;
    };
    fields.push(ValidatedField {
        field: field.to_string(),
        value: None,
        accepted_values: accepted_values.clone(),
        valid: true,
    });
}

fn validate_option_value(
    entity: &str,
    field: &str,
    value: i32,
    option_values: &HashMap<(String, String), Vec<i32>>,
    fields: &mut Vec<ValidatedField>,
) {
    let key = (entity.to_string(), field.to_string());
    let Some(accepted_values) = option_values.get(&key) else {
        return;
    };
    fields.push(ValidatedField {
        field: field.to_string(),
        value: Some(value),
        accepted_values: accepted_values.clone(),
        valid: accepted_values.contains(&value),
    });
}
