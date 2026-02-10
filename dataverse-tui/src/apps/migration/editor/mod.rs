//! Migration editor app for editing a migration's phases and entity mappings.

mod child_operations;
mod config_operations;
mod detail_views;
mod entity_operations;
mod helpers;
mod insert_target;
mod item_operations;
mod phase_operations;
pub(crate) mod preview;
mod transform_operations;
mod tree;
mod tree_builder;
mod tree_types;
mod value_map_helpers;

use crate::apps::migration::types::CoalesceChain;
use crate::apps::migration::types::EntityMapping;
use crate::apps::migration::types::FieldMapping;
use crate::apps::migration::types::FindCondition;
use crate::apps::migration::types::FindMode;
use crate::apps::migration::types::MatchBranch;
use crate::apps::migration::types::MatchCondition;
use crate::apps::migration::types::Migration;
use crate::apps::migration::types::Mode;
use crate::apps::migration::types::Phase;
use crate::apps::migration::types::Transform;
use crate::apps::migration::types::TransformData;
use crate::apps::migration::types::Variable;
use dataverse_lib::DataverseClient;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::Table;
use rafter::widgets::TableState;
use rafter::widgets::Text;
use rafter::widgets::Tree;
use rafter::widgets::TreeState;

use crate::apps::migration::comparison::MappingComparison;
use crate::apps::migration::comparison::OperationTypeCounts;
use preview::PreviewRow;
use tree::MigrationTreeNode;

/// Page routing for the migration editor.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Editor,
    Preview,
}

/// Migration editor app.
#[app(name = "Migration Editor", on_blur = Continue, pages)]
pub struct MigrationEditor {
    /// The migration being edited.
    migration: Migration,
    /// Client for the source environment.
    source_client: DataverseClient,
    /// Client for the target environment.
    target_client: DataverseClient,
    /// Tree state for phases and entity mappings.
    tree_state: TreeState<MigrationTreeNode>,
    /// All phases (for tree building).
    phases: Vec<Phase>,
    /// All entity mappings (for tree building).
    entity_mappings: Vec<EntityMapping>,
    /// All variables (for tree building).
    variables: Vec<Variable>,
    /// All field mappings (for tree building).
    field_mappings: Vec<FieldMapping>,
    /// All transforms (for tree building).
    transforms: Vec<Transform>,
    /// All match branches (for tree building).
    match_branches: Vec<MatchBranch>,
    /// All coalesce chains (for tree building).
    coalesce_chains: Vec<CoalesceChain>,
    /// All find conditions (for tree building).
    find_conditions: Vec<FindCondition>,
    /// All match conditions (for tree building).
    match_conditions: Vec<MatchCondition>,
    // =========================================================================
    // Preview state
    // =========================================================================
    /// Phase name shown in the preview header.
    preview_phase_name: String,
    /// Comparison results per entity mapping.
    preview_results: Vec<MappingComparison>,
    /// Current entity index in the preview.
    preview_entity_index: usize,
    /// Table state for the preview record list.
    preview_table: TableState<PreviewRow>,
}

impl MigrationEditor {
    /// Create a new editor for the given migration and clients.
    pub fn new_editor(
        migration: Migration,
        source_client: DataverseClient,
        target_client: DataverseClient,
    ) -> Self {
        Self::new(
            migration,
            source_client,
            target_client,
            TreeState::default(),
            Vec::new(), // phases
            Vec::new(), // entity_mappings
            Vec::new(), // variables
            Vec::new(), // field_mappings
            Vec::new(), // transforms
            Vec::new(), // match_branches
            Vec::new(), // coalesce_chains
            Vec::new(), // find_conditions
            Vec::new(), // match_conditions
            String::new(),           // preview_phase_name
            Vec::new(),              // preview_results
            0,                       // preview_entity_index
            TableState::default(),   // preview_table
        )
    }
}

#[app_impl]
#[allow(clippy::match_single_binding)]
impl MigrationEditor {
    #[on_start]
    async fn on_start(&self, gx: &GlobalContext) {
        self.load_db_data(gx).await;
    }

