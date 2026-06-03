use rafter::element;
use rafter::prelude::*;
use rafter::widgets::TreeNode;
use tuidom::Color;
use tuidom::Element;

use super::types::{QuestionnaireSummary, ValidationReport};
use super::util::short_id;

impl TreeItem for QuestionnaireSummary {
    type Key = String;

    fn key(&self) -> String {
        self.id.clone()
    }

    fn render(&self) -> Element {
        let state_label = self.state_label();
        let state_color = if self.is_active() { "success" } else { "muted" };
        let code = self.code.clone().unwrap_or_else(|| "no code".to_string());
        let questionnaire_type = self
            .questionnaire_type
            .map(|value| value.to_string())
            .unwrap_or_else(|| "type ?".to_string());
        let statuscode = self
            .statuscode
            .map(|value| value.to_string())
            .unwrap_or_else(|| "status ?".to_string());
        let id = self.short_id();

        element! {
            row (gap: 1) {
                text (content: {self.name.clone()}) style (fg: primary)
                text (content: {format!("[{}]", state_label)}) style (fg: {Color::var(state_color)})
                text (content: {code}) style (fg: muted)
                text (content: {format!("type {}", questionnaire_type)}) style (fg: muted)
                text (content: {format!("status {}", statuscode)}) style (fg: muted)
                text (content: {id}) style (fg: muted)
            }
        }
    }
}

#[derive(Clone, Debug)]
pub(super) enum ValidationTreeNode {
    Root {
        name: String,
        finding_count: usize,
        record_count: usize,
    },
    Entity {
        entity: String,
        finding_count: usize,
        record_count: usize,
    },
    Record {
        entity: String,
        id: String,
        name: String,
        finding_count: usize,
    },
    Field {
        entity: String,
        id: String,
        field: String,
        value: Option<i32>,
        accepted_values: Vec<i32>,
        valid: bool,
    },
    FieldValue {
        entity: String,
        id: String,
        field: String,
        value: Option<i32>,
    },
    FieldOptions {
        entity: String,
        id: String,
        field: String,
    },
    FieldOption {
        entity: String,
        id: String,
        field: String,
        value: i32,
    },
    ConditionSummary {
        entity: String,
        id: String,
        mode: Option<String>,
        question_id: Option<String>,
        operator: Option<String>,
        value: Option<String>,
        parameter_type: Option<String>,
        valid: bool,
    },
    ConditionTargets {
        entity: String,
        id: String,
    },
    ConditionTargetQuestion {
        entity: String,
        id: String,
        question_id: String,
        visible: bool,
        required: bool,
        valid: bool,
    },
    ConditionActions {
        entity: String,
        id: String,
    },
    ConditionAction {
        entity: String,
        id: String,
        action_id: String,
        name: String,
        visible: Option<bool>,
        required: Option<bool>,
        valid: bool,
    },
}

impl TreeItem for ValidationTreeNode {
    type Key = String;

    fn key(&self) -> String {
        match self {
            Self::Root { .. } => "root".to_string(),
            Self::Entity { entity, .. } => format!("entity-{}", entity),
            Self::Record { entity, id, .. } => format!("record-{}-{}", entity, id),
            Self::Field {
                entity,
                id,
                field,
                value,
                ..
            } => format!(
                "field-{}-{}-{}-{}",
                entity,
                id,
                field,
                value
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".to_string())
            ),
            Self::FieldValue {
                entity,
                id,
                field,
                value,
            } => format!(
                "field-value-{}-{}-{}-{}",
                entity,
                id,
                field,
                value
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".to_string())
            ),
            Self::FieldOptions { entity, id, field } => {
                format!("field-options-{}-{}-{}", entity, id, field)
            }
            Self::FieldOption {
                entity,
                id,
                field,
                value,
            } => format!("field-option-{}-{}-{}-{}", entity, id, field, value),
            Self::ConditionSummary { entity, id, .. } => {
                format!("condition-summary-{}-{}", entity, id)
            }
            Self::ConditionTargets { entity, id } => {
                format!("condition-targets-{}-{}", entity, id)
            }
            Self::ConditionTargetQuestion {
                entity,
                id,
                question_id,
                ..
            } => format!("condition-target-{}-{}-{}", entity, id, question_id),
            Self::ConditionActions { entity, id } => {
                format!("condition-actions-{}-{}", entity, id)
            }
            Self::ConditionAction {
                entity,
                id,
                action_id,
                ..
            } => format!("condition-action-{}-{}-{}", entity, id, action_id),
        }
    }

