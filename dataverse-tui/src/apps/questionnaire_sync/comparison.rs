use std::collections::HashMap;
use std::collections::HashSet;

use dataverse_lib::model::Record;
use dataverse_lib::model::Value;
use uuid::Uuid;

use crate::apps::migration::engine::util::values_equal;

use super::scope::QUESTIONNAIRE_ENTITIES;
use super::scope::QUESTIONNAIRE_RELATIONS;
use super::scope::QuestionnaireEntitySpec;
use super::scope::QuestionnaireRelationSpec;
use super::types::QuestionnaireEnvironmentSnapshot;
use super::types::QuestionnaireRelationMembership;

#[derive(Debug, Clone, PartialEq)]
pub enum QuestionnaireOperation {
    Create,
    Update,
    Skip,
    Delete,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct QuestionnaireFieldDiff {
    pub field: String,
    pub source_value: Value,
    pub target_value: Value,
}

#[derive(Debug, Clone)]
pub struct QuestionnaireRecordComparison {
    pub entity: String,
    pub operation: QuestionnaireOperation,
    pub source_id: Option<Uuid>,
    pub target_id: Option<Uuid>,
    pub source_record: Record,
    pub target_record: Option<Record>,
    pub diffs: Vec<QuestionnaireFieldDiff>,
    pub source_statecode: Value,
    pub target_statecode: Value,
    pub source_statuscode: Value,
    pub target_statuscode: Value,
    pub source_is_active: bool,
    pub target_is_active: bool,
}

impl QuestionnaireRecordComparison {
    pub fn has_diffs(&self) -> bool {
        !self.diffs.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct QuestionnaireOrphanComparison {
    pub operation: QuestionnaireOperation,
    pub record_id: Option<Uuid>,
    pub record: Record,
}

#[derive(Debug, Clone)]
pub struct QuestionnaireEntityComparison {
    pub entity: String,
    pub records: Vec<QuestionnaireRecordComparison>,
    pub orphans: Vec<QuestionnaireOrphanComparison>,
}

impl QuestionnaireEntityComparison {
    pub fn total_records(&self) -> usize {
        self.records.len() + self.orphans.len()
    }

    pub fn count_operations(&self) -> QuestionnaireOperationCounts {
        let mut counts = QuestionnaireOperationCounts::default();

        for record in &self.records {
            match &record.operation {
                QuestionnaireOperation::Create => counts.create += 1,
                QuestionnaireOperation::Update => counts.update += 1,
                QuestionnaireOperation::Skip => counts.skip += 1,
                QuestionnaireOperation::Error(_) => counts.error += 1,
                QuestionnaireOperation::Delete => {}
            }
        }

        for orphan in &self.orphans {
            match &orphan.operation {
                QuestionnaireOperation::Delete => counts.delete += 1,
                QuestionnaireOperation::Error(_) => counts.error += 1,
                QuestionnaireOperation::Create
                | QuestionnaireOperation::Update
                | QuestionnaireOperation::Skip => {}
            }
        }

        counts
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct QuestionnaireOperationCounts {
    pub create: usize,
    pub update: usize,
    pub skip: usize,
    pub delete: usize,
    pub error: usize,
}

#[derive(Debug, Clone)]
pub struct QuestionnaireRelationComparison {
    pub relationship_name: String,
    pub parent_entity: String,
    pub related_entity: String,
    pub associations: Vec<QuestionnaireRelationMembership>,
    pub disassociations: Vec<QuestionnaireRelationMembership>,
}

#[derive(Debug, Clone)]
pub struct QuestionnaireComparison {
    pub source: QuestionnaireEnvironmentSnapshot,
    pub target: QuestionnaireEnvironmentSnapshot,
    pub entities: Vec<QuestionnaireEntityComparison>,
    pub relations: Vec<QuestionnaireRelationComparison>,
}

impl QuestionnaireComparison {
    pub fn total_records(&self) -> usize {
        self.entities
            .iter()
            .map(QuestionnaireEntityComparison::total_records)
            .sum()
    }

    pub fn count_operations(&self) -> QuestionnaireOperationCounts {
        self.entities.iter().fold(
            QuestionnaireOperationCounts::default(),
            |mut acc, entity| {
                let counts = entity.count_operations();
                acc.create += counts.create;
                acc.update += counts.update;
                acc.skip += counts.skip;
                acc.delete += counts.delete;
                acc.error += counts.error;
                acc
            },
        )
    }
}

pub fn compare_questionnaire(
    source: &QuestionnaireEnvironmentSnapshot,
    target: &QuestionnaireEnvironmentSnapshot,
) -> QuestionnaireComparison {
    let mut entities = Vec::with_capacity(QUESTIONNAIRE_ENTITIES.len());

    for spec in QUESTIONNAIRE_ENTITIES {
        entities.push(compare_entity(spec, source, target));
    }

    let mut relations = Vec::with_capacity(QUESTIONNAIRE_RELATIONS.len());
    for spec in QUESTIONNAIRE_RELATIONS {
        relations.push(compare_relation(spec, source, target));
    }

    QuestionnaireComparison {
        source: source.clone(),
        target: target.clone(),
        entities,
        relations,
    }
}

fn compare_relation(
    spec: &QuestionnaireRelationSpec,
    source: &QuestionnaireEnvironmentSnapshot,
    target: &QuestionnaireEnvironmentSnapshot,
) -> QuestionnaireRelationComparison {
    let source_memberships: HashSet<QuestionnaireRelationMembership> = source
        .relation(spec.relationship_name)
        .map(|relation| relation.memberships.iter().cloned().collect())
        .unwrap_or_default();
    let target_memberships: HashSet<QuestionnaireRelationMembership> = target
        .relation(spec.relationship_name)
        .map(|relation| relation.memberships.iter().cloned().collect())
        .unwrap_or_default();

    let associations = source_memberships
        .difference(&target_memberships)
        .cloned()
        .collect();
    let disassociations = target_memberships
        .difference(&source_memberships)
        .cloned()
        .collect();

    QuestionnaireRelationComparison {
        relationship_name: spec.relationship_name.to_string(),
        parent_entity: spec.parent_entity.to_string(),
        related_entity: spec.related_entity.to_string(),
        associations,
        disassociations,
    }
}

fn compare_entity(
    spec: &QuestionnaireEntitySpec,
    source: &QuestionnaireEnvironmentSnapshot,
    target: &QuestionnaireEnvironmentSnapshot,
) -> QuestionnaireEntityComparison {
    let source_entity = source.entity(spec.logical_name);
    let target_entity = target.entity(spec.logical_name);

    let source_records = source_entity
        .map(|entity| entity.records.as_slice())
        .unwrap_or(&[]);
    let target_records = target_entity
        .map(|entity| entity.records.as_slice())
        .unwrap_or(&[]);

    let target_by_id = index_records(target_records, spec.primary_key);
    let mut matched_target_ids = HashSet::new();

    let mut records = Vec::with_capacity(source_records.len());
    for source_record in source_records {
        let source_id = record_id(source_record, spec.primary_key);
        let target_record = source_id.and_then(|id| target_by_id.get(&id));

        let (operation, target_id, diffs, target_statecode, target_statuscode, target_is_active) =
            match target_record {
                Some(target_record) => {
                    let diffs = diff_record(spec, source_record, target_record);
                    let target_id = record_id(target_record, spec.primary_key);
                    if let Some(id) = target_id {
                        matched_target_ids.insert(id);
                    }

                    let target_statecode = record_value(target_record, "statecode");
                    let target_statuscode = record_value(target_record, "statuscode");
                    let target_is_active = is_active_statecode(&target_statecode);

                    let operation = if diffs.is_empty() {
                        QuestionnaireOperation::Skip
                    } else {
                        QuestionnaireOperation::Update
                    };

                    (
                        operation,
                        target_id,
                        diffs,
                        target_statecode,
                        target_statuscode,
                        target_is_active,
                    )
                }
                None => {
                    let source_statecode = record_value(source_record, "statecode");
                    let source_statuscode = record_value(source_record, "statuscode");
                    let source_is_active = is_active_statecode(&source_statecode);

                    records.push(QuestionnaireRecordComparison {
                        entity: spec.logical_name.to_string(),
                        operation: QuestionnaireOperation::Create,
                        source_id,
                        target_id: None,
                        source_record: source_record.clone(),
                        target_record: None,
                        diffs: diff_record_against_empty(spec, source_record),
                        source_statecode,
                        target_statecode: Value::Null,
                        source_statuscode,
                        target_statuscode: Value::Null,
                        source_is_active,
                        target_is_active: true,
                    });
                    continue;
                }
            };

        let source_statecode = record_value(source_record, "statecode");
        let source_statuscode = record_value(source_record, "statuscode");
        let source_is_active = is_active_statecode(&source_statecode);

        records.push(QuestionnaireRecordComparison {
            entity: spec.logical_name.to_string(),
            operation,
            source_id,
            target_id,
            source_record: source_record.clone(),
            target_record: target_record.map(|record| (*record).clone()),
            diffs,
            source_statecode,
            target_statecode,
            source_statuscode,
            target_statuscode,
            source_is_active,
            target_is_active,
        });
    }

    let mut orphans = Vec::new();
    for target_record in target_records {
        let Some(target_id) = record_id(target_record, spec.primary_key) else {
            orphans.push(QuestionnaireOrphanComparison {
                operation: QuestionnaireOperation::Error("Missing target ID".to_string()),
                record_id: None,
                record: target_record.clone(),
            });
            continue;
        };

        if matched_target_ids.contains(&target_id) {
            continue;
        }

        orphans.push(QuestionnaireOrphanComparison {
            operation: QuestionnaireOperation::Delete,
            record_id: Some(target_id),
            record: target_record.clone(),
        });
    }

    QuestionnaireEntityComparison {
        entity: spec.logical_name.to_string(),
        records,
        orphans,
    }
}

fn diff_record(
    spec: &QuestionnaireEntitySpec,
    source: &Record,
    target: &Record,
) -> Vec<QuestionnaireFieldDiff> {
    let mut diffs = Vec::new();

    for field in spec
        .fields
        .iter()
        .map(|field| field.field_name)
        .chain(spec.state_fields.iter().copied())
    {
        let source_value = record_value(source, field);
        let target_value = record_value(target, field);

        if !values_equal(&source_value, &target_value) {
            diffs.push(QuestionnaireFieldDiff {
                field: field.to_string(),
                source_value,
                target_value,
            });
        }
    }

    diffs
}

fn diff_record_against_empty(
    spec: &QuestionnaireEntitySpec,
    source: &Record,
) -> Vec<QuestionnaireFieldDiff> {
    let mut diffs = Vec::new();

    for field in spec
        .fields
        .iter()
        .map(|field| field.field_name)
        .chain(spec.state_fields.iter().copied())
    {
        let source_value = record_value(source, field);
        if !matches!(source_value, Value::Null) {
            diffs.push(QuestionnaireFieldDiff {
                field: field.to_string(),
                source_value,
                target_value: Value::Null,
            });
        }
    }

    diffs
}

fn index_records<'a>(records: &'a [Record], primary_key: &str) -> HashMap<Uuid, &'a Record> {
    let mut index = HashMap::new();

    for record in records {
        if let Some(id) = record_id(record, primary_key) {
            index.insert(id, record);
        }
    }

    index
}

fn record_id(record: &Record, primary_key: &str) -> Option<Uuid> {
    record
        .id()
        .or_else(|| record_value(record, primary_key).as_guid())
}

fn record_value(record: &Record, field: &str) -> Value {
    record.get(field).cloned().unwrap_or(Value::Null)
}

fn is_active_statecode(value: &Value) -> bool {
    match value {
        Value::OptionSet(option) => option.value == 0,
        Value::Int(value) => *value == 0,
        Value::Long(value) => *value == 0,
        Value::Null => true,
        _ => true,
    }
}

trait ValueGuidExt {
    fn as_guid(&self) -> Option<Uuid>;
}

impl ValueGuidExt for Value {
    fn as_guid(&self) -> Option<Uuid> {
        match self {
            Value::Guid(id) => Some(*id),
            Value::String(s) => Uuid::parse_str(s).ok(),
            _ => None,
        }
    }
}