    /// Reactive rebuild: auto-triggers whenever any of the 8 data Vec fields change.
    /// Fetches entity metadata, discovers navigation entities, builds the tree.
    #[watch]
    async fn rebuild(&self, gx: &GlobalContext) {
        use helpers::collect_navigation_paths;
        use helpers::discover_navigation_entities;
        use helpers::fetch_entity_field_types;
        use tree::FieldTypeCache;
        use tree_builder::build_tree_nodes;

        // 1. Read all dependencies (registers for change detection)
        let phases = self.phases.get();
        let entity_mappings = self.entity_mappings.get();
        let variables = self.variables.get();
        let field_mappings = self.field_mappings.get();
        let transforms = self.transforms.get();
        let match_branches = self.match_branches.get();
        let coalesce_chains = self.coalesce_chains.get();
        let find_conditions = self.find_conditions.get();
        let match_conditions = self.match_conditions.get();

        // 2. Collect unique entity names
        let mut source_entities: Vec<String> = Vec::new();
        let mut target_entities: Vec<String> = Vec::new();
        for em in entity_mappings.iter() {
            if !em.source_entity.is_empty() && !source_entities.contains(&em.source_entity) {
                source_entities.push(em.source_entity.clone());
            }
            if !em.target_entity.is_empty() && !target_entities.contains(&em.target_entity) {
                target_entities.push(em.target_entity.clone());
            }
        }

        // 3. Fetch metadata (DataverseClient has API-level cache with TTL)
        let source_client = self.source_client.get().clone();
        let target_client = self.target_client.get().clone();

        let mut source_field_types = if source_entities.is_empty() {
            FieldTypeCache::new()
        } else {
            log::debug!(
                "watch rebuild: fetching metadata for {} source entities",
                source_entities.len(),
            );
            let result: FieldTypeCache = gx
                .modal(crate::modals::LoadingModal::run(
                    "Loading source entity metadata...",
                    fetch_entity_field_types(source_client.clone(), source_entities),
                ))
                .await;
            result
        };

        // 4. Discover and fetch navigation entities (dotted copy paths + variable navigation)
        let nav_paths = collect_navigation_paths(&transforms, &entity_mappings, &variables);
        if !nav_paths.is_empty() {
            log::debug!(
                "watch rebuild: found {} navigation paths for entity scanning",
                nav_paths.len(),
            );
            loop {
                let nav_entities = discover_navigation_entities(&nav_paths, &source_field_types);
                if nav_entities.is_empty() {
                    break;
                }
                log::debug!(
                    "watch rebuild: fetching metadata for {} navigation entities: {:?}",
                    nav_entities.len(),
                    nav_entities,
                );
                let result: FieldTypeCache = gx
                    .modal(crate::modals::LoadingModal::run(
                        "Loading navigation entity metadata...",
                        fetch_entity_field_types(source_client.clone(), nav_entities),
                    ))
                    .await;
                let fetched_any = !result.is_empty();
                for (entity, fields) in result {
                    source_field_types.insert(entity, fields);
                }
                if !fetched_any {
                    break;
                }
            }
        }

        let target_field_types = if target_entities.is_empty() {
            FieldTypeCache::new()
        } else {
            log::debug!(
                "watch rebuild: fetching metadata for {} target entities",
                target_entities.len(),
            );
            let result: FieldTypeCache = gx
                .modal(crate::modals::LoadingModal::run(
                    "Loading target entity metadata...",
                    fetch_entity_field_types(target_client, target_entities),
                ))
                .await;
            result
        };

        // 5. Build tree — type tracking is embedded in tree nodes
        let nodes = build_tree_nodes(
            phases,
            entity_mappings,
            variables,
            field_mappings,
            transforms,
            match_branches,
            coalesce_chains,
            find_conditions,
            match_conditions,
            &source_field_types,
            &target_field_types,
        );

        // 6. Update tree
        self.tree_state.update(|s| {
            s.set_roots(nodes);
            s.expand_all();
        });
    }

    fn title(&self) -> String {
        format!("Edit: {}", self.migration.get().name)
    }

    // =========================================================================
    // Keybinds
    // =========================================================================

    #[keybinds]
    fn global_keybinds() {
        bind("escape", back);
        bind("f10", run_preview);
    }

    #[keybinds(page = Editor)]
    fn editor_keybinds() {
        bind("a", add_item);
        bind("d", delete_item);
        bind("J", move_item_down);
        bind("K", move_item_up);
    }

    #[keybinds(page = Preview)]
    fn preview_keybinds() {
        bind("[", prev_entity);
        bind("]", next_entity);
    }

    #[handler]
    async fn back(&self, cx: &AppContext, gx: &GlobalContext) {
        match self.page() {
            Page::Preview => {
                self.navigate(Page::Editor);
            }
            Page::Editor => {
                let confirmed = gx
                    .modal(crate::modals::ConfirmModal::with_message(
                        "Close the migration editor?",
                    ))
                    .await;

                if confirmed {
                    cx.close();
                }
            }
        }
    }

