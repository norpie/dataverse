use dataverse_lib::model::Record;
use uuid::Uuid;

use super::util::{guid_value, option_value, short_id, string_value};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ValidatorView {
    List,
    Detail,
}

impl Default for ValidatorView {
    fn default() -> Self {
        Self::List
    }
}

#[derive(Clone, Debug)]
pub(super) struct QuestionnaireSummary {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) code: Option<String>,
    pub(super) questionnaire_type: Option<i32>,
    pub(super) statecode: Option<i32>,
    pub(super) statuscode: Option<i32>,
}

impl QuestionnaireSummary {
    pub(super) fn from_record(record: &Record) -> Self {
        let id = guid_value(record, "nrq_questionnaireid")
            .map(|id| id.to_string())
            .unwrap_or_else(|| "unknown-id".to_string());
        let name = string_value(record, "nrq_name").unwrap_or_else(|| "unknown name".to_string());

        Self {
            id,
            name,
            code: string_value(record, "nrq_code"),
            questionnaire_type: option_value(record, "nrq_type"),
            statecode: option_value(record, "statecode"),
            statuscode: option_value(record, "statuscode"),
        }
    }

    pub(super) fn id_uuid(&self) -> Option<Uuid> {
        Uuid::parse_str(&self.id).ok()
    }

    pub(super) fn is_active(&self) -> bool {
        self.statecode == Some(0)
    }

    pub(super) fn state_label(&self) -> &'static str {
        if self.is_active() {
            "active"
        } else {
            "inactive"
        }
    }

    pub(super) fn short_id(&self) -> String {
        short_id(&self.id)
    }
}

#[derive(Clone, Debug)]
pub(super) struct EntityRecordSet {
    pub(super) entity: String,
    pub(super) records: Vec<Record>,
}

#[derive(Clone, Debug)]
pub(super) struct QuestionnaireGraph {
    pub(super) records_by_entity: Vec<EntityRecordSet>,
}

#[derive(Clone, Debug)]
pub(super) struct ValidatedField {
    pub(super) field: String,
    pub(super) value: Option<i32>,
    pub(super) accepted_values: Vec<i32>,
    pub(super) valid: bool,
}

#[derive(Clone, Debug)]
pub(super) struct ConditionQuestionValidation {
    pub(super) question_id: String,
    pub(super) visible: bool,
    pub(super) required: bool,
    pub(super) valid: bool,
}

#[derive(Clone, Debug)]
pub(super) struct ConditionActionValidation {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) visible: Option<bool>,
    pub(super) required: Option<bool>,
    pub(super) valid: bool,
}

#[derive(Clone, Debug)]
pub(super) struct ConditionValidation {
    pub(super) mode: Option<String>,
    pub(super) trigger_question_id: Option<String>,
    pub(super) operator: Option<String>,
    pub(super) value: Option<String>,
    pub(super) parameter_type: Option<String>,
    pub(super) parameter_values: Vec<String>,
    pub(super) questions: Vec<ConditionQuestionValidation>,
    pub(super) actions: Vec<ConditionActionValidation>,
    pub(super) valid: bool,
}

impl ConditionValidation {
    pub(super) fn finding_count(&self) -> usize {
        let mut count = 0;
        if !self.valid {
            count += 1;
        }
        count += self
            .questions
            .iter()
            .filter(|question| !question.valid)
            .count();
        count += self.actions.iter().filter(|action| !action.valid).count();
        count
    }
}

#[derive(Clone, Debug)]
pub(super) struct RecordValidation {
    pub(super) entity: String,
    pub(super) record_id: String,
    pub(super) name: String,
    pub(super) fields: Vec<ValidatedField>,
    pub(super) condition: Option<ConditionValidation>,
}

impl RecordValidation {
    pub(super) fn finding_count(&self) -> usize {
        self.fields.iter().filter(|field| !field.valid).count()
            + self
                .condition
                .as_ref()
                .map(ConditionValidation::finding_count)
                .unwrap_or(0)
    }
}

#[derive(Clone, Debug)]
pub(super) struct EntityValidation {
    pub(super) entity: String,
    pub(super) record_count: usize,
    pub(super) findings_count: usize,
    pub(super) records: Vec<RecordValidation>,
}

#[derive(Clone, Debug)]
pub(super) struct ValidationReport {
    pub(super) questionnaire: QuestionnaireSummary,
    pub(super) record_count: usize,
    pub(super) finding_count: usize,
    pub(super) entities: Vec<EntityValidation>,
}
