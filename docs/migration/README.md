# Migrations

The migration framework moves Dataverse data between two configured environments. A migration is a saved configuration: it can be edited, previewed, and executed repeatedly.

Use this app when data needs more structure than a one-off import/export: multiple tables, ordered phases, field transforms, record matching, lookup handling, association changes, and repeatable execution.

## Documentation

- [Concepts](concepts.md) — the objects that make up a migration configuration.
- [Transforms](transforms.md) — transform chains, path syntax, and the transform catalogue.
- [Lua](lua.md) — Lua match scripts, Lua find scripts, entity-level Lua, and phase-level Lua.
- [Execution](execution.md) — preview, comparison, queue submission, and execution passes.
- [Developer notes](developer.md) — code map and extension points.

## Normal workflow

### 1. Prepare environments

Before creating a migration, configure and authenticate the environments in Client Management.

A migration needs:

- a **source environment** to read from,
- a **target environment** to compare and write to.

The migration list can show environments without an active session, but opening a migration requires an authenticated account for both environments.

### 2. Create a migration

1. Open the launcher with `ctrl+p`.
2. Select **Migrations**.
3. Press `n` or activate **New**.
4. Enter a name and optional description.
5. Select the source and target environments.
6. Create the migration.

The migration appears in the list. Focus it and press `enter` to open the editor.

### 3. Add phases

A migration is split into ordered phases. Use phases to separate logical units of work, for example:

- reference data first,
- parent records next,
- child records after parents,
- cleanup or association-only work last.

In the editor:

- press `a` on the migration tree to add an item in the focused location,
- press `d` to delete the focused item,
- use `J` and `K` to reorder items.

New phases are declarative by default. Existing phases can be edited into Lua phases when the whole phase needs script-controlled behavior.

### 4. Add entity mappings

An entity mapping describes one source entity and one target entity. It decides what records to fetch, how to match source records to target records, how to transform field values, and how unmatched records should be handled.

For each entity mapping, configure the relevant child sections:

- **Match Config** — how source records match target records.
- **Source Filter** — limits source records fetched for preview/execution.
- **Target Filter** — limits target records considered for matching and orphan detection.
- **Unmatched Handling** — what to do when source or target records do not match.
- **Passes** — which execution passes are enabled for this mapping.
- **Test GUIDs** — optional source GUID list for safe test runs.
- **Variables** — reusable computed values for field mappings.
- **Field Mappings** — target fields and transform chains that produce values.

### 5. Configure variables and field mappings

A field mapping targets one target field. Its transform chain computes the desired value for that field.

A variable is also a transform chain, but it is named and can be reused by later variables and field mappings through `$variable_name` paths.

Typical examples:

- copy a source field into a target field,
- format a label from multiple source fields,
- map option set values,
- resolve a target lookup with a Find transform,
- generate a deterministic field value with a Lua mapping when declarative transforms are not enough.

See [Transforms](transforms.md) for details.

### 6. Preview

Press `f10` in the editor to preview a phase.

Preview fetches source data, target data, and any find caches; then it runs transforms, matches records, and builds a comparison.

Review the operation counts before executing:

- creates,
- updates,
- skips,
- ignored records,
- association/disassociation work,
- deletes/deactivations,
- errors.

Use `[` and `]` in preview to move between entity mappings in the selected phase.

### 7. Execute

From preview, press `f10` again to execute.

Execution submits operations to the persistent Queue app. The migration editor tracks queue item completion and advances through execution passes automatically.

Do not close the app during an active execution unless cancelling is intended. Use the Queue and execution screen to monitor progress and failures.