    #[handler]
    async fn run_preview(&self, gx: &GlobalContext) {
        use crate::apps::migration::comparison::MappingComparison;
        use crate::apps::migration::engine::materializer::MaterializeData;
        use crate::apps::migration::engine::materializer::materialize_chain;
        use crate::apps::migration::modals::SelectPhaseModal;
        use crate::apps::migration::pipeline;
        use crate::apps::migration::types::MatchStrategy;
        use crate::apps::migration::types::ParentType;
        use crate::modals::odata_fetch::ODataFetchModal;

        let phases = self.phases.get();
        if phases.is_empty() {
            gx.toast(Toast::warning("No phases to preview"));
            return;
        }

        let phase_options: Vec<(i64, String)> = phases
            .iter()
            .map(|p| (p.id, p.name.clone()))
            .collect();

        let Some(phase_id) = gx.modal(SelectPhaseModal::new_modal(phase_options)).await else {
            return;
        };

        let phase_name = phases
            .iter()
            .find(|p| p.id == phase_id)
            .map(|p| p.name.clone())
            .unwrap_or_default();

        // Gather entity mappings for this phase
        let entity_mappings = self.entity_mappings.get();
        let phase_mappings: Vec<_> = entity_mappings
            .iter()
            .filter(|em| em.phase_id == phase_id && !em.source_entity.is_empty() && !em.target_entity.is_empty())
            .cloned()
            .collect();

        if phase_mappings.is_empty() {
            gx.toast(Toast::warning("No entity mappings in this phase"));
            return;
        }

        // Load all transforms/branches/chains/conditions
        let all_transforms = self.transforms.get();
        let all_match_branches = self.match_branches.get();
        let all_coalesce_chains = self.coalesce_chains.get();
        let all_find_conditions = self.find_conditions.get();
        let all_field_mappings = self.field_mappings.get();
        let all_variables = self.variables.get();
        let all_match_conditions = self.match_conditions.get();

        let source_client = self.source_client.get().clone();
        let target_client = self.target_client.get().clone();

        // Fetch primary keys for all source/target entities
        let mut primary_keys: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        for em in &phase_mappings {
            for entity in [&em.source_entity, &em.target_entity] {
                if !entity.is_empty() && !primary_keys.contains_key(entity.as_str()) {
                    let client = if entity == &em.source_entity {
                        &source_client
                    } else {
                        &target_client
                    };
                    match client.metadata().entity(entity.as_str()).await {
                        Ok(meta) => {
                            primary_keys.insert(
                                entity.clone(),
                                meta.primary_id_attribute().to_string(),
                            );
                        }
                        Err(e) => {
                            gx.toast(Toast::error(format!(
                                "Failed to fetch metadata for {}: {}",
                                entity, e
                            )));
                            return;
                        }
                    }
                }
            }
        }

        // Per-mapping: materialize chains and build MappingInputs
        let mut materialized_field_mappings: Vec<Vec<(String, Vec<super::engine::ChainItem>)>> = Vec::new();
        let mut materialized_variables: Vec<Vec<(String, Vec<super::engine::ChainItem>)>> = Vec::new();
        let mut materialized_match_conditions: Vec<Vec<(String, Vec<super::engine::ChainItem>)>> = Vec::new();

        for em in &phase_mappings {
            // Filter data for this entity mapping
            let em_transforms: Vec<_> = all_transforms
                .iter()
                .filter(|t| t.entity_mapping_id == em.id)
                .cloned()
                .collect();
            let em_branches: Vec<_> = all_match_branches
                .iter()
                .filter(|b| em_transforms.iter().any(|t| t.id == b.transform_id))
                .cloned()
                .collect();
            let em_coalesces: Vec<_> = all_coalesce_chains
                .iter()
                .filter(|c| em_transforms.iter().any(|t| t.id == c.transform_id))
                .cloned()
                .collect();
            let em_find_conds: Vec<_> = all_find_conditions
                .iter()
                .filter(|f| em_transforms.iter().any(|t| t.id == f.transform_id))
                .cloned()
                .collect();

            let mat_data = MaterializeData::new(
                em_transforms,
                em_branches,
                em_coalesces,
                em_find_conds,
            );

            // Materialize field mapping chains
            let fm_chains: Vec<(String, Vec<_>)> = all_field_mappings
                .iter()
                .filter(|fm| fm.entity_mapping_id == em.id)
                .map(|fm| {
                    let chain = materialize_chain(ParentType::FieldMapping, fm.id, &mat_data);
                    (fm.target_field.clone(), chain)
                })
                .collect();

            // Materialize variable chains
            let var_chains: Vec<(String, Vec<_>)> = all_variables
                .iter()
                .filter(|v| v.entity_mapping_id == em.id)
                .map(|v| {
                    let chain = materialize_chain(ParentType::Variable, v.id, &mat_data);
                    (v.name.clone(), chain)
                })
                .collect();

            // Materialize match condition chains (for Find strategy)
            let mc_chains: Vec<(String, Vec<_>)> = all_match_conditions
                .iter()
                .filter(|mc| mc.entity_mapping_id == em.id)
                .map(|mc| {
                    let chain = materialize_chain(ParentType::MatchCondition, mc.id, &mat_data);
                    (mc.target_field.clone(), chain)
                })
                .collect();

            materialized_field_mappings.push(fm_chains);
            materialized_variables.push(var_chains);
            materialized_match_conditions.push(mc_chains);
        }

        // Build MappingInputs
        let mapping_inputs: Vec<pipeline::MappingInput<'_>> = phase_mappings
            .iter()
            .enumerate()
            .map(|(i, em)| {
                let source_pk = primary_keys
                    .get(&em.source_entity)
                    .map(|s| s.as_str())
                    .unwrap_or("id");
                let target_pk = primary_keys
                    .get(&em.target_entity)
                    .map(|s| s.as_str())
                    .unwrap_or("id");

                pipeline::MappingInput {
                    source_entity: &em.source_entity,
                    target_entity: &em.target_entity,
                    source_primary_key: source_pk,
                    target_primary_key: target_pk,
                    field_mappings: &materialized_field_mappings[i],
                    variables: &materialized_variables[i],
                    match_config_chain: None, // TODO: if needed for analysis
                    source_filter: em.source_filter.as_ref(),
                    target_filter: em.target_filter.as_ref(),
                    test_guids: em.test_guids.as_deref(),
                    mapping_name: &em.name,
                }
            })
            .collect();

        // 1. Analyze phase
        for (i, input) in mapping_inputs.iter().enumerate() {
            log::debug!(
                "[preview] MappingInput[{}]: name={:?} source={:?} target={:?} src_pk={:?} tgt_pk={:?} field_mappings={} variables={} test_guids={:?}",
                i, input.mapping_name, input.source_entity, input.target_entity,
                input.source_primary_key, input.target_primary_key,
                input.field_mappings.len(), input.variables.len(), input.test_guids,
            );
        }
        let phase_plan = pipeline::analyze_phase(&mapping_inputs);
        for (i, plan) in phase_plan.mapping_plans.iter().enumerate() {
            log::debug!(
                "[preview] FetchPlan[{}]: source_entity={:?} source_select={:?} expands={} target={:?} find_caches={}",
                i, plan.source.entity, plan.source.select, plan.source.expands.len(),
                plan.target.as_ref().map(|t| t.entity.as_str()),
                plan.find_caches.len(),
            );
        }
        log::debug!("[preview] merged_find_caches: {:?}", phase_plan.merged_find_caches.iter().map(|c| &c.entity).collect::<Vec<_>>());

        // 2. Build fetch tasks
        let fetch_tasks = match pipeline::build_phase_fetch_tasks(
            &phase_plan,
            &mapping_inputs,
            &source_client,
            &target_client,
        ) {
            Ok(tasks) => tasks,
            Err(e) => {
                gx.toast(Toast::error(format!("Failed to build fetch tasks: {:?}", e)));
                return;
            }
        };

        // 3. Collect and execute fetches
        let (all_tasks, index) = pipeline::collect_all_tasks(fetch_tasks);
        if all_tasks.is_empty() {
            gx.toast(Toast::warning("No data to fetch"));
            return;
        }

        let fetch_results = match gx.modal(ODataFetchModal::create(all_tasks)).await {
            Ok(results) => results,
            Err(e) => {
                gx.toast(Toast::error(format!("Fetch failed: {}", e)));
                return;
            }
        };

        // 4. Split results
        let split = pipeline::split_fetch_results(fetch_results, &index);

        // 5. Build find cache
        let find_cache = pipeline::build_find_cache(
            split.find_cache_records,
            &phase_plan.merged_find_caches,
        );

        // 6. Execute transforms + 7. Compare — per entity mapping
        let mut comparisons: Vec<MappingComparison> = Vec::new();
        for (i, em) in phase_mappings.iter().enumerate() {
            let source_records = &split.source_records[i];

            // Find target records for this mapping
            let target_records: Vec<_> = split
                .target_records
                .iter()
                .find(|(idx, _)| *idx == i)
                .map(|(_, records)| records.clone())
                .unwrap_or_default();

            // Execute transforms
            log::debug!(
                "[preview] Executing mapping[{}] {:?}: {} source records, {} target records",
                i, em.name, source_records.len(), target_records.len(),
            );
            let mapping_result = pipeline::execute_mapping(
                source_records,
                &materialized_variables[i],
                &materialized_field_mappings[i],
                &em.source_entity,
                &em.target_entity,
                &find_cache,
            );
            for (j, rr) in mapping_result.record_results.iter().enumerate() {
                log::debug!(
                    "[preview] mapping[{}] record[{}]: fields={:?} errors={:?}",
                    i, j,
                    rr.fields.iter().map(|(f, v)| format!("{}={:?}", f, v)).collect::<Vec<_>>(),
                    rr.errors.iter().map(|(f, e)| format!("{}={}", f, e)).collect::<Vec<_>>(),
                );
                if j >= 2 { break; } // only log first 3 records
            }

            // Compare
            let source_pk = primary_keys
                .get(&em.source_entity)
                .map(|s| s.as_str())
                .unwrap_or("id");
            let target_pk = primary_keys
                .get(&em.target_entity)
                .map(|s| s.as_str())
                .unwrap_or("id");

            let comparison = pipeline::compare_mapping_results(&pipeline::ComparisonInput {
                source_records,
                mapping_result: &mapping_result,
                target_records: &target_records,
                strategy: em.match_strategy,
                source_primary_key: source_pk,
                target_primary_key: target_pk,
                match_conditions: &materialized_match_conditions[i],
                source_entity: &em.source_entity,
                target_entity: &em.target_entity,
                find_cache: &find_cache,
                no_match_fallback: em.no_match_fallback,
                orphan_strategy: em.orphan_strategy,
            });

            comparisons.push(comparison);
        }

        // Store results and navigate
        self.preview_phase_name.set(phase_name);
        self.preview_results.set(comparisons);
        self.preview_entity_index.set(0);
        self.navigate(Page::Preview);
    }