    fn render(&self) -> Element {
        match self {
            Self::Root {
                name,
                finding_count,
                record_count,
            } => {
                let status = validation_status(*finding_count);
                let color = validation_color(*finding_count);
                element! {
                    row (gap: 1) {
                        text (content: {name.clone()}) style (bold, fg: interact)
                        text (content: {status}) style (fg: {Color::var(color)})
                        text (content: {format!("{} records", record_count)}) style (fg: muted)
                    }
                }
            }
            Self::Entity {
                entity,
                finding_count,
                record_count,
            } => {
                let status = validation_status(*finding_count);
                let color = validation_color(*finding_count);
                element! {
                    row (gap: 1) {
                        text (content: {entity.clone()}) style (fg: primary)
                        text (content: {status}) style (fg: {Color::var(color)})
                        text (content: {format!("{} records", record_count)}) style (fg: muted)
                    }
                }
            }
            Self::Record {
                id,
                name,
                finding_count,
                ..
            } => {
                let status = validation_status(*finding_count);
                let color = validation_color(*finding_count);
                element! {
                    row (gap: 1) {
                        text (content: {name.clone()}) style (fg: primary)
                        text (content: {status}) style (fg: {Color::var(color)})
                        text (content: {short_id(id)}) style (fg: muted)
                    }
                }
            }
            Self::Field {
                field,
                value,
                valid,
                ..
            } => {
                let has_value = value.is_some();
                let status = if *valid { "ok" } else { "error" };
                let color = if *valid { "success" } else { "error" };
                let value_text = value
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "not set".to_string());
                element! {
                    row (gap: 1) {
                        text (content: {field.clone()}) style (fg: primary)
                        text (content: {value_text}) style (fg: muted)
                        if has_value {
                            text (content: {status}) style (fg: {Color::var(color)})
                        }
                    }
                }
            }
            Self::FieldValue { value, .. } => {
                let value_text = value
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "not set".to_string());
                element! {
                    row (gap: 1) {
                        text (content: "value") style (fg: muted)
                        text (content: {value_text}) style (fg: primary)
                    }
                }
            }
            Self::FieldOptions { .. } => {
                element! {
                    row (gap: 1) {
                        text (content: "options") style (fg: muted)
                    }
                }
            }
            Self::FieldOption { value, .. } => {
                element! {
                    row (gap: 1) {
                        text (content: "option") style (fg: muted)
                        text (content: {value.to_string()}) style (fg: primary)
                    }
                }
            }
            Self::ConditionSummary {
                mode,
                question_id,
                operator,
                value,
                parameter_type,
                valid,
                ..
            } => {
                let status = if *valid { "ok" } else { "error" };
                let color = if *valid { "success" } else { "error" };
                element! {
                    row (gap: 1) {
                        text (content: "condition") style (fg: muted)
                        text (content: {mode.clone().unwrap_or_else(|| "?".to_string())}) style (fg: primary)
                        text (content: {question_id.clone().unwrap_or_else(|| "unknown question".to_string())}) style (fg: muted)
                        text (content: {operator.clone().unwrap_or_else(|| "?".to_string())}) style (fg: muted)
                        text (content: {value.clone().unwrap_or_else(|| "not set".to_string())}) style (fg: muted)
                        if let Some(parameter_type) = parameter_type {
                            text (content: {format!("parameter {}", parameter_type)}) style (fg: muted)
                        }
                        text (content: {status}) style (fg: {Color::var(color)})
                    }
                }
            }
            Self::ConditionTargets { .. } => {
                element! {
                    row (gap: 1) {
                        text (content: "targets") style (fg: muted)
                    }
                }
            }
            Self::ConditionTargetQuestion {
                question_id,
                visible,
                required,
                valid,
                ..
            } => {
                let status = if *valid { "ok" } else { "error" };
                let color = if *valid { "success" } else { "error" };
                element! {
                    row (gap: 1) {
                        text (content: "question") style (fg: muted)
                        text (content: {question_id.clone()}) style (fg: primary)
                        text (content: {format!("visible {}", visible)}) style (fg: muted)
                        text (content: {format!("required {}", required)}) style (fg: muted)
                        text (content: {status}) style (fg: {Color::var(color)})
                    }
                }
            }
            Self::ConditionActions { .. } => {
                element! {
                    row (gap: 1) {
                        text (content: "actions") style (fg: muted)
                    }
                }
            }
            Self::ConditionAction {
                action_id,
                name,
                visible,
                required,
                valid,
                ..
            } => {
                let status = if *valid { "ok" } else { "error" };
                let color = if *valid { "success" } else { "error" };
                element! {
                    row (gap: 1) {
                        text (content: {name.clone()}) style (fg: primary)
                        text (content: {short_id(action_id)}) style (fg: muted)
                        text (content: {visible.map(|v| format!("visible {}", v)).unwrap_or_else(|| "visible ?".to_string())}) style (fg: muted)
                        text (content: {required.map(|v| format!("required {}", v)).unwrap_or_else(|| "required ?".to_string())}) style (fg: muted)
                        text (content: {status}) style (fg: {Color::var(color)})
                    }
                }
            }
        }
    }
}

