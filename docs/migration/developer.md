# Migration Developer Notes

The migration implementation lives under `dataverse-tui/src/apps/migration/`.

## Code map

- `mod.rs` — module wiring and public `MigrationList` export.
- `list/` — migration list app: create/delete/open migrations.
- `editor/` — migration editor UI, tree editing, preview, execution state machine, detail panels, and modal orchestration.
- `modals/` — focused dialogs for phases, entity mappings, transforms, filters, match config, passes, and validation helpers.
- `types/` — domain types, enums, transform data, conditions, and type-tracking structures.
- `repository/` — SQLite persistence for migrations, phases, mappings, variables, field mappings, transforms, and child rows.
- `migrations/` — SQLite schema migrations for the migration repository.
- `pipeline/` — phase analysis, fetch planning, find cache construction, and record-level transform execution orchestration.
- `engine/` — transform chain materialization/execution, path resolution, conditions, variables, and transform implementations.
- `comparison/` — matching, diffing, operation classification, entity Lua, and phase Lua parsing.
- `execution/` — conversion from comparison results or phase Lua operations into Dataverse batch operations.
- `validation/` — path parsing and validation helpers used by editor modals/type tracking.

## End-to-end data flow

### Editor load

1. `MigrationList` loads migrations from `MigrationRepository`.
2. Opening a migration resolves source and target clients with `ClientManagement/GetAnyClient`.
3. `MigrationEditor` loads all phase/mapping/transform data from the repository.
4. The editor rebuild watcher fetches needed metadata and builds the visible tree.
5. Tree nodes include type-tracking output and warnings for variables, field mappings, and transforms.

### Preview

1. The editor selects a phase.
2. DB rows are materialized into transform chains with `engine::materializer`.
3. `pipeline::analyze_phase` discovers required source/target fields and cache needs.
4. `pipeline::build_phase_fetch_tasks` creates OData fetch tasks.
5. The UI fetches data through `ODataFetchModal`.
6. The pipeline builds `LiveFindCache` values.
7. `pipeline::execute_mapping` runs variables and field mappings per source record.
8. `comparison::compare_mapping` matches, diffs, and detects orphans.
9. Junction mappings are remapped to associate/disassociate operation types.

### Execution

1. The editor starts a phase run record.
2. Comparisons are converted into pass-specific `EntityBatches` by `execution::generate_*_pass` functions.
3. Phase-level Lua uses `execution::phase_lua::build_phase_lua_batches` instead.
4. Batches are submitted to `Queue` as `QueuePayload::Batch` items.
5. Queue completion events are correlated with tracked item IDs.
6. Created IDs and operation errors are read from queue item results.
7. The editor advances through sub-phases and finalizes the phase run.

## Persistence model

The repository stores migration configuration as normalized rows:

- migrations,
- phases,
- entity mappings,
- variables,
- field mappings,
- transforms,
- match branches,
- coalesce chains,
- find conditions,
- match conditions,
- phase runs.

Transforms are stored as flat rows with:

- `entity_mapping_id`,
- `parent_type`,
- `parent_id`,
- `order`,
- `transform_type`,
- serialized transform data.

Nested transform scopes are represented with `ParentType` values. For example:

- field mapping chains use `ParentType::FieldMapping`,
- variable chains use `ParentType::Variable`,
- guard fallback chains use `ParentType::GuardFallback`,
- match branches use `ParentType::MatchBranch`,
- coalesce alternatives use `ParentType::CoalesceChain`,
- find condition source chains use `ParentType::FindCondition`.

`engine::materializer` converts these flat rows into `ChainItem` trees before execution.

## Adding a transform

A new transform usually touches several layers.

1. Add a variant to `types::TransformData`.
2. Add persistence support:
   - transform type string mapping,
   - serialization/deserialization if needed,
   - schema CHECK constraint migration if transform types are constrained.
3. Add execution support in `engine/transforms/` and dispatch from `engine::executor::execute_transform`.
4. Add materializer handling if the transform owns child chains.
5. Add pipeline analysis support if the transform references source fields, target fields, find caches, entity refs, or Lua declarations.
6. Add editor modal support under `modals/`.
7. Add insert/edit wiring in `editor/transform_operations.rs`.
8. Add tree rendering/detail display support if needed.
9. Add type-tracking support in `editor/tree_types.rs` and/or `types/type_tracking.rs` if output type can be inferred.
10. Add tests outside implementation modules, following repository test conventions.

For a simple stateless transform, the minimum path is usually:

- `TransformData` variant,
- repository string/data mapping,
- executor dispatch,
- transform implementation,
- selection/edit modal wiring.

For scope-owning transforms like Guard, Match, Coalesce, or Find, also update:

- `ParentType`,
- materialization,
- child-scope editor operations,
- deletion/cascade assumptions,
- type tracking through child chains.

## Adding execution behavior

Execution behavior is split by pass. Prefer adding behavior to the smallest pass-specific function that needs it.

Important files:

- `execution/mod.rs` — declarative pass generation.
- `execution/phase_lua.rs` — phase-level Lua operation conversion.
- `editor/execute.rs` — execution state machine and queue tracking.

Be careful with:

- pass order,
- disabled passes per entity mapping,
- batch size,
- lookup deferral from Create to Update,
- inactive record handling through Activate/Deactivate,
- created ID capture from queue results,
- junction entity remapping.

## Adding or changing Lua behavior

Lua parsing/execution is split by use case:

- `comparison/matching.rs` — Lua match declarations and match indexes.
- `comparison/entity_lua.rs` — entity-level Lua mapping.
- `comparison/phase_lua.rs` — phase-level Lua operation parsing.
- `execution/phase_lua.rs` — phase-level Lua operation batching.
- `pipeline/analysis.rs` — `M.declare()` fetch planning.

When adding Lua-accessible data, update both the declaration analysis and runtime table construction. Preview and execution must fetch the same data the script expects.

## Type tracking

The editor does static-ish type tracking for transform chains.

Main files:

- `editor/tree_builder.rs` — tree construction and type accumulator.
- `editor/tree_types.rs` — path/transform output type inference.
- `types/type_tracking.rs` — shared type result and warning structures.

Type tracking is advisory. It should help users catch mapping mistakes without preventing valid dynamic configurations from being represented.

## Repository migrations

Migration repository schema migrations live in `dataverse-tui/src/apps/migration/migrations/`.

When changing persisted shape:

1. Add a numbered SQL migration.
2. Update repository row mapping and CRUD functions.
3. Update domain types.
4. Update editor loading/rebuild code.
5. Consider how old saved migrations should be interpreted after the schema change.

## Design guidance

Keep migration changes vertical. A feature is usually not complete until it can be configured in the editor, saved in the repository, previewed, and executed or intentionally documented as preview-only.

Prefer adding behavior to existing deep modules instead of creating shallow parallel paths. The migration framework already has clear boundaries: repository, editor, pipeline, engine, comparison, execution.
