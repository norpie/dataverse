# Migration Preview and Execution

Preview and execution use the same configuration, but they do different things.

- **Preview** fetches data, runs transforms, matches records, and calculates what would happen.
- **Execution** turns preview/comparison results into Queue items and waits for them to complete.

## Preview pipeline

When you press `f10` in the editor and select a phase, the migration app builds a phase-level fetch plan.

The preview pipeline is:

1. Analyze every entity mapping in the selected phase.
2. Determine source fields, target fields, lookup navigation expands, find caches, entity-reference caches, and Lua-declared data requirements.
3. Fetch source records from the source environment.
4. Fetch target records from the target environment.
5. Fetch shared find caches and extra Lua-declared entities.
6. Materialize flat DB transform rows into executable transform chains.
7. Execute variables and field mappings for each source record.
8. Match each source record to a target record.
9. Diff transformed fields against matched target records.
10. Detect target orphans.
11. Present operation counts and details.

Preview does not write to Dataverse.

## Operation types

Preview classifies records as operation types.

Source-side records can become:

- **Create** — no target match and no-match handling allows creation.
- **Update** — matched target exists and field diffs are present.
- **Skip** — matched target exists and no diffs are present.
- **Associate** — junction entity create mapped to an N:N association.
- **Ignore Source** — source record intentionally skipped or no-match handling is Ignore.
- **Error** — transform, matching, or configuration error.

Target-side orphans can become:

- **Delete** — orphan strategy is Delete.
- **Deactivate** — orphan strategy is Deactivate.
- **Disassociate** — junction entity orphan mapped to an N:N disassociation.
- **Ignore Target** — orphan strategy is Ignore.
- **Error** — orphan strategy is Error.

## Matching and orphan scope

Only fetched target records can match or become orphans.

That means target filters are important:

- a narrow target filter limits matching and orphan detection,
- a broad target filter can make many target records eligible for deletion/deactivation if orphan handling is destructive.

Use Test GUIDs and safe target filters when validating a new mapping.

## Execution passes

Execution runs in ordered sub-phases:

1. **Create**
2. **Activate**
3. **Update**
4. **Associate**
5. **Disassociate**
6. **Deactivate**
7. **Delete**

Each entity mapping can enable or disable each pass.

### Create

Creates target records for Create comparisons.

Declarative Create operations include scalar fields immediately. Lookup fields are normally deferred to the Update pass so records can be created before cross-record lookup dependencies are applied.

If the Update pass is disabled for an entity, lookup fields are included in Create instead.

When the transformed fields include the target primary key, the Create pass includes it so Dataverse uses that GUID.

### Activate

Reactivates inactive target records before Update.

Dataverse rejects normal PATCH updates on inactive records. The Activate pass sets active state/status before updates, and the Deactivate pass can restore the intended inactive state afterwards.

### Update

Applies:

- deferred lookup fields on newly created records,
- field diffs on existing records.

State fields are handled by Activate/Deactivate. Status code is handled in Update only when the record remains active.

### Associate

Creates N:N associations for junction entity mappings.

The execution layer uses Dataverse relationship metadata to build associate operations.

### Disassociate

Removes N:N associations for orphaned junction records when orphan handling calls for removal.

### Deactivate

Sets inactive state/status for records that should end inactive, including orphan records when orphan strategy is Deactivate.

### Delete

Deletes orphaned target records when orphan strategy is Delete.

## Queue integration

Execution does not write directly from the migration editor. It builds Dataverse batch operations and submits them to the persistent Queue app.

Queue items use source `migration` and descriptions like:

```text
Create account (50)
Update contact (12)
Disassociate account_contact (4)
```

Batches are grouped by entity and pass. Batch size is 50 operations.

The migration editor tracks the queue item IDs for the active execution. When Queue publishes completion events, the editor updates pass progress, captures created IDs, records operation errors, and advances to the next pass when the current pass completes.

## Created ID capture

Create operations use the source record GUID as content ID. Queue execution results are inspected after each create batch.

Captured target IDs are used by later passes, especially deferred lookup updates and deactivation operations for newly-created records.

## Failure behavior

If a queue item partially or fully fails, the migration editor records operation-level errors when results are available.

The execution can continue through tracked queue completion, but the final phase run is marked according to collected errors/status. Review the execution screen and Queue item details before rerunning.

## Cancellation and cleanup

Execution creates a phase run record for history. The editor tracks all queue items submitted for the run.

When cancelling an execution, queued items for the run can be cleaned up by source/item tracking. Avoid manually deleting migration queue items while an execution is still active unless you are intentionally interrupting the run.

## Phase run history

Each execution creates a Phase Run record with:

- phase ID,
- started timestamp,
- completion timestamp when finished,
- status: running, completed, failed, or cancelled,
- queue item IDs,
- error text when applicable.
