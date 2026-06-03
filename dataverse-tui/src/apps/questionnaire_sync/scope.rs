/// Declarative questionnaire sync scope.
///
/// This keeps the questionnaire-related field names in one in-code table so
/// fetch, diff, and queue planning can all consume the same source of truth.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestionnaireFieldKind {
    Value,
    Lookup { target_entity: &'static str },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestionnaireFieldSpec {
    /// Field name returned by fetch.
    pub source_name: &'static str,
    /// Canonical field name used for updates and comparisons.
    pub field_name: &'static str,
    pub kind: QuestionnaireFieldKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestionnaireEntitySpec {
    pub logical_name: &'static str,
    pub primary_key: &'static str,
    pub fields: &'static [QuestionnaireFieldSpec],
    pub state_fields: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestionnaireRelationSpec {
    pub parent_entity: &'static str,
    pub related_entity: &'static str,
    pub relationship_name: &'static str,
}

macro_rules! value_field {
    ($name:expr) => {
        QuestionnaireFieldSpec {
            source_name: $name,
            field_name: $name,
            kind: QuestionnaireFieldKind::Value,
        }
    };
}

macro_rules! lookup_field {
    ($source:expr, $field:expr, $target:expr) => {
        QuestionnaireFieldSpec {
            source_name: $source,
            field_name: $field,
            kind: QuestionnaireFieldKind::Lookup {
                target_entity: $target,
            },
        }
    };
}

macro_rules! state_fields {
    () => {
        &["statecode", "statuscode"]
    };
}

pub const QUESTIONNAIRE_ENTITIES: &[QuestionnaireEntitySpec] = &[
    QuestionnaireEntitySpec {
        logical_name: "nrq_questionnaire",
        primary_key: "nrq_questionnaireid",
        fields: &[
            value_field!("nrq_name"),
            value_field!("nrq_code"),
            value_field!("nrq_type"),
            value_field!("nrq_publishdate"),
            value_field!("nrq_copypostfix"),
            value_field!("nrq_pullquestionstrigger"),
            value_field!("nrq_expectsdeliverables"),
            value_field!("nrq_publishdeliverablestemp"),
            lookup_field!("nrq_deadline", "nrq_deadline", "nrq_deadline"),
            lookup_field!("nrq_domain", "nrq_domain", "nrq_domain"),
        ],
        state_fields: state_fields!(),
    },
    QuestionnaireEntitySpec {
        logical_name: "nrq_questionnairepage",
        primary_key: "nrq_questionnairepageid",
        fields: &[
            value_field!("nrq_name"),
            value_field!("nrq_description"),
            value_field!("nrq_pagecode"),
            value_field!("nrq_isdeliverable"),
            value_field!("nrq_schijf"),
            lookup_field!(
                "nrq_relatedquestionnaire",
                "nrq_relatedquestionnaire",
                "nrq_questionnaire"
            ),
        ],
        state_fields: state_fields!(),
    },
    QuestionnaireEntitySpec {
        logical_name: "nrq_questionnairepageline",
        primary_key: "nrq_questionnairepagelineid",
        fields: &[
            value_field!("nrq_name"),
            value_field!("nrq_code"),
            value_field!("nrq_order"),
            value_field!("nrq_createquestions"),
            value_field!("nrq_requeststatus"),
            value_field!("nrq_submittedrequeststatus"),
            value_field!("nrq_editablerequeststatusses"),
            value_field!("nrq_visibleinstatusses"),
            lookup_field!(
                "nrq_questionnaireid",
                "nrq_questionnaireid",
                "nrq_questionnaire"
            ),
            lookup_field!(
                "nrq_questionnairepageid",
                "nrq_questionnairepageid",
                "nrq_questionnairepage"
            ),
        ],
        state_fields: state_fields!(),
    },
    QuestionnaireEntitySpec {
        logical_name: "nrq_questiongroup",
        primary_key: "nrq_questiongroupid",
        fields: &[
            value_field!("nrq_name"),
            value_field!("nrq_code"),
            value_field!("nrq_description"),
            value_field!("nrq_enablemultipleentries"),
        ],
        state_fields: state_fields!(),
    },
    QuestionnaireEntitySpec {
        logical_name: "nrq_questiongroupline",
        primary_key: "nrq_questiongrouplineid",
        fields: &[
            value_field!("nrq_name"),
            value_field!("nrq_code"),
            value_field!("nrq_order"),
            lookup_field!(
                "nrq_questiongroupid",
                "nrq_questiongroupid",
                "nrq_questiongroup"
            ),
            lookup_field!(
                "nrq_questionnairepageid",
                "nrq_questionnairepageid",
                "nrq_questionnairepage"
            ),
        ],
        state_fields: state_fields!(),
    },
    QuestionnaireEntitySpec {
        logical_name: "nrq_question",
        primary_key: "nrq_questionid",
        fields: &[
            value_field!("nrq_name"),
            value_field!("nrq_questiontext"),
            value_field!("nrq_questiontype"),
            value_field!("nrq_required"),
            value_field!("nrq_ismultiselect"),
            value_field!("nrq_lookuptype"),
            value_field!("nrq_options"),
            value_field!("nrq_publicorprivatefile"),
            value_field!("nrq_regex"),
            value_field!("nrq_regexerrormessage"),
            value_field!("nrq_showasradio"),
            value_field!("nrq_targetentity"),
            value_field!("nrq_targetentityfield"),
            value_field!("nrq_targetfield"),
            value_field!("nrq_tooltip"),
            value_field!("nrq_uploadfolder"),
            value_field!("nrq_versionnumber"),
            value_field!("nrq_maxfiles"),
            lookup_field!(
                "nrq_questiongroupid",
                "nrq_questiongroupid",
                "nrq_questiongroup"
            ),
            lookup_field!(
                "nrq_questionnaireid",
                "nrq_questionnaireid",
                "nrq_questionnaire"
            ),
            lookup_field!("nrq_questiontagid", "nrq_questiontagid", "nrq_questiontag"),
            lookup_field!(
                "nrq_questiontemplateid",
                "nrq_questiontemplateid",
                "nrq_questiontemplate"
            ),
            lookup_field!("nrq_contactrole", "nrq_contactrole", "nrq_role"),
            lookup_field!(
                "nrq_questionpage",
                "nrq_questionpage",
                "nrq_questionnairepage"
            ),
        ],
        state_fields: state_fields!(),
    },
    QuestionnaireEntitySpec {
        logical_name: "nrq_questiontemplateline",
        primary_key: "nrq_questiontemplatelineid",
        fields: &[
            value_field!("nrq_name"),
            value_field!("nrq_order"),
            value_field!("nrq_code"),
            value_field!("nrq_size"),
            lookup_field!(
                "nrq_questiontemplateid",
                "nrq_questiontemplateid",
                "nrq_questiontemplate"
            ),
            lookup_field!(
                "nrq_questiongroupid",
                "nrq_questiongroupid",
                "nrq_questiongroup"
            ),
        ],
        state_fields: state_fields!(),
    },
    QuestionnaireEntitySpec {
        logical_name: "nrq_questioncondition",
        primary_key: "nrq_questionconditionid",
        fields: &[
            value_field!("nrq_name"),
            value_field!("nrq_conditionjson"),
            value_field!("nrq_logicaloperator"),
            value_field!("nrq_value"),
            value_field!("nrq_conditiongroup"),
            value_field!("nrq_conditiontype"),
            value_field!("nrq_parametertype"),
            value_field!("nrq_parametervalue"),
            lookup_field!("nrq_questionid", "nrq_questionid", "nrq_question"),
            lookup_field!(
                "nrq_questionnaireid",
                "nrq_questionnaireid",
                "nrq_questionnaire"
            ),
        ],
        state_fields: state_fields!(),
    },
    QuestionnaireEntitySpec {
        logical_name: "nrq_questionconditionaction",
        primary_key: "nrq_questionconditionactionid",
        fields: &[
            value_field!("nrq_name"),
            value_field!("nrq_required"),
            value_field!("nrq_visible"),
            value_field!("nrq_affectsvisibility"),
            lookup_field!(
                "nrq_questionconditionid",
                "nrq_questionconditionid",
                "nrq_questioncondition"
            ),
            lookup_field!("nrq_questionid", "nrq_questionid", "nrq_question"),
        ],
        state_fields: state_fields!(),
    },
    QuestionnaireEntitySpec {
        logical_name: "nrq_questiontemplate",
        primary_key: "nrq_questiontemplateid",
        fields: &[
            value_field!("nrq_name"),
            value_field!("nrq_contracttext"),
            value_field!("nrq_damfolder"),
            value_field!("nrq_deliverablenamebackendview"),
            value_field!("nrq_deliverableteam"),
            value_field!("nrq_isdeliverable"),
            value_field!("nrq_ismultiselect"),
            value_field!("nrq_lookuptype"),
            value_field!("nrq_maxfiles"),
            value_field!("nrq_options"),
            value_field!("nrq_publicorprivatefile"),
            value_field!("nrq_questiontext"),
            value_field!("nrq_questiontype"),
            value_field!("nrq_regexerrormessage"),
            value_field!("nrq_regex"),
            value_field!("nrq_required"),
            value_field!("nrq_showasradio"),
            value_field!("nrq_spordamupload"),
            value_field!("nrq_targetentityfield"),
            value_field!("nrq_targetentity"),
            value_field!("nrq_targetfield"),
            lookup_field!(
                "nrq_templatetoreplace",
                "nrq_templatetoreplace",
                "nrq_questiontemplate"
            ),
            value_field!("nrq_tooltip"),
            value_field!("nrq_updatestrategy"),
            value_field!("nrq_uploadfolder"),
            value_field!("nrq_versionnumber"),
            lookup_field!("nrq_questiontagid", "nrq_questiontagid", "nrq_questiontag"),
            lookup_field!("nrq_replacedby", "nrq_replacedby", "nrq_questiontemplate"),
            lookup_field!(
                "nrq_betalingsschijflijn",
                "nrq_betalingsschijflijn",
                "nrq_betalingsschijflijn"
            ),
        ],
        state_fields: state_fields!(),
    },
    QuestionnaireEntitySpec {
        logical_name: "nrq_questiontag",
        primary_key: "nrq_questiontagid",
        fields: &[value_field!("nrq_name")],
        state_fields: state_fields!(),
    },
    QuestionnaireEntitySpec {
        logical_name: "nrq_role",
        primary_key: "nrq_roleid",
        fields: &[
            value_field!("nrq_name"),
            value_field!("nrq_addtoconnectionssummary"),
            value_field!("nrq_addtorequestconnectionfields"),
            value_field!("nrq_connectionssummaryroleoverride"),
            value_field!("nrq_legacy"),
            value_field!("nrq_portalmails"),
            value_field!("nrq_requestconnectionfield"),
        ],
        state_fields: state_fields!(),
    },
    QuestionnaireEntitySpec {
        logical_name: "nrq_pdfreport",
        primary_key: "nrq_pdfreportid",
        fields: &[
            value_field!("nrq_name"),
            value_field!("nrq_filenameprefix"),
            value_field!("nrq_folderlocation"),
            value_field!("nrq_triggeronstatus"),
            value_field!("nrq_editableinstatus"),
        ],
        state_fields: state_fields!(),
    },
    QuestionnaireEntitySpec {
        logical_name: "nrq_domain",
        primary_key: "nrq_domainid",
        fields: &[
            value_field!("nrq_name"),
            value_field!("nrq_listorder"),
            value_field!("nrq_subsidiewijzerlabel"),
        ],
        state_fields: state_fields!(),
    },
    QuestionnaireEntitySpec {
        logical_name: "nrq_type",
        primary_key: "nrq_typeid",
        fields: &[
            value_field!("nrq_name"),
            value_field!("nrq_abbreviation"),
            value_field!("nrq_listorder"),
            value_field!("nrq_subsidiewijzerlabeltype"),
            value_field!("nrq_subtype"),
            lookup_field!("nrq_fund", "nrq_fund", "nrq_fund"),
        ],
        state_fields: state_fields!(),
    },
    QuestionnaireEntitySpec {
        logical_name: "nrq_fund",
        primary_key: "nrq_fundid",
        fields: &[
            value_field!("nrq_name"),
            value_field!("nrq_abbreviation"),
            value_field!("nrq_listorder"),
            value_field!("nrq_subsidiewijzerlabeltype"),
            value_field!("nrq_subtype"),
            lookup_field!("nrq_fund", "nrq_fund", "nrq_fund"),
        ],
        state_fields: state_fields!(),
    },
    QuestionnaireEntitySpec {
        logical_name: "nrq_support",
        primary_key: "nrq_supportid",
        fields: &[
            value_field!("nrq_name"),
            value_field!("nrq_abbreviation"),
            value_field!("nrq_listorder"),
            value_field!("nrq_expectsquestionnaire"),
            value_field!("nrq_subsidiewijzerlabelsupport"),
        ],
        state_fields: state_fields!(),
    },
    QuestionnaireEntitySpec {
        logical_name: "nrq_category",
        primary_key: "nrq_categoryid",
        fields: &[
            value_field!("nrq_name"),
            value_field!("nrq_abbreviation"),
            value_field!("nrq_code"),
            value_field!("nrq_description"),
            lookup_field!("nrq_category", "nrq_category", "nrq_category"),
            lookup_field!("nrq_fund", "nrq_fund", "nrq_fund"),
            lookup_field!("nrq_support", "nrq_support", "nrq_support"),
            lookup_field!("nrq_flemishshare", "nrq_flemishshare", "nrq_flemishshare"),
        ],
        state_fields: state_fields!(),
    },
    QuestionnaireEntitySpec {
        logical_name: "nrq_subcategory",
        primary_key: "nrq_subcategoryid",
        fields: &[
            value_field!("nrq_name"),
            value_field!("nrq_abbreviation"),
            value_field!("nrq_listorder"),
            value_field!("nrq_subsidiewijzerlabel"),
        ],
        state_fields: state_fields!(),
    },
    QuestionnaireEntitySpec {
        logical_name: "nrq_flemishshare",
        primary_key: "nrq_flemishshareid",
        fields: &[
            value_field!("nrq_name"),
            value_field!("nrq_listorder"),
            value_field!("nrq_subsidiewijzerlabelflemishshare"),
        ],
        state_fields: state_fields!(),
    },
    QuestionnaireEntitySpec {
        logical_name: "nrq_betalingsschijf",
        primary_key: "nrq_betalingsschijfid",
        fields: &[
            value_field!("nrq_name"),
            value_field!("nrq_2026"),
            value_field!("nrq_einddatum"),
            value_field!("nrq_extrainfo"),
            lookup_field!("nrq_fundid", "nrq_fundid", "nrq_fund"),
            value_field!("nrq_startdatum"),
            lookup_field!("nrq_supportid", "nrq_supportid", "nrq_support"),
        ],
        state_fields: state_fields!(),
    },
    QuestionnaireEntitySpec {
        logical_name: "nrq_betalingsschijflijn",
        primary_key: "nrq_betalingsschijflijnid",
        fields: &[
            value_field!("nrq_name"),
            lookup_field!(
                "nrq_betalingsschijf",
                "nrq_betalingsschijf",
                "nrq_betalingsschijf"
            ),
            value_field!("nrq_contracttekst"),
            value_field!("nrq_percentage"),
            value_field!("nrq_volgorde"),
        ],
        state_fields: state_fields!(),
    },
    QuestionnaireEntitySpec {
        logical_name: "nrq_grootboekrekening",
        primary_key: "nrq_grootboekrekeningid",
        fields: &[
            value_field!("nrq_name"),
            value_field!("nrq_code"),
            value_field!("nrq_description"),
            lookup_field!("nrq_domain", "nrq_domain", "nrq_domain"),
            lookup_field!("nrq_fund", "nrq_fund", "nrq_fund"),
            lookup_field!("nrq_support", "nrq_support", "nrq_support"),
            lookup_field!("nrq_category", "nrq_category", "nrq_category"),
        ],
        state_fields: state_fields!(),
    },
    QuestionnaireEntitySpec {
        logical_name: "nrq_kostenplaats",
        primary_key: "nrq_kostenplaatsid",
        fields: &[
            value_field!("nrq_name"),
            value_field!("nrq_code"),
            value_field!("nrq_description"),
            lookup_field!("nrq_domain", "nrq_domain", "nrq_domain"),
            lookup_field!("nrq_fund", "nrq_fund", "nrq_fund"),
        ],
        state_fields: state_fields!(),
    },
];

pub const QUESTIONNAIRE_RELATIONS: &[QuestionnaireRelationSpec] = &[
    QuestionnaireRelationSpec {
        parent_entity: "nrq_questionnaire",
        related_entity: "nrq_category",
        relationship_name: "nrq_questionnaire_nrq_Category_nrq_Category",
    },
    QuestionnaireRelationSpec {
        parent_entity: "nrq_questionnaire",
        related_entity: "nrq_domain",
        relationship_name: "nrq_questionnaire_nrq_Domain_nrq_Domain",
    },
    QuestionnaireRelationSpec {
        parent_entity: "nrq_questionnaire",
        related_entity: "nrq_fund",
        relationship_name: "nrq_questionnaire_nrq_Fund_nrq_Fund",
    },
    QuestionnaireRelationSpec {
        parent_entity: "nrq_questionnaire",
        related_entity: "nrq_support",
        relationship_name: "nrq_questionnaire_nrq_Support_nrq_Support",
    },
    QuestionnaireRelationSpec {
        parent_entity: "nrq_questionnaire",
        related_entity: "nrq_type",
        relationship_name: "nrq_questionnaire_nrq_Type_nrq_Type",
    },
    QuestionnaireRelationSpec {
        parent_entity: "nrq_questionnaire",
        related_entity: "nrq_subcategory",
        relationship_name: "nrq_questionnaire_nrq_Subcategory_nrq_Subcategory",
    },
    QuestionnaireRelationSpec {
        parent_entity: "nrq_questionnaire",
        related_entity: "nrq_flemishshare",
        relationship_name: "nrq_questionnaire_nrq_FlemishShare_nrq_FlemishShare",
    },
    QuestionnaireRelationSpec {
        parent_entity: "nrq_questionnaire",
        related_entity: "nrq_pdfreport",
        relationship_name: "nrq_PdfReport_nrq_questionnaire_nrq_questionnaire",
    },
];
