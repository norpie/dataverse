# Migration Concepts

## Migration

A migration is the top-level saved configuration. It has a name, optional description, source environment, and target environment.

The source environment is read-only during preview and execution. The target environment is compared during preview and written during execution.

## Phase

A phase is an ordered group inside a migration. A phase is previewed and executed as a unit.

Use phases to control dependencies between records. For example, migrate lookup/reference entities before records that refer to them.

Phase modes:

- **Declarative** — contains entity mappings, variables, field mappings, transforms, filters, and match configuration.
- **Lua** — a phase-level Lua script returns explicit operations and bypasses declarative mapping/comparison.

## Entity mapping

An entity mapping connects one source entity to one target entity.

Each mapping contains:

- source entity logical name,
- target entity logical name,
- match strategy,
- unmatched handling,
- pass enablement,
- source and target filters,
- optional test GUIDs,
- variables,
- field mappings.

Entity mapping modes:

- **Declarative** — field values are produced by transform chains.
- **Lua** — one Lua script receives source/target records and returns desired target fields and optional target matches.

## Match config

Matching determines whether a source record corresponds to an existing target record.

Strategies:

- **Same ID** — source and target records use the same GUID.
- **Find** — target records are matched with configured target fields and source-side transform chains.
- **Lua** — a Lua script builds a source GUID to target GUID match index.

When no target match is found, the mapping's no-match fallback decides what happens:

- **Create** — treat the source record as a target create.
- **Ignore** — ignore the source record.
- **Error** — mark the source record as an error.

## Orphans

An orphan is a target record that is included in the target fetch but did not match any source record.

Orphan strategies:

- **Ignore** — leave the target record untouched.
- **Delete** — queue a delete operation.
- **Deactivate** — queue a deactivate operation.
- **Error** — mark the target record as an error.

Be careful with broad target filters and destructive orphan strategies. The target filter defines the scope of records that can be treated as orphans.

## Source and target filters

Filters restrict the records fetched for a mapping.

- **Source filter** limits source records that transform into target records.
- **Target filter** limits target records considered for matching and orphan handling.

Filters are part of preview and execution. They are not just UI filters.

## Test GUIDs

Test GUIDs restrict a mapping to an explicit set of source record IDs. When present, they override the source filter for that mapping.

Use test GUIDs for safe trial runs of a mapping or phase before widening the source filter.

## Variables

A variable is a named transform chain scoped to one entity mapping.

Variables:

- execute before field mappings,
- are processed in order,
- can reference source fields and earlier variables,
- can be referenced with `$name`,
- have a declared output type used by editor type checking.

Example uses:

- normalize a source value once and reuse it,
- find a target lookup record once and reuse it for multiple fields,
- build a shared formatted name.

## Field mappings

A field mapping targets one target field. Its transform chain computes the desired value for that target field.

During preview, transformed field values are compared with the matched target record. Differences become update operations. Values for unmatched source records become create payloads.

## Transform chains

Transforms run in order. Each transform receives the previous output as `#value`, and its output becomes the next `#value`.

Top-level field mappings and variables start with `#value = null` unless they are inside a child scope where a parent transform provides a value.

See [Transforms](transforms.md).

## Junction entities and associations

Target entities that Dataverse metadata marks as intersect/junction entities are treated as N:N associations.

For junction mappings:

- normal creates become **Associate** operations,
- normal updates become **Skip** because associations are binary,
- orphan deletes/deactivations become **Disassociate** operations.

## Execution passes

Execution is split into ordered passes:

1. Create
2. Activate
3. Update
4. Associate
5. Disassociate
6. Deactivate
7. Delete

Passes can be disabled per entity mapping. This is useful when a phase should preview certain operation types but not execute them, or when create/update responsibilities are split across phases.

See [Execution](execution.md).
