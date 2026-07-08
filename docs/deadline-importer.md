# VAF Deadline Importer

The deadline importer loads a VAF deadline Excel workbook, compares it with the active Dataverse environment, and queues the required create/update operations.

This app is intentionally conservative: rows with validation warnings are shown to the user and are not queued.

## Normal maintenance flow

### 1. Select the correct environment

Before opening the importer, make sure the active Dataverse connection is the environment you want to update.

- Use **Client Management** (`alt+m`) to switch account/environment if needed.
- Open the launcher with `ctrl+p`.
- Select **VAF - Deadline Import**.

The importer opens a file picker immediately when it starts. You can also press `o` later to open a different workbook.

### 2. Choose the workbook and sheet

The importer accepts Excel `.xlsx` files.

After selecting a workbook, choose the sheet containing the deadline data. The sheet must contain a header row with cells containing both `Domein` and `Deadline`; rows above that are ignored.

Rows are skipped when:

- the row is completely empty, or
- the sheet has an `IGNORE` column and that row has any value in it.

### 3. Wait for loading and comparison

The importer performs three loading phases:

1. **Metadata loading** — resolves Dataverse entity set names for deadline-related tables.
2. **NRQ data loading** — fetches lookup data, existing deadlines, and existing deadline support links.
3. **Processing** — reads the workbook, resolves lookups, validates rows, and compares workbook rows with existing Dataverse records.

The comparison is based on the deadline GUID in the workbook `id` column:

- valid existing `id` -> possible update,
- empty `id` -> new generated GUID and create,
- invalid `id` -> row warning and no queued operation.

If a row matches an existing deadline but no fields or associations changed, it is marked **Unchanged**.

### 4. Review the result table

The result table shows:

- **Row** — Excel row number.
- **Mode** — `Create`, `Update`, `Unchanged`, or `Error`.
- **ID** — deadline GUID that will be used.
- **Deadline** — deadline name.
- **Warn** — number of validation warnings for the row.

Focus a row to inspect details on the right side. The detail pane includes resolved lookups, changed field count, association additions/removals, and warnings.

Mode meanings:

- **Create** — row is valid and will create a new Dataverse deadline.
- **Update** — row is valid and changes an existing Dataverse deadline.
- **Unchanged** — row matches existing Dataverse state and will not be queued.
- **Error** — row has validation warnings and will not be queued.

`OPM` notes are displayed and counted in the top warning/note summary, but do not by themselves block queueing.

### 5. Queue operations

Press `q` or activate **Queue**.

The confirmation modal shows the number of create and update operations that will be queued. Only rows that are both actionable and warning-free are queued.

Queued work is sent to the persistent **Queue** app with source `deadline-import`. Monitor the Queue app/status area for completion, partial failures, retries, or errors.

The importer queues Dataverse batch operations rather than writing directly from the import screen.

## Supported workbook conventions

### Direct columns

Current mapped workbook columns:

- `Domein*` or `Pillar` -> `nrq_DomainId` lookup
- `Fonds*` -> `nrq_FundId` lookup
- `Deadline*` -> `nrq_deadlinename`
- `Projectbeheerder` -> `nrq_ProjectManagerId` lookup
- `Info` -> `nrq_description`
- `Datum Deadline` + `Uur` -> `nrq_deadlinedate`
- `Commissie` -> `nrq_CommissionId` lookup
- `Raad van Bestuur` -> `nrq_BoardOfDirectorsMeetingId` lookup by meeting date
- `Type` -> `nrq_TypeID` lookup
- `Datum Commissievergadering` + `Uur Commissie` -> `nrq_committeemeetingdate`
- `Online of Fysiek` -> `nrq_committeemeetinginperson`
- `Support Type` -> `nrq_supporttypeoptionset`

### Checkbox columns

Checkbox columns start after the `Raad van Bestuur` column, or after `Type` when no board column exists.

A checkbox is considered checked when the value is one of:

- `x`
- `1`
- `true`
- `yes`

Checked column headers are resolved against active Dataverse records for:

- support
- category
- subcategory
- Flemish share

`OPM` and `IGNORE` are not treated as checkbox columns.

### Lookup matching

Lookups are matched case-insensitively by the lookup record name. For `systemuser`, domain name and internal email are also considered.

Most lookup fetches only include active records (`statecode = 0`). `systemuser` is the exception.

### Dates and times

Dates accept common Excel serial dates and string formats such as:

- `YYYY-MM-DD`
- `DD/MM/YYYY`
- `MM/DD/YYYY`
- `DD-MM-YYYY`
- `MM-DD-YYYY`
- `YYYY/MM/DD`

Times accept Excel time fractions and formats such as:

- `HH:MM`
- `HH:MM:SS`
- `H:MM AM/PM`

Deadline and committee datetimes are interpreted in the Europe/Brussels timezone and converted to UTC for Dataverse.

## Developer extension guide

Deadline importer code lives in `dataverse-tui/src/apps/deadline_import/`.

Important files:

