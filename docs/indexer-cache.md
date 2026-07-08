# Indexer and Cache

The metadata indexer keeps Dataverse metadata caches warm for authenticated environments. It runs in the background and can be monitored from the Indexer Dashboard.

Open the dashboard with `alt+i`, or click the **Indexer** section in the taskbar.

## What the indexer stores

There are two separate kinds of storage:

- **Indexer database** — tracks sync status and sync history.
  - Linux: `~/.local/share/dataverse/indexer.db`
- **Environment cache databases** — store the actual cached Dataverse metadata and query payloads.
  - Linux: `~/.cache/dataverse/{host}_{hash}.db`

The indexer database does not contain the cached metadata itself. It only records whether an environment is idle, syncing, or in error, plus recent sync history.

## What is cached

The cache is shared by Dataverse clients and apps. Cache categories are:

- **Entities** — entity list and entity metadata.
- **Attributes** — attribute metadata.
- **Relationships** — relationship metadata.
- **Global Option Sets** — global option-set metadata.
- **Queries** — OData/FetchXML query result pages.

The background indexer mainly warms:

- the entity list,
- full/core entity metadata,
- global option sets.

Query results are cached when apps run queries. The dashboard can clear query cache entries, but the indexer does not proactively generate query results.

## Important: cache is not invalidated by Dataverse edits

The cache does not know when records or metadata are changed outside the TUI. It also does not automatically invalidate query results after edits made through the app.

If you make changes and need the TUI to reflect them immediately, manually clear the relevant cache category or use **Clear All** for the active environment.

Common examples:

- changed table/field metadata -> clear **Entities**, **Attributes**, or **Relationships**;
- changed global option sets -> clear **Global Option Sets**;
- changed data that appears in query results -> clear **Queries**.

## Dashboard workflow

### Status tab

Use the Status tab to:

- view overall indexer status,
- view per-environment sync status,
- pause or resume the indexer,
- sync the selected environment,
- sync all authenticated environments.

Only authenticated environments are shown. If an environment is configured but not authenticated with any account, it will not appear in the indexer status list.

### Settings tab

Use the Settings tab to configure:

- check interval,
- refresh threshold,
- cache TTLs.

Defaults:

| Setting | Default |
| --- | ---: |
| Check interval | 60 seconds |
| Refresh threshold | 80% |
| Entity list TTL | 24 hours |
| Entity metadata TTL | 6 hours |
| Attribute metadata TTL | 6 hours |
| Global option set TTL | 12 hours |
| Relationship TTL | 12 hours |
| Query TTL | 1 hour |

Refresh threshold means: refresh when this percentage of the TTL has elapsed. For example, with a 6-hour TTL and an 80% threshold, the indexer treats the cache as ready to refresh after about 4.8 hours.

Changing indexer settings invalidates cached Dataverse clients so future client creation uses the new cache configuration.

### Cache tab

Use the Cache tab to clear cached data by category.

You can clear categories for:

- the active environment, or
- all currently cached environments.

Clearing cache removes cached data only. It does not delete accounts, environments, OAuth tokens, saved settings, migrations, or queue items.

After clearing cache, data is rebuilt by app usage or by manually triggering a sync.

## Taskbar status

The taskbar shows the indexer state:

- idle,
- syncing,
- partial error,
- error.

The dashboard gives more detail per environment. During sync, progress shows entity metadata progress and global option-set progress.

## How sync works

At startup, Client Management publishes the authenticated environment list. The indexer builds check tasks for those environments.

For each environment, the indexer:

1. opens or reuses the Dataverse client and its environment cache,
2. inspects cache keys and expiration times,
3. checks whether entries are missing, expired, or past the refresh threshold,
4. fetches the entity list when needed,
5. fetches stale or missing entity metadata,
6. fetches all global option sets,
7. updates indexer status and sync logs.

The indexer processes sync work through an internal queue. Only one scheduled queue-processing job is active at a time.

## Cache clearing behavior

**Active environment** clearing uses the active client and its cache.

**All Envs** clearing uses currently cached clients in memory. If an environment has not had a client opened in the current session, it may not be affected by **All Envs** clearing until that client exists.

If a persistent cache cannot be opened, the app falls back to an in-memory cache for that client. In-memory cache data is not preserved across restarts.

## Troubleshooting

- **Metadata looks stale** — clear the relevant metadata category or run Sync Selected.
- **Query results look stale** — clear Queries for the active environment.
- **Indexer is not syncing** — check whether it is paused.
- **Environment is missing** — authenticate an account/environment pair in Client Management.
- **Wrong environment is active** — check the Client section of the taskbar or open Client Management.
- **Cache warning appears** — the environment cache could not be opened; the client is using in-memory cache.
- **Settings changed but old behavior remains** — reopen affected flows or clients so they are recreated with the new cache settings.

## Developer map

Important files:

- `dataverse-tui/src/systems/indexer/mod.rs` — indexer system, task queue, scheduler, request/event handlers.
- `dataverse-tui/src/systems/indexer/api.rs` — requests, events, settings, and cache categories.
- `dataverse-tui/src/systems/indexer/sync.rs` — sync task types and task execution.
- `dataverse-tui/src/systems/indexer/repository.rs` — indexer database persistence.
- `dataverse-tui/src/systems/indexer/modal.rs` — Indexer Dashboard UI.
- `dataverse-tui/src/systems/client_management/mod.rs` — opens per-environment SQLite caches and builds cache TTL config.
- `dataverse-tui/src/client_manager.rs` — Dataverse client pooling and cache-config invalidation.
- `dataverse-tui/src/paths.rs` — indexer and cache database paths.
- `dataverse-lib/src/cache/` — cache provider trait, SQLite cache implementation, and TTL configuration.
