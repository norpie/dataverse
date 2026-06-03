//! Tree nodes and builders for questionnaire sync.

use std::collections::BTreeMap;

use dataverse_lib::model::Record;
use dataverse_lib::model::Value;
use rafter::element;
use rafter::widgets::Text;
use rafter::widgets::TreeItem;
use rafter::widgets::TreeNode;
use tuidom::Color;
use tuidom::Element;
use uuid::Uuid;

use crate::apps::migration::engine::util::values_equal;
use crate::apps::questionnaire_sync::comparison::QuestionnaireComparison;
use crate::apps::questionnaire_sync::comparison::QuestionnaireEntityComparison;
use crate::apps::questionnaire_sync::comparison::QuestionnaireOperation;
use crate::apps::questionnaire_sync::scope::QuestionnaireEntitySpec;
use crate::apps::questionnaire_sync::scope::QUESTIONNAIRE_ENTITIES;
use crate::formatting::format_value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuestionnaireTreeSide {
    Source,
    Target,
}

impl QuestionnaireTreeSide {
    fn label(self) -> &'static str {
        match self {
            Self::Source => "Source",
            Self::Target => "Target",
        }
    }

}

#[derive(Clone, Debug)]
pub enum QuestionnaireTreeNode {
    Entity {
        entity: String,
        side: QuestionnaireTreeSide,
        source_count: usize,
        target_count: usize,
        create_count: usize,
        update_count: usize,
        skip_count: usize,
        delete_count: usize,
        error_count: usize,
    },
    Record {
        entity: String,
        record_key: String,
        side: QuestionnaireTreeSide,
        operation: QuestionnaireOperation,
        source_name: String,
        target_name: String,
        source_present: bool,
        target_present: bool,
        source_id: Option<Uuid>,
        target_id: Option<Uuid>,
        diff_count: usize,
        source_is_active: bool,
        target_is_active: bool,
    },
    Field {
        entity: String,
        record_key: String,
        field: String,
        side: QuestionnaireTreeSide,
        source_value: Value,
        target_value: Value,
        changed: bool,
        source_present: bool,
        target_present: bool,
    },
}

impl TreeItem for QuestionnaireTreeNode {
    type Key = String;

    fn key(&self) -> String {
        match self {
            Self::Entity { entity, .. } => format!("entity-{}", entity),
            Self::Record {
                entity,
                record_key,
                ..
            } => format!("record-{}-{}", entity, record_key),
            Self::Field {
                entity,
                record_key,
                field,
                ..
            } => format!("field-{}-{}-{}", entity, record_key, field),
        }
    }