pub(super) fn build_validation_tree(
    report: &ValidationReport,
) -> Vec<TreeNode<ValidationTreeNode>> {
    let children = report
        .entities
        .iter()
        .map(|entity| {
            let records = entity
                .records
                .iter()
                .map(|record| {
                    let mut children = record
                        .fields
                        .iter()
                        .map(|field| {
                            TreeNode::branch(
                                ValidationTreeNode::Field {
                                    entity: record.entity.clone(),
                                    id: record.record_id.clone(),
                                    field: field.field.clone(),
                                    value: field.value,
                                    accepted_values: field.accepted_values.clone(),
                                    valid: field.valid,
                                },
                                vec![
                                    TreeNode::leaf(ValidationTreeNode::FieldValue {
                                        entity: record.entity.clone(),
                                        id: record.record_id.clone(),
                                        field: field.field.clone(),
                                        value: field.value,
                                    }),
                                    TreeNode::branch(
                                        ValidationTreeNode::FieldOptions {
                                            entity: record.entity.clone(),
                                            id: record.record_id.clone(),
                                            field: field.field.clone(),
                                        },
                                        field
                                            .accepted_values
                                            .iter()
                                            .map(|value| {
                                                TreeNode::leaf(ValidationTreeNode::FieldOption {
                                                    entity: record.entity.clone(),
                                                    id: record.record_id.clone(),
                                                    field: field.field.clone(),
                                                    value: *value,
                                                })
                                            })
                                            .collect::<Vec<_>>(),
                                    ),
                                ],
                            )
                        })
                        .collect::<Vec<_>>();
                    if let Some(condition) = &record.condition {
                        children.push(TreeNode::branch(
                            ValidationTreeNode::ConditionSummary {
                                entity: record.entity.clone(),
                                id: record.record_id.clone(),
                                mode: condition.mode.clone(),
                                question_id: condition.trigger_question_id.clone(),
                                operator: condition.operator.clone(),
                                value: condition.value.clone(),
                                parameter_type: condition.parameter_type.clone(),
                                valid: condition.valid,
                            },
                            vec![
                                TreeNode::branch(
                                    ValidationTreeNode::ConditionTargets {
                                        entity: record.entity.clone(),
                                        id: record.record_id.clone(),
                                    },
                                    condition
                                        .questions
                                        .iter()
                                        .map(|question| {
                                            TreeNode::leaf(
                                                ValidationTreeNode::ConditionTargetQuestion {
                                                    entity: record.entity.clone(),
                                                    id: record.record_id.clone(),
                                                    question_id: question.question_id.clone(),
                                                    visible: question.visible,
                                                    required: question.required,
                                                    valid: question.valid,
                                                },
                                            )
                                        })
                                        .collect::<Vec<_>>(),
                                ),
                                TreeNode::branch(
                                    ValidationTreeNode::ConditionActions {
                                        entity: record.entity.clone(),
                                        id: record.record_id.clone(),
                                    },
                                    condition
                                        .actions
                                        .iter()
                                        .map(|action| {
                                            TreeNode::leaf(ValidationTreeNode::ConditionAction {
                                                entity: record.entity.clone(),
                                                id: record.record_id.clone(),
                                                action_id: action.id.clone(),
                                                name: action.name.clone(),
                                                visible: action.visible,
                                                required: action.required,
                                                valid: action.valid,
                                            })
                                        })
                                        .collect::<Vec<_>>(),
                                ),
                            ],
                        ));
                    }
                    let finding_count = record.finding_count();
                    if children.is_empty() {
                        TreeNode::leaf(ValidationTreeNode::Record {
                            entity: record.entity.clone(),
                            id: record.record_id.clone(),
                            name: record.name.clone(),
                            finding_count,
                        })
                    } else {
                        TreeNode::branch(
                            ValidationTreeNode::Record {
                                entity: record.entity.clone(),
                                id: record.record_id.clone(),
                                name: record.name.clone(),
                                finding_count,
                            },
                            children,
                        )
                    }
                })
                .collect::<Vec<_>>();
            TreeNode::branch(
                ValidationTreeNode::Entity {
                    entity: entity.entity.clone(),
                    finding_count: entity.findings_count,
                    record_count: entity.record_count,
                },
                records,
            )
        })
        .collect::<Vec<_>>();

    vec![TreeNode::branch(
        ValidationTreeNode::Root {
            name: report.questionnaire.name.clone(),
            finding_count: report.finding_count,
            record_count: report.record_count,
        },
        children,
    )]
}

pub(super) fn validation_status(finding_count: usize) -> &'static str {
    if finding_count == 0 { "ok" } else { "error" }
}

pub(super) fn validation_color(finding_count: usize) -> &'static str {
    if finding_count == 0 {
        "success"
    } else {
        "error"
    }
}