    #[handler]
    async fn prev_entity(&self, _gx: &GlobalContext) {
        let count = self.preview_results.with_ref(|r| r.len());
        if count == 0 {
            return;
        }
        let current = self.preview_entity_index.get();
        let next = if current == 0 { count - 1 } else { current - 1 };
        self.preview_entity_index.set(next);
    }

    #[handler]
    async fn next_entity(&self, _gx: &GlobalContext) {
        let count = self.preview_results.with_ref(|r| r.len());
        if count == 0 {
            return;
        }
        let current = self.preview_entity_index.get();
        let next = if current + 1 >= count { 0 } else { current + 1 };
        self.preview_entity_index.set(next);
    }

    #[handler]
    async fn open_record_detail(&self, gx: &GlobalContext) {
        use crate::apps::migration::modals::RecordDetail;
        use crate::apps::migration::modals::RecordDetailModal;

        let focused_key = self.preview_table.with_ref(|t| t.focused_key);
        let key = match focused_key {
            Some(k) => k,
            None => return,
        };

        let detail = self.preview_results.with_ref(|results| {
            let index = self.preview_entity_index.get();
            let comparison = match results.get(index) {
                Some(c) => c,
                None => return None,
            };

            let record_count = comparison.records.len();
            if key < record_count {
                Some(RecordDetail::Record(comparison.records[key].clone()))
            } else {
                let orphan_index = key - record_count;
                comparison
                    .orphans
                    .get(orphan_index)
                    .map(|o| RecordDetail::Orphan(o.clone()))
            }
        });

        if let Some(detail) = detail {
            gx.modal(RecordDetailModal::with_detail(detail)).await;
        }
    }

    // =========================================================================
    // Preview derived values
    // =========================================================================

    #[derived]
    fn preview_entity_names(&self) -> Vec<String> {
        let results = self.preview_results.get();
        preview::entity_names(&results)
    }