    fn render(&self) -> Element {
        match self {
            Self::Entity {
                entity,
                side,
                source_count,
                target_count,
                create_count,
                update_count,
                skip_count,
                delete_count,
                error_count,
            } => {
                let count = match side {
                    QuestionnaireTreeSide::Source => *source_count,
                    QuestionnaireTreeSide::Target => *target_count,
                };
                element! {
                    row (gap: 1) {
                        text (content: {entity.clone()}) style (bold, fg: interact)
                        text (content: {format!("[{}]", side.label())}) style (fg: muted)
                        text (content: {format!("{}", count)}) style (fg: primary)
                        if matches!(side, QuestionnaireTreeSide::Target) {
                            text (content: {format!("C:{} U:{} S:{} D:{} E:{}", create_count, update_count, skip_count, delete_count, error_count)}) style (fg: muted)
                        }
                    }
                }
            }
            Self::Record {
                record_key,
                side,
                operation,
                source_name,
                target_name,
                source_present,
                target_present,
                source_id,
                target_id,
                diff_count,
                source_is_active,
                target_is_active,
                ..
            } => {
                let id = match side {
                    QuestionnaireTreeSide::Source => (*source_id).or(*target_id),
                    QuestionnaireTreeSide::Target => (*target_id).or(*source_id),
                };
                let id_text = id
                    .map(short_guid)
                    .unwrap_or_else(|| short_key(record_key));
                let active_text = match side {
                    QuestionnaireTreeSide::Source => active_marker(*source_is_active),
                    QuestionnaireTreeSide::Target => active_marker(*target_is_active),
                };
                let source_side = matches!(side, QuestionnaireTreeSide::Source);
                let (label, color) = record_label(operation, *source_present, *target_present, *side);
                let display_name = match side {
                    QuestionnaireTreeSide::Source => source_name.clone(),
                    QuestionnaireTreeSide::Target => target_name.clone(),
                };
                let show_gap = source_side && !*target_present;
                let show_target_meta = !source_side;

                if source_side {
                    element! {
                        row (gap: 1) {
                            text (content: {display_name}) style (fg: primary)
                            text (content: {id_text}) style (fg: muted)
                            if !active_text.is_empty() {
                                text (content: {active_text}) style (fg: muted)
                            }
                            if show_gap {
                                text (content: "[gap]") style (fg: warning)
                            }
                        }
                    }
                } else {
                    element! {
                        row (gap: 1) {
                            text (content: {label}) style (fg: {Color::var(color)})
                            text (content: {display_name}) style (fg: primary)
                            text (content: {id_text}) style (fg: muted)
                            if *diff_count > 0 {
                                text (content: {format!("({} diffs)", diff_count)}) style (fg: warning)
                            }
                            if !active_text.is_empty() {
                                text (content: {active_text}) style (fg: muted)
                            }
                            if show_target_meta && !*source_present {
                                text (content: "[gap]") style (fg: warning)
                            }
                        }
                    }
                }
            }
            Self::Field {
                field,
                side,
                source_value,
                target_value,
                changed,
                source_present,
                target_present,
                ..
            } => {
                let source_side = matches!(side, QuestionnaireTreeSide::Source);
                let (own_value, other_present, arrow) = if source_side {
                    (source_value, *target_present, "→")
                } else {
                    (target_value, *source_present, "←")
                };
                let own = format_value(own_value);
                let own_color = if *changed { "primary" } else { "secondary" };
                let show_gap = !other_present;

                element! {
                    row (gap: 1) {
                        text (content: {field.clone()}) style (fg: muted)
                        text (content: {own.display}) style (fg: {Color::var(own_color)})
                        if !source_side && *changed {
                            text (content: {arrow}) style (fg: muted)
                            text (content: {format_value(source_value).display}) style (fg: muted)
                        }
                        if show_gap {
                            text (content: "[gap]") style (fg: warning)
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
struct QuestionnaireRecordEntry {
    key: String,
    source_record: Option<Record>,
    target_record: Option<Record>,
    source_name: String,
    target_name: String,
    operation: QuestionnaireOperation,
    diff_count: usize,
    source_is_active: bool,
    target_is_active: bool,
}

pub fn build_tree_nodes(
    comparison: &QuestionnaireComparison,
    side: QuestionnaireTreeSide,
    entity_index: usize,
) -> Vec<TreeNode<QuestionnaireTreeNode>> {
    let Some(entity) = comparison.entities.get(entity_index) else {
        return Vec::new();
    };

    let Some(spec) = QUESTIONNAIRE_ENTITIES
        .iter()
        .find(|spec| spec.logical_name == entity.entity)
    else {
        return Vec::new();
    };

    vec![build_entity_node(spec, entity, side)]
}

fn build_entity_node(
    spec: &QuestionnaireEntitySpec,
    entity: &QuestionnaireEntityComparison,
    side: QuestionnaireTreeSide,
) -> TreeNode<QuestionnaireTreeNode> {
    let records = build_record_entries(entity);
    let mut children = Vec::with_capacity(records.len());

    for record in records {
        children.push(build_record_node(spec, &record, side));
    }

    TreeNode::branch(
        QuestionnaireTreeNode::Entity {
            entity: spec.logical_name.to_string(),
            side,
            source_count: entity.records.len(),
            target_count: target_record_count(entity),
            create_count: entity.count_operations().create,
            update_count: entity.count_operations().update,
            skip_count: entity.count_operations().skip,
            delete_count: entity.count_operations().delete,
            error_count: entity.count_operations().error,
        },
        children,
    )
}

fn build_record_node(
    spec: &QuestionnaireEntitySpec,
    record: &QuestionnaireRecordEntry,
    side: QuestionnaireTreeSide,
) -> TreeNode<QuestionnaireTreeNode> {
    let mut children = Vec::with_capacity(spec.fields.len() + spec.state_fields.len());

    for field in spec
        .fields
        .iter()
        .map(|field| field.field_name)
        .chain(spec.state_fields.iter().copied())
    {
        let source_value = record
            .source_record
            .as_ref()
            .and_then(|r| r.get(field))
            .cloned()
            .unwrap_or(Value::Null);
        let target_value = record
            .target_record
            .as_ref()
            .and_then(|r| r.get(field))
            .cloned()
            .unwrap_or(Value::Null);
        let changed = !values_equal(&source_value, &target_value);

        children.push(TreeNode::leaf(QuestionnaireTreeNode::Field {
            entity: spec.logical_name.to_string(),
            record_key: record.key.clone(),
            field: field.to_string(),
            side,
            source_value,
            target_value,
            changed,
            source_present: record.source_record.is_some(),
            target_present: record.target_record.is_some(),
        }));
    }

    TreeNode::branch(
        QuestionnaireTreeNode::Record {
            entity: spec.logical_name.to_string(),
            record_key: record.key.clone(),
            side,
            operation: record.operation.clone(),
            source_name: record.source_name.clone(),
            target_name: record.target_name.clone(),
            source_present: record.source_record.is_some(),
            target_present: record.target_record.is_some(),
            source_id: record.source_record.as_ref().and_then(Record::id),
            target_id: record.target_record.as_ref().and_then(Record::id),
            diff_count: record.diff_count,
            source_is_active: record.source_is_active,
            target_is_active: record.target_is_active,
        },
        children,
    )
}

fn build_record_entries(entity: &QuestionnaireEntityComparison) -> Vec<QuestionnaireRecordEntry> {
    let mut entries: BTreeMap<String, QuestionnaireRecordEntry> = BTreeMap::new();

    for record in &entity.records {
        let key = record_key(record.source_id, record.target_id, &record.entity, entries.len());
        let entry = entries.entry(key.clone()).or_insert_with(|| QuestionnaireRecordEntry {
            key: key.clone(),
            source_record: None,
            target_record: None,
            source_name: "unknown name".to_string(),
            target_name: "unknown name".to_string(),
            operation: record.operation.clone(),
            diff_count: record.diffs.len(),
            source_is_active: record.source_is_active,
            target_is_active: record.target_is_active,
        });
        entry.source_name = record_name(&record.source_record);
        entry.target_name = record.target_record.as_ref().map(record_name).unwrap_or_else(|| "unknown name".to_string());
        entry.source_record = Some(record.source_record.clone());
        entry.target_record = record.target_record.clone();
        entry.operation = record.operation.clone();
        entry.diff_count = record.diffs.len();
        entry.source_is_active = record.source_is_active;
        entry.target_is_active = record.target_is_active;
    }

    for orphan in &entity.orphans {
        let key = record_key(None, orphan.record_id, &entity.entity, entries.len());
        let entry = entries.entry(key.clone()).or_insert_with(|| QuestionnaireRecordEntry {
            key: key.clone(),
            source_record: None,
            target_record: None,
            source_name: "unknown name".to_string(),
            target_name: "unknown name".to_string(),
            operation: orphan.operation.clone(),
            diff_count: 0,
            source_is_active: false,
            target_is_active: false,
        });
        entry.target_name = record_name(&orphan.record);
        entry.target_record = Some(orphan.record.clone());
        entry.operation = orphan.operation.clone();
    }

    entries.into_values().collect()
}

fn record_key(
    source_id: Option<Uuid>,
    target_id: Option<Uuid>,
    entity: &str,
    fallback_index: usize,
) -> String {
    source_id
        .or(target_id)
        .map(|id| id.to_string())
        .unwrap_or_else(|| format!("{}-missing-{}", entity, fallback_index))
}

fn target_record_count(entity: &QuestionnaireEntityComparison) -> usize {
    entity.records.iter().filter(|record| record.target_record.is_some()).count() + entity.orphans.len()
}

fn record_label(
    operation: &QuestionnaireOperation,
    source_present: bool,
    target_present: bool,
    side: QuestionnaireTreeSide,
) -> (&'static str, &'static str) {
    match (source_present, target_present, side) {
        (true, true, _) => operation_label(operation),
        (true, false, QuestionnaireTreeSide::Source) => operation_label(operation),
        (true, false, QuestionnaireTreeSide::Target) => ("GAP", "muted"),
        (false, true, QuestionnaireTreeSide::Source) => ("GAP", "muted"),
        (false, true, QuestionnaireTreeSide::Target) => operation_label(operation),
        (false, false, _) => ("GAP", "muted"),
    }
}

fn operation_label(operation: &QuestionnaireOperation) -> (&'static str, &'static str) {
    match operation {
        QuestionnaireOperation::Create => ("CREATE", "success"),
        QuestionnaireOperation::Update => ("UPDATE", "info"),
        QuestionnaireOperation::Skip => ("SKIP", "muted"),
        QuestionnaireOperation::Delete => ("DELETE", "error"),
        QuestionnaireOperation::Error(_) => ("ERROR", "error"),
    }
}

fn active_marker(is_active: bool) -> &'static str {
    if is_active {
        "[active]"
    } else {
        "[inactive]"
    }
}

fn record_name(record: &Record) -> String {
    record
        .get_string("nrq_name")
        .ok()
        .flatten()
        .map(|name| name.to_string())
        .unwrap_or_else(|| "unknown name".to_string())
}

fn source_record_name(record: &QuestionnaireRecordEntry) -> String {
    record.source_name.clone()
}

fn target_record_name(record: &QuestionnaireRecordEntry) -> String {
    record.target_name.clone()
}

fn short_guid(id: Uuid) -> String {
    let s = id.to_string();
    s[..8].to_string()
}

fn short_key(key: &str) -> String {
    key.chars().take(8).collect()
}