- `scope.rs` — Dataverse entity constants, relationship names, lookup entity list, field mappings, picklist mappings, and constant fields.
- `excel.rs` — workbook and sheet parsing.
- `fetch.rs` — metadata resolution, lookup fetches, existing deadline fetches, and existing association parsing.
- `transform.rs` — row validation, lookup resolution, date/time parsing, checkbox association detection, and conversion into `DeadlineRecord`.
- `diff.rs` — comparison between workbook rows and existing Dataverse records.
- `operations.rs` — queue operation generation for creates, updates, associations, disassociations, and support junction records.
- `types.rs` — importer data model.
- `mod.rs` — app UI and end-to-end flow.

### Main pipeline

The importer flow is:

1. `excel::read_deadline_sheet` reads workbook headers and rows.
2. `fetch_metadata` resolves entity set names.
3. `build_fetch_tasks` fetches lookup records, existing deadlines, and existing support junctions.
4. `build_import_context` builds the lookup cache and existing deadline map.
5. `transform_workbook` turns Excel rows into `DeadlineRecord` values.
6. `diff::apply_diffs` marks each valid record as create, update, or unchanged.
7. `operations::build_queue_items` converts actionable records into queue items.

### Adding a simple mapped field

For a normal deadline field backed by an existing workbook column:

1. Add a `FieldMapping` in `scope.rs`.
2. Choose the correct `FieldKind`:
   - `Direct` for string fields.
   - `Lookup { target_entity }` for lookup fields.
   - `Picklist(...)` for option sets.
   - `Boolean { true_value, false_value }` for booleans.
3. If it is a lookup, ensure the target entity is included in `LOOKUP_ENTITIES`.
4. If updates should compare the field against existing Dataverse data, add the Dataverse field to the existing deadline query in `fetch.rs`.
5. For lookup comparison, the existing query must select the Dataverse lookup value field, usually `_<logical_lookup_field_lowercase>_value`.

Direct fields, lookups, picklists, and booleans are already handled generically by payload generation in `operations.rs`.

### Adding lookup support for a new entity

When adding a new lookup target:

1. Add the logical entity name to `LOOKUP_ENTITIES` in `scope.rs`.
2. Add or update `FieldMapping` entries that reference it.
3. Confirm `fetch::record_name` can find the display name for that entity.
4. Confirm `fetch::record_id` can find the primary key for that entity.

The default ID logic expects `<entity logical name>id`, except for `systemuser`. If the entity does not follow that convention, update `record_id`.

The default name logic checks `name`, `nrq_name`, `fullname`, and `domainname`. If the entity uses a different primary name field, update `record_name`.

### Adding a date/time field

Date/time fields are not fully generic today. `DeadlineFields` currently has dedicated storage for:

- deadline date/time,
- committee date/time.

To add another date/time field:

1. Add storage to `DeadlineFields` in `types.rs`.
2. Update `transform.rs` to assign parsed `Date` and `Time` mappings into that storage.
3. Update `operations.rs` to convert the local Brussels date/time into UTC and insert the Dataverse field.
4. Update `diff.rs` to compare the existing Dataverse value.
5. Add the field to the existing deadline select list in `fetch.rs`.

### Adding checkbox/association support

Checkbox associations currently support:

- support through the `nrq_deadlinesupport` junction entity,
- category through a many-to-many relationship,
- subcategory through a many-to-many relationship,
- Flemish share through a many-to-many relationship.

To add another checkbox-backed association:

1. Add entity and relationship constants in `scope.rs`.
2. Add the lookup entity to `LOOKUP_ENTITIES`.
3. Add storage to `DeadlineAssociations`, `ExistingAssociations`, and `AssociationDiff` in `types.rs`.
4. Update `transform::resolve_checkbox_columns` to classify checked headers into the new association bucket.
5. Update `fetch.rs` to fetch existing associations:
   - use expanded relationships for many-to-many associations, or
   - add a separate fetch task for junction-entity associations.
6. Update `diff.rs` to calculate additions/removals.
7. Update `operations.rs` to generate associate/disassociate operations or junction create/delete operations.
8. Update the detail display in `mod.rs` so users can review additions/removals.

### Adding or changing option-set values

Option-set labels are mapped in `scope.rs`.

For `Support Type`, update `SUPPORT_TYPE_OPTIONS`. Labels must match workbook cell values exactly. If both Dutch and English labels are expected, include both labels with the same Dataverse option value.

### Changing constants applied to every deadline

`scope::constant_fields()` returns fields inserted into every create/update payload. Update this function when a field must always be set by the importer.

Current constants:

- `nrq_vafvalidated = true`
- `nrq_publishdeadlineonvafbe = false`
- `nrq_canbepublished = true`

### Operational behavior to preserve

The queue operations intentionally use:

- batch size of 50 operations,
- separate queue groups for creates, updates, removals, and additions,
- priority ordering that runs creates before updates, updates before removals, and removals before additions,
- plugin/flow/sync bypass flags on generated operations.

Be deliberate before changing those defaults; they affect how imports behave in production environments.
