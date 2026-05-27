use std::collections::HashMap;

use dataverse_lib::model::Record;

use crate::apps::migration::comparison::MappingComparison;
use crate::apps::migration::execution::{EntityBatches, SubPhase};

#[derive(Clone, Default)]
pub struct QuestionnaireEntitySnapshot {
    pub entity: String,
    pub records: Vec<Record>,
}

impl QuestionnaireEntitySnapshot {
    pub fn record_count(&self) -> usize {
        self.records.len()
    }
}

#[derive(Clone, Default)]
pub struct QuestionnaireEnvironmentSnapshot {
    pub environment_id: i64,
    pub environment_name: String,
    pub entities: Vec<QuestionnaireEntitySnapshot>,
}

impl QuestionnaireEnvironmentSnapshot {
    pub fn total_records(&self) -> usize {
        self.entities.iter().map(QuestionnaireEntitySnapshot::record_count).sum()
    }

    pub fn entity(&self, logical_name: &str) -> Option<&QuestionnaireEntitySnapshot> {
        self.entities.iter().find(|entity| entity.entity == logical_name)
    }
}

/// Comparison output for the questionnaire sync scope.
///
/// This intentionally mirrors the migration app's compare → phase plan shape:
/// we keep the per-entity `MappingComparison`s, then group execution into a
/// fixed set of sub-phases.
#[derive(Clone, Default)]
pub struct QuestionnaireComparison {
    pub source: QuestionnaireEnvironmentSnapshot,
    pub target: QuestionnaireEnvironmentSnapshot,
    pub mappings: Vec<MappingComparison>,
}

impl QuestionnaireComparison {
    pub fn total_records(&self) -> usize {
        self.mappings
            .iter()
            .map(|mapping| mapping.records.len() + mapping.orphans.len())
            .sum()
    }
}

/// Fixed execution buckets for questionnaire sync.
#[derive(Clone, Default)]
pub struct QuestionnaireExecutionPlan {
    pub create: Vec<EntityBatches>,
    pub activate: Vec<EntityBatches>,
    pub update: Vec<EntityBatches>,
    pub associate: Vec<EntityBatches>,
    pub disassociate: Vec<EntityBatches>,
    pub deactivate: Vec<EntityBatches>,
    pub delete: Vec<EntityBatches>,
}

impl QuestionnaireExecutionPlan {
    pub fn is_empty(&self) -> bool {
        self.total_operations() == 0
    }

    pub fn total_operations(&self) -> usize {
        [
            &self.create,
            &self.activate,
            &self.update,
            &self.associate,
            &self.disassociate,
            &self.deactivate,
            &self.delete,
        ]
        .into_iter()
        .flat_map(|batches| batches.iter())
        .map(|batches| batches.operation_count)
        .sum()
    }

    pub fn push(&mut self, sub_phase: SubPhase, batches: Vec<EntityBatches>) {
        let target = match sub_phase {
            SubPhase::Create => &mut self.create,
            SubPhase::Activate => &mut self.activate,
            SubPhase::Update => &mut self.update,
            SubPhase::Associate => &mut self.associate,
            SubPhase::Disassociate => &mut self.disassociate,
            SubPhase::Deactivate => &mut self.deactivate,
            SubPhase::Delete => &mut self.delete,
        };

        target.extend(batches);
    }

    pub fn batches_for(&self, sub_phase: SubPhase) -> &[EntityBatches] {
        match sub_phase {
            SubPhase::Create => &self.create,
            SubPhase::Activate => &self.activate,
            SubPhase::Update => &self.update,
            SubPhase::Associate => &self.associate,
            SubPhase::Disassociate => &self.disassociate,
            SubPhase::Deactivate => &self.deactivate,
            SubPhase::Delete => &self.delete,
        }
    }
}

pub type QuestionnaireEnvironmentMap = HashMap<i64, String>;