    #[derived]
    fn preview_counts(&self) -> OperationTypeCounts {
        let results = self.preview_results.get();
        let index = self.preview_entity_index.get();
        preview::entity_counts(&results, index)
    }

    // =========================================================================
    // Preview table rebuild
    // =========================================================================

    #[watch]
    async fn rebuild_preview_table(&self) {
        use preview::build_preview_table;
        use rafter::widgets::SelectionMode;

        let results = self.preview_results.get();
        let index = self.preview_entity_index.get();

        if let Some(comparison) = results.get(index) {
            let (rows, columns) = build_preview_table(comparison);
            self.preview_table.set(
                TableState::new(rows, columns)
                    .with_selection(SelectionMode::Single)
                    .with_frozen(&["op", "source_id", "info"]),
            );
        } else {
            self.preview_table.set(TableState::default());
        }
    }

    #[handler]
    async fn add_item(&self, gx: &GlobalContext) {
        let focused_node = self.tree_state.with_ref(|s| {
            s.focused_key
                .as_ref()
                .and_then(|key| s.find_node(key))
                .map(|node| node.value.clone())
        });

        match focused_node {
            None => {
                // No selection -> add new phase
                self.add_phase_impl(gx).await;
            }
            Some(MigrationTreeNode::Phase(phase)) => {
                // Phase selected -> add entity mapping under it
                self.add_entity_mapping_impl(phase.id, gx).await;
            }
            Some(MigrationTreeNode::EntityMapping(em)) => {
                // Entity mapping selected -> add sibling entity mapping
                self.add_entity_mapping_impl(em.phase_id, gx).await;
            }
            Some(MigrationTreeNode::Variables { entity_mapping_id }) => {
                // Variables section -> add new variable
                self.add_variable_impl(entity_mapping_id, gx).await;
            }
            Some(MigrationTreeNode::Variable(_)) => {
                // Variable selected -> add transform to its chain
                self.add_transform_impl(gx).await;
            }
            Some(MigrationTreeNode::FieldMappings { entity_mapping_id }) => {
                // Field mappings section -> add new field mapping
                self.add_field_mapping_impl(entity_mapping_id, gx).await;
            }
            Some(MigrationTreeNode::FieldMapping(_)) => {
                // Field mapping selected -> add transform to its chain
                self.add_transform_impl(gx).await;
            }
            Some(MigrationTreeNode::Transform(ref tn))
                if matches!(tn.transform.data, TransformData::Match { .. }) =>
            {
                // Match transform -> add branch
                self.add_match_branch_impl(&tn.transform, gx).await;
            }
            Some(MigrationTreeNode::Transform(ref tn))
                if matches!(tn.transform.data, TransformData::Coalesce) =>
            {
                // Coalesce transform -> add fallback chain
                self.add_coalesce_chain_impl(&tn.transform, gx).await;
            }
            Some(MigrationTreeNode::Transform(ref tn))
                if matches!(
                    tn.transform.data,
                    TransformData::Find {
                        mode: FindMode::Where,
                        ..
                    }
                ) =>
            {
                // Find (Where mode) -> add condition
                self.add_find_condition_impl(&tn.transform, gx).await;
            }
            Some(MigrationTreeNode::Transform(..)) => {
                // Transform selected -> add transform after it in the chain
                self.add_transform_impl(gx).await;
            }
            Some(MigrationTreeNode::MatchBranch(_)) => {
                // Match branch selected -> add transform to the branch
                self.add_transform_impl(gx).await;
            }
            Some(MigrationTreeNode::CoalesceChain(_)) => {
                // Coalesce chain selected -> add transform to the chain
                self.add_transform_impl(gx).await;
            }
            Some(MigrationTreeNode::FindCondition(_)) => {
                // Find condition selected -> add transform to the condition
                self.add_transform_impl(gx).await;
            }
            Some(MigrationTreeNode::MatchDefault { .. }) => {
                // Match default selected -> add transform to the default chain
                self.add_transform_impl(gx).await;
            }
            Some(MigrationTreeNode::FindDefault { .. }) => {
                // Find default selected -> add transform to the default chain
                self.add_transform_impl(gx).await;
            }
            Some(MigrationTreeNode::Chain { .. }) => {
                // Chain wrapper selected -> add transform to the chain
                self.add_transform_impl(gx).await;
            }
            Some(MigrationTreeNode::MatchConfig { entity_mapping_id }) => {
                // MatchConfig (Find mode) -> add match condition
                self.add_match_condition_impl(entity_mapping_id, gx).await;
            }
            Some(MigrationTreeNode::MatchCondition(_)) => {
                // MatchCondition selected -> add transform to the condition's chain
                self.add_transform_impl(gx).await;
            }
            // Other config nodes don't support adding children
            Some(_) => {}
        }
    }

