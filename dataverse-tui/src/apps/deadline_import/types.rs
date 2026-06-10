use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{NaiveDate, NaiveTime};
use dataverse_lib::model::Record;
use dataverse_lib::model::Value;
use rafter::widgets::TableRow;
use tuidom::Element;
use uuid::Uuid;

#[derive(Clone, Debug, Default)]
pub struct ImportData {
    pub file_path: PathBuf,
    pub sheet_name: String,
    pub records: Vec<DeadlineRecord>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct DeadlineRecord {
    pub source_row: usize,
    pub id: Uuid,
    pub id_from_excel: Option<Uuid>,
    pub mode: DeadlineMode,
    pub fields: DeadlineFields,
    pub associations: DeadlineAssociations,
    pub existing: Option<ExistingDeadline>,
    pub warnings: Vec<String>,
    pub notes: Option<String>,
}

impl DeadlineRecord {
    pub fn action_label(&self) -> &'static str {
        match self.mode {
            DeadlineMode::Create => "Create",
            DeadlineMode::Update => "Update",
            DeadlineMode::Unchanged => "Unchanged",
            DeadlineMode::Error(_) => "Error",
        }
    }

    pub fn is_actionable(&self) -> bool {
        matches!(self.mode, DeadlineMode::Create | DeadlineMode::Update) && self.warnings.is_empty()
    }

    pub fn name(&self) -> &str {
        self.fields
            .direct
            .get("nrq_deadlinename")
            .map(String::as_str)
            .unwrap_or("<missing name>")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeadlineMode {
    Create,
    Update,
    Unchanged,
    Error(String),
}

#[derive(Clone, Debug, Default)]
pub struct DeadlineFields {
    pub direct: HashMap<String, String>,
    pub lookups: HashMap<String, LookupValue>,
    pub picklists: HashMap<String, i32>,
    pub booleans: HashMap<String, bool>,
    pub deadline_date: Option<NaiveDate>,
    pub deadline_time: Option<NaiveTime>,
    pub committee_date: Option<NaiveDate>,
    pub committee_time: Option<NaiveTime>,
}

#[derive(Clone, Debug)]
pub struct LookupValue {
    pub id: Uuid,
    pub target_entity: String,
    pub target_set: String,
    pub label: String,
}

#[derive(Clone, Debug, Default)]
pub struct DeadlineAssociations {
    pub support: HashMap<Uuid, String>,
    pub category: HashMap<Uuid, String>,
    pub subcategory: HashMap<Uuid, String>,
    pub flemishshare: HashMap<Uuid, String>,
}

#[derive(Clone, Debug)]
pub struct ExistingDeadline {
    pub id: Uuid,
    pub fields: HashMap<String, Value>,
    pub associations: ExistingAssociations,
}

impl ExistingDeadline {
    pub fn from_record(record: Record, associations: ExistingAssociations) -> Option<Self> {
        let id = record.id()?;
        Some(Self {
            id,
            fields: record.fields().clone(),
            associations,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct ExistingAssociations {
    pub support: HashMap<Uuid, ExistingJunctionRecord>,
    pub category: HashMap<Uuid, String>,
    pub subcategory: HashMap<Uuid, String>,
    pub flemishshare: HashMap<Uuid, String>,
}

#[derive(Clone, Debug)]
pub struct ExistingJunctionRecord {
    pub junction_id: Uuid,
    pub related_id: Uuid,
    pub name: String,
}

#[derive(Clone, Debug, Default)]
pub struct AssociationDiff {
    pub support_to_add: Vec<(Uuid, String)>,
    pub support_to_remove: Vec<ExistingJunctionRecord>,
    pub category_to_add: Vec<(Uuid, String)>,
    pub category_to_remove: Vec<(Uuid, String)>,
    pub subcategory_to_add: Vec<(Uuid, String)>,
    pub subcategory_to_remove: Vec<(Uuid, String)>,
    pub flemishshare_to_add: Vec<(Uuid, String)>,
    pub flemishshare_to_remove: Vec<(Uuid, String)>,
}

impl AssociationDiff {
    pub fn has_changes(&self) -> bool {
        !self.support_to_add.is_empty()
            || !self.support_to_remove.is_empty()
            || !self.category_to_add.is_empty()
            || !self.category_to_remove.is_empty()
            || !self.subcategory_to_add.is_empty()
            || !self.subcategory_to_remove.is_empty()
            || !self.flemishshare_to_add.is_empty()
            || !self.flemishshare_to_remove.is_empty()
    }
}

#[derive(Clone, Debug)]
pub struct LookupCache {
    pub records: HashMap<String, Vec<Record>>,
    pub entity_sets: HashMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct ImportContext {
    pub cache: LookupCache,
    pub existing_deadlines: HashMap<Uuid, ExistingDeadline>,
}

#[derive(Clone, Debug)]
pub struct DeadlineTableRow {
    pub key: usize,
    pub row: usize,
    pub mode: String,
    pub id: String,
    pub name: String,
    pub warnings: usize,
}

impl TableRow for DeadlineTableRow {
    type Key = usize;

    fn key(&self) -> Self::Key {
        self.key
    }

    fn cell(&self, column_id: &str) -> Element {
        let text = match column_id {
            "row" => self.row.to_string(),
            "mode" => self.mode.clone(),
            "id" => self.id.clone(),
            "name" => self.name.clone(),
            "warnings" => {
                if self.warnings == 0 {
                    String::new()
                } else {
                    self.warnings.to_string()
                }
            }
            _ => String::new(),
        };
        Element::text(text)
    }
}
