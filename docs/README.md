# Dataverse TUI

High-level documentation for the terminal user interface in this repository.

## Apps

- **Welcome** — Home screen with basic startup guidance and launcher hints.
- **Entity Explorer** — Browse Dataverse table/entity metadata for the active environment.
- **Record Explorer** — View records returned by a query, page through results, and inspect values.
- **Audit Log** — Select a record and review its Dataverse audit history.
- **Query Builder** — Build OData queries visually, save/load query definitions, and send results to record viewing or export flows.
- **Import** — Load CSV or Excel files, preview rows, configure import behavior, and queue Dataverse operations.
- **Export** — Run a query and export the returned records to CSV or Excel.
- **Queue** — Persistent operation queue for Dataverse writes, with execution controls, filtering, retries, and result inspection.
- **Migrations** — Define and run structured data migrations between Dataverse environments.
- **[VAF - Deadline Import](deadline-importer.md)** — Import VAF deadline workbook data, compare it with Dataverse state, and queue create/update operations.
- **VAF - Questionnaire Sync** — Compare questionnaire data between environments and queue synchronization operations.
- **VAF - Questionnaire Validator** — Validate questionnaire configuration and export validation results.

## Client

- **[Client Management](client-management.md)** — Manages accounts, environments, active sessions, authentication, and Dataverse client creation for apps and systems.
- **Client Manager** — Pools Dataverse clients per account/environment pair and wires each client to the appropriate persistent environment cache.

## Indexer

- **Metadata Indexer** — Background system that keeps Dataverse metadata caches warm, tracks sync status per environment, and exposes dashboard controls for pause/resume, manual sync, settings, and cache clearing.