    #[handler]
    async fn delete_item(&self, cx: &AppContext, gx: &GlobalContext) {
        let focused_node = self.tree_state.with_ref(|s| {
            s.focused_key
                .as_ref()
                .and_then(|key| s.find_node(key))
                .map(|node| node.value.clone())
        });

        match focused_node {
            Some(MigrationTreeNode::Phase(phase)) => {
                self.delete_phase_impl(phase.id, cx, gx).await;
            }
            Some(MigrationTreeNode::EntityMapping(em)) => {
                self.delete_entity_mapping_impl(em.id, cx, gx).await;
            }
            Some(MigrationTreeNode::Variable(vn)) => {
                self.delete_variable_impl(vn.variable.id, vn.variable.entity_mapping_id, cx, gx)
                    .await;
            }
            Some(MigrationTreeNode::FieldMapping(fmn)) => {
                self.delete_field_mapping_impl(
                    fmn.field_mapping.id,
                    fmn.field_mapping.entity_mapping_id,
                    cx,
                    gx,
                )
                .await;
            }
            Some(MigrationTreeNode::Transform(tn)) => {
                self.delete_transform_impl(&tn.transform, cx, gx).await;
            }
            Some(MigrationTreeNode::MatchBranch(mb)) => {
                self.delete_match_branch_impl(&mb, cx, gx).await;
            }
            Some(MigrationTreeNode::CoalesceChain(cc)) => {
                self.delete_coalesce_chain_impl(&cc, cx, gx).await;
            }
            Some(MigrationTreeNode::FindCondition(fc)) => {
                self.delete_find_condition_impl(&fc, cx, gx).await;
            }
            Some(MigrationTreeNode::MatchCondition(mc)) => {
                self.delete_match_condition_impl(&mc, cx, gx).await;
            }
            // Other config nodes can't be deleted
            Some(_) | None => {}
        }
    }

    #[handler]
    async fn move_item_up(&self, gx: &GlobalContext) {
        log::debug!("move_item_up called");
        self.move_item(-1, gx).await;
    }

    #[handler]
    async fn move_item_down(&self, gx: &GlobalContext) {
        log::debug!("move_item_down called");
        self.move_item(1, gx).await;
    }

    async fn move_item(&self, direction: i32, gx: &GlobalContext) {
        let Some(focused) = self.focused_node() else {
            log::debug!("move_item: no focused node");
            return;
        };
        log::debug!("move_item: focused node = {:?}", focused);

        match focused {
            MigrationTreeNode::Variable(vn) => {
                self.reorder_variable_impl(
                    vn.variable.id,
                    vn.variable.entity_mapping_id,
                    direction,
                    gx,
                )
                .await;
            }
            MigrationTreeNode::FieldMapping(fmn) => {
                self.reorder_field_mapping_impl(
                    fmn.field_mapping.id,
                    fmn.field_mapping.entity_mapping_id,
                    direction,
                    gx,
                )
                .await;
            }
            MigrationTreeNode::Transform(tn) => {
                self.reorder_transform_impl(&tn.transform, direction, gx)
                    .await;
            }
            MigrationTreeNode::MatchBranch(mb) => {
                self.reorder_match_branch_impl(&mb, direction, gx).await;
            }
            MigrationTreeNode::CoalesceChain(cc) => {
                self.reorder_coalesce_chain_impl(&cc, direction, gx).await;
            }
            MigrationTreeNode::FindCondition(fc) => {
                self.reorder_find_condition_impl(&fc, direction, gx).await;
            }
            MigrationTreeNode::MatchCondition(mc) => {
                self.reorder_match_condition_impl(&mc, direction, gx).await;
            }
            // Other nodes don't support reordering
            _ => {}
        }
    }

    #[handler]
    async fn edit_item(&self, gx: &GlobalContext) {
        let Some(focused) = self.focused_node() else {
            return;
        };

        match focused {
            MigrationTreeNode::Phase(phase) => {
                self.edit_phase_impl(&phase, gx).await;
            }
            MigrationTreeNode::EntityMapping(em) => {
                self.edit_entity_mapping_impl(&em, gx).await;
            }
            MigrationTreeNode::MatchConfig { entity_mapping_id } => {
                self.edit_match_config_impl(entity_mapping_id, gx).await;
            }
            MigrationTreeNode::MatchCondition(mc) => {
                self.edit_match_condition_impl(&mc, gx).await;
            }
            MigrationTreeNode::SourceFilter { entity_mapping_id } => {
                self.edit_source_filter_impl(entity_mapping_id, gx).await;
            }
            MigrationTreeNode::TargetFilter { entity_mapping_id } => {
                self.edit_target_filter_impl(entity_mapping_id, gx).await;
            }
            MigrationTreeNode::UnmatchedHandling { entity_mapping_id } => {
                self.edit_unmatched_handling_impl(entity_mapping_id, gx)
                    .await;
            }
            MigrationTreeNode::Passes { entity_mapping_id } => {
                self.edit_passes_impl(entity_mapping_id, gx).await;
            }
            MigrationTreeNode::TestGuids { entity_mapping_id } => {
                self.edit_test_guids_impl(entity_mapping_id, gx).await;
            }
            MigrationTreeNode::Variables { entity_mapping_id } => {
                self.add_variable_impl(entity_mapping_id, gx).await;
            }
            MigrationTreeNode::Variable(vn) => {
                self.edit_variable_impl(&vn.variable, gx).await;
            }
            MigrationTreeNode::FieldMappings { .. } => {
                // Section header - no action, use 'a' to add
            }
            MigrationTreeNode::FieldMapping(_) => {
                self.add_transform_impl(gx).await;
            }
            MigrationTreeNode::Transform(tn) => {
                self.edit_transform_impl(&tn.transform, gx).await;
            }
            MigrationTreeNode::MatchBranch(mb) => {
                self.edit_match_branch_impl(&mb, gx).await;
            }
            MigrationTreeNode::CoalesceChain(_cc) => {
                // Coalesce chains have no config - Enter adds a transform
                self.add_transform_impl(gx).await;
            }
            MigrationTreeNode::FindCondition(fc) => {
                self.edit_find_condition_impl(&fc, gx).await;
            }
            MigrationTreeNode::MatchDefault { .. } => {
                // MatchDefault is managed by the Match transform's has_default flag
            }
            MigrationTreeNode::FindDefault { .. } => {
                // FindDefault is managed by the Find transform's fallback field
            }
            MigrationTreeNode::Chain { .. } => {
                // Chain wrappers don't have configuration - just add transforms under them
            }
        }
    }

