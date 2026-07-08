//! Audit table row types and flattening of change history into rows.

use std::collections::BTreeSet;

use dataverse_lib::api::AuditDetailCollection;
use dataverse_lib::model::Record;
use rafter::widgets::TableRow;
use tuidom::Element;

use crate::formatting::format_value;

/// Column ids for the audit table.
pub const COL_TIMESTAMP: &str = "timestamp";
pub const COL_ACTION: &str = "action";
pub const COL_OPERATION: &str = "operation";
pub const COL_USER: &str = "user";
pub const COL_ATTRIBUTE: &str = "attribute";
pub const COL_OLD: &str = "old";
pub const COL_NEW: &str = "new";

/// A single attribute-change row in the audit table.
#[derive(Clone, Debug)]
pub struct AuditRow {
    key: String,
    timestamp: String,
    action: String,
    operation: String,
    user: String,
    attribute: String,
    old: String,
    new: String,
}

impl TableRow for AuditRow {
    type Key = String;

    fn key(&self) -> String {
        self.key.clone()
    }

    fn cell(&self, column_id: &str) -> Element {
        let text = match column_id {
            COL_TIMESTAMP => &self.timestamp,
            COL_ACTION => &self.action,
            COL_OPERATION => &self.operation,
            COL_USER => &self.user,
            COL_ATTRIBUTE => &self.attribute,
            COL_OLD => &self.old,
            COL_NEW => &self.new,
            _ => "",
        };
        Element::text(text)
    }
}

/// Raw string value of a field on a record (empty when absent).
fn raw(record: &Record, field: &str) -> String {
    record
        .get(field)
        .map(|v| format_value(v).raw)
        .unwrap_or_default()
}

/// Flatten a change-history collection into one row per changed attribute.
///
/// Only attribute-change details are included; relationship/share/role details
/// are skipped.
pub fn flatten_history(collection: &AuditDetailCollection) -> Vec<AuditRow> {
    let mut rows = Vec::new();

    for detail in &collection.audit_details {
        if !detail.is_attribute_change() {
            continue;
        }

        let audit = &detail.audit_record;
        let timestamp = raw(audit, "createdon");
        let action = raw(audit, "action");
        let operation = raw(audit, "operation");
        let user = raw(audit, "_userid_value");
        let audit_id = raw(audit, "auditid");

        // Union of the attributes present in the old and new value records.
        let mut attributes: BTreeSet<String> = BTreeSet::new();
        if let Some(old) = &detail.old_value {
            attributes.extend(old.fields().keys().cloned());
        }
        if let Some(new) = &detail.new_value {
            attributes.extend(new.fields().keys().cloned());
        }

        for attribute in attributes {
            let old = detail
                .old_value
                .as_ref()
                .map(|r| raw(r, &attribute))
                .unwrap_or_default();
            let new = detail
                .new_value
                .as_ref()
                .map(|r| raw(r, &attribute))
                .unwrap_or_default();

            rows.push(AuditRow {
                key: format!("{audit_id}:{attribute}"),
                timestamp: timestamp.clone(),
                action: action.clone(),
                operation: operation.clone(),
                user: user.clone(),
                attribute,
                old,
                new,
            });
        }
    }

    rows
}
