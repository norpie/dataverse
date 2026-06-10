use std::collections::HashMap;

#[derive(Clone, Debug)]
pub enum FieldKind {
    Direct,
    Lookup {
        target_entity: &'static str,
    },
    Date,
    Time,
    Picklist(&'static [(&'static str, i32)]),
    Boolean {
        true_value: &'static str,
        false_value: &'static str,
    },
}

#[derive(Clone, Debug)]
pub struct FieldMapping {
    pub column: &'static str,
    pub field: &'static str,
    pub kind: FieldKind,
    pub required: bool,
}

pub const ENTITY_DEADLINE: &str = "nrq_deadline";
pub const ENTITY_SUPPORT: &str = "nrq_support";
pub const ENTITY_CATEGORY: &str = "nrq_category";
pub const ENTITY_SUBCATEGORY: &str = "nrq_subcategory";
pub const ENTITY_FLEMISHSHARE: &str = "nrq_flemishshare";
pub const ENTITY_DEADLINE_SUPPORT: &str = "nrq_deadlinesupport";

pub const REL_CATEGORY: &str = "nrq_Deadline_nrq_Category_nrq_Category";
pub const REL_SUBCATEGORY: &str = "nrq_Deadline_nrq_Subcategory_nrq_Subcategory";
pub const REL_FLEMISHSHARE: &str = "nrq_Deadline_nrq_FlemishShare_nrq_Flemish";

pub const SUPPORT_TYPE_OPTIONS: &[(&str, i32)] = &[
    ("Automatische steun", 875810000),
    ("Automatic Support", 875810000),
    ("Selectieve steun", 875810001),
    ("Selective Support", 875810001),
];

pub const LOOKUP_ENTITIES: &[&str] = &[
    "nrq_domain",
    "nrq_fund",
    "nrq_commission",
    "systemuser",
    "nrq_support",
    "nrq_category",
    "nrq_subcategory",
    "nrq_flemishshare",
    "nrq_boardofdirectorsmeeting",
    "nrq_type",
];

pub const FIELD_MAPPINGS: &[FieldMapping] = &[
    FieldMapping {
        column: "Domein*",
        field: "nrq_DomainId",
        kind: FieldKind::Lookup {
            target_entity: "nrq_domain",
        },
        required: false,
    },
    FieldMapping {
        column: "Pillar",
        field: "nrq_DomainId",
        kind: FieldKind::Lookup {
            target_entity: "nrq_domain",
        },
        required: false,
    },
    FieldMapping {
        column: "Fonds*",
        field: "nrq_FundId",
        kind: FieldKind::Lookup {
            target_entity: "nrq_fund",
        },
        required: false,
    },
    FieldMapping {
        column: "Deadline*",
        field: "nrq_deadlinename",
        kind: FieldKind::Direct,
        required: false,
    },
    FieldMapping {
        column: "Projectbeheerder",
        field: "nrq_ProjectManagerId",
        kind: FieldKind::Lookup {
            target_entity: "systemuser",
        },
        required: false,
    },
    FieldMapping {
        column: "Info",
        field: "nrq_description",
        kind: FieldKind::Direct,
        required: false,
    },
    FieldMapping {
        column: "Datum Deadline",
        field: "nrq_deadlinedate",
        kind: FieldKind::Date,
        required: false,
    },
    FieldMapping {
        column: "Uur",
        field: "nrq_deadlinedate",
        kind: FieldKind::Time,
        required: false,
    },
    FieldMapping {
        column: "Commissie",
        field: "nrq_CommissionId",
        kind: FieldKind::Lookup {
            target_entity: "nrq_commission",
        },
        required: false,
    },
    FieldMapping {
        column: "Raad van Bestuur",
        field: "nrq_BoardOfDirectorsMeetingId",
        kind: FieldKind::Lookup {
            target_entity: "nrq_boardofdirectorsmeeting",
        },
        required: false,
    },
    FieldMapping {
        column: "Type",
        field: "nrq_TypeID",
        kind: FieldKind::Lookup {
            target_entity: "nrq_type",
        },
        required: false,
    },
    FieldMapping {
        column: "Datum Commissievergadering",
        field: "nrq_committeemeetingdate",
        kind: FieldKind::Date,
        required: false,
    },
    FieldMapping {
        column: "Uur Commissie",
        field: "nrq_committeemeetingdate",
        kind: FieldKind::Time,
        required: false,
    },
    FieldMapping {
        column: "Online of Fysiek",
        field: "nrq_committeemeetinginperson",
        kind: FieldKind::Boolean {
            true_value: "Fysiek",
            false_value: "Online",
        },
        required: false,
    },
    FieldMapping {
        column: "Support Type",
        field: "nrq_supporttypeoptionset",
        kind: FieldKind::Picklist(SUPPORT_TYPE_OPTIONS),
        required: true,
    },
];

pub fn constant_fields() -> HashMap<&'static str, dataverse_lib::model::Value> {
    HashMap::from([
        ("nrq_vafvalidated", true.into()),
        ("nrq_publishdeadlineonvafbe", false.into()),
        ("nrq_canbepublished", true.into()),
    ])
}