    #[page(Editor)]
    fn editor_page(&self) -> Element {
        let focused = self.focused_node();
        let has_selection = focused.is_some();
        let (can_add, add_label) = match &focused {
            None => (true, "Add Phase"),
            Some(MigrationTreeNode::Phase(_)) => (true, "Add Entity"),
            Some(MigrationTreeNode::EntityMapping(_)) => (true, "Add Entity"),
            Some(MigrationTreeNode::Variables { .. }) => (true, "Add Variable"),
            Some(MigrationTreeNode::Variable(_)) => (true, "Add Transform"),
            Some(MigrationTreeNode::FieldMappings { .. }) => (true, "Add Field"),
            Some(MigrationTreeNode::FieldMapping(_)) => (true, "Add Transform"),
            Some(MigrationTreeNode::Transform(tn))
                if matches!(tn.transform.data, TransformData::Match { .. }) =>
            {
                (true, "Add Branch")
            }
            Some(MigrationTreeNode::Transform(tn))
                if matches!(tn.transform.data, TransformData::Coalesce) =>
            {
                (true, "Add Fallback")
            }
            Some(MigrationTreeNode::Transform(tn))
                if matches!(
                    tn.transform.data,
                    TransformData::Find {
                        mode: FindMode::Where,
                        ..
                    }
                ) =>
            {
                (true, "Add Condition")
            }
            Some(MigrationTreeNode::Transform(..)) => (true, "Add Transform"),
            Some(MigrationTreeNode::MatchBranch(_)) => (true, "Add Transform"),
            Some(MigrationTreeNode::CoalesceChain(_)) => (true, "Add Transform"),
            Some(MigrationTreeNode::FindCondition(_)) => (true, "Add Transform"),
            Some(MigrationTreeNode::MatchDefault { .. }) => (true, "Add Transform"),
            Some(MigrationTreeNode::FindDefault { .. }) => (true, "Add Transform"),
            Some(MigrationTreeNode::MatchCondition(_)) => (true, "Add Transform"),
            Some(MigrationTreeNode::MatchConfig { entity_mapping_id }) => {
                let is_find = self
                    .entity_mappings
                    .get()
                    .iter()
                    .find(|em| em.id == *entity_mapping_id)
                    .map(|em| {
                        em.match_strategy == crate::apps::migration::types::MatchStrategy::Find
                    })
                    .unwrap_or(false);
                if is_find {
                    (true, "Add Condition")
                } else {
                    (false, "Add")
                }
            }
            Some(MigrationTreeNode::Chain { .. }) => (true, "Add Transform"),
            Some(_) => (false, "Add"), // Other config nodes - can't add
        };

        let edit_label = match &focused {
            Some(MigrationTreeNode::CoalesceChain(_)) => "Add Transform",
            _ => "Edit",
        };

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: background) {
                row (width: fill, justify: between) {
                    text (content: {self.title()}) style (bold, fg: interact)
                    button (label: "Preview", hint: "f10", id: "preview-btn") on_activate: run_preview()
                }

                row (width: fill, height: fill) {
                    row (width: {tuidom::Size::Flex(3)}, height: fill) {
                        box_ (id: "migration-tree-container", height: fill, width: fill) style (bg: surface) {
                            tree (state: self.tree_state, id: "migration-tree", width: fill, height: fill)
                                on_activate: edit_item()
                        }
                        column (width: 1)
                    }

                    column (padding: 1, width: {tuidom::Size::Flex(2)}, height: fill) style (bg: surface) {
                        match focused {
                            None => {
                                column (width: fill, height: fill, justify: center, align: center) {
                                    text (content: "Select a phase or entity mapping") style (fg: muted)
                                }
                            }
                            Some(MigrationTreeNode::Phase(phase)) => {
                                column (gap: 1) {
                                    text (content: "Phase") style (bold, fg: interact)
                                    column {
                                        row (gap: 1) {
                                            text (content: "Name") style (fg: muted)
                                            text (content: {phase.name.clone()})
                                        }
                                        row (gap: 1) {
                                            text (content: "Mode") style (fg: muted)
                                            text (content: {if phase.mode == Mode::Lua { "Lua" } else { "Declarative" }})
                                        }
                                        row (gap: 1) {
                                            text (content: "Entities") style (fg: muted)
                                            text (content: {format!("{}", self.entity_count_for_phase(phase.id))})
                                        }
                                    }
                                }
                            }
                            Some(MigrationTreeNode::EntityMapping(em)) => {
                                column (gap: 1) {
                                    text (content: "Entity Mapping") style (bold, fg: interact)
                                    column {
                                        row (gap: 1) {
                                            text (content: "Name") style (fg: muted)
                                            text (content: {em.name.clone()})
                                        }
                                        row (gap: 1) {
                                            text (content: "Source") style (fg: muted)
                                            text (content: {em.source_entity.clone()})
                                        }
                                        row (gap: 1) {
                                            text (content: "Target") style (fg: muted)
                                            text (content: {em.target_entity.clone()})
                                        }
                                        row (gap: 1) {
                                            text (content: "Mode") style (fg: muted)
                                            text (content: {if em.mode == Mode::Lua { "Lua" } else { "Declarative" }})
                                        }
                                    }
                                }
                            }
                            Some(MigrationTreeNode::MatchConfig { entity_mapping_id }) => {
                                { self.render_match_config_detail(entity_mapping_id) }
                            }
                            Some(MigrationTreeNode::SourceFilter { entity_mapping_id }) => {
                                { self.render_config_detail("Source Filter", entity_mapping_id, "Filter which source records to process") }
                            }
                            Some(MigrationTreeNode::TargetFilter { entity_mapping_id }) => {
                                { self.render_config_detail("Target Filter", entity_mapping_id, "Filter which target records to consider for matching") }
                            }
                            Some(MigrationTreeNode::UnmatchedHandling { entity_mapping_id }) => {
                                { self.render_config_detail("Unmatched Handling", entity_mapping_id, "Configure behavior for unmatched source and target records") }
                            }
                            Some(MigrationTreeNode::Passes { entity_mapping_id }) => {
                                { self.render_config_detail("Passes", entity_mapping_id, "Enable or disable migration passes (create, update, delete, etc.)") }
                            }
                            Some(MigrationTreeNode::TestGuids { entity_mapping_id }) => {
                                { self.render_config_detail("Test GUIDs", entity_mapping_id, "Specify record GUIDs to test the migration with") }
                            }
                            Some(MigrationTreeNode::Variables { entity_mapping_id }) => {
                                { self.render_variables_detail(entity_mapping_id) }
                            }
                            Some(MigrationTreeNode::Variable(vn)) => {
                                { self.render_variable_detail(&vn) }
                            }
                            Some(MigrationTreeNode::FieldMappings { entity_mapping_id }) => {
                                { self.render_field_mappings_detail(entity_mapping_id) }
                            }
                            Some(MigrationTreeNode::FieldMapping(fmn)) => {
                                { self.render_field_mapping_detail(&fmn) }
                            }
                            Some(MigrationTreeNode::Transform(tn)) => {
                                { self.render_transform_detail(&tn) }
                            }
                            Some(MigrationTreeNode::MatchBranch(mb)) => {
                                { self.render_match_branch_detail(&mb) }
                            }
                            Some(MigrationTreeNode::CoalesceChain(cc)) => {
                                { self.render_coalesce_chain_detail(&cc) }
                            }
                            Some(MigrationTreeNode::FindCondition(fc)) => {
                                { self.render_find_condition_detail(&fc) }
                            }
                            Some(MigrationTreeNode::MatchCondition(mc)) => {
                                { self.render_match_condition_detail(&mc) }
                            }
                            Some(MigrationTreeNode::MatchDefault { .. }) => {
                                { self.render_match_default_detail() }
                            }
                            Some(MigrationTreeNode::FindDefault { .. }) => {
                                { self.render_find_default_detail() }
                            }
                            Some(MigrationTreeNode::Chain { parent_type, parent_id }) => {
                                { self.render_chain_detail(parent_type, parent_id) }
                            }
                        }
                    }
                }

                row (width: fill, justify: between) {
                    button (label: "Close", hint: "esc", id: "close-btn") on_activate: back()
                    row (gap: 1) {
                        button (label: {add_label}, hint: "a", id: "add-btn", disabled: {!can_add}) on_activate: add_item()
                        if has_selection {
                            button (label: {edit_label}, hint: "enter", id: "edit-btn") on_activate: edit_item()
                        }
                        if has_selection {
                            button (label: "Delete", hint: "d", id: "delete-btn") on_activate: delete_item()
                        }
                    }
                }
            }
        }
    }

    #[page(Preview)]
    fn preview_page(&self) -> Element {
        let phase_name = self.preview_phase_name.get();
        let counts = self.preview_counts();
        let entity_names = self.preview_entity_names();
        let entity_index = self.preview_entity_index.get();
        let has_entities = !entity_names.is_empty();

        let current_entity_label = entity_names
            .get(entity_index)
            .cloned()
            .unwrap_or_else(|| "No entities".to_string());

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: background) {
                // Header
                text (content: {format!("Preview: {}", phase_name)}) style (bold, fg: interact)

                // Stats row
                row (gap: 2) {
                    text (content: {format!("Create: {}", counts.create)}) style (fg: success)
                    text (content: {format!("Update: {}", counts.update)}) style (fg: info)
                    text (content: {format!("Skip: {}", counts.skip)}) style (fg: muted)
                    text (content: {format!("Delete: {}", counts.delete)}) style (fg: error)
                    if counts.deactivate > 0 {
                        text (content: {format!("Deactivate: {}", counts.deactivate)}) style (fg: warning)
                    }
                    if counts.ignore > 0 {
                        text (content: {format!("Ignore: {}", counts.ignore)}) style (fg: muted)
                    }
                    if counts.error > 0 {
                        text (content: {format!("Error: {}", counts.error)}) style (fg: error)
                    }
                }

                // Record table
                if has_entities {
                    box_ (id: "preview-table-container", height: fill, width: fill) style (bg: surface) {
                        table (state: self.preview_table, id: "preview-table") on_activate: open_record_detail()
                    }
                }
                if !has_entities {
                    column (width: fill, height: fill, justify: center, align: center) {
                        text (content: "No results") style (fg: muted)
                    }
                }

                // Footer
                row (width: fill, justify: between) {
                    button (label: "Back", hint: "esc", id: "back-btn") on_activate: back()
                    row (gap: 1) {
                        button (label: "◄", hint: "[", id: "prev-entity-btn") on_activate: prev_entity()
                        text (content: {current_entity_label}) style (fg: primary)
                        button (label: "►", hint: "]", id: "next-entity-btn") on_activate: next_entity()
                    }
                }
            }
        }
    }
}
