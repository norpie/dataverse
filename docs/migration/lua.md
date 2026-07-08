# Migration Lua

Lua is available when declarative mappings are not expressive enough.

There are four Lua entry points:

- Lua match strategy
- Lua Find transform
- entity-level Lua mapping
- phase-level Lua

Use declarative mappings first when possible. Lua is best for complex matching, heavily custom data shaping, or phase-level operations that do not fit the normal source-to-target comparison model.

## Common shape

Lua scripts are loaded as modules and usually return a table named `M`.

Most migration Lua uses two functions:

- `M.declare()` — tells the migration pipeline what entities and fields to fetch.
- `M.resolve(source, target)` — performs the script logic.

The exact return shape depends on the Lua entry point.

## M.declare()

`M.declare()` is used during analysis to build fetch tasks before the script is executed.

It can declare:

- primary source fields,
- primary target fields,
- extra source-side entities,
- extra target-side entities.

A typical declaration looks like:

```lua
local M = {}

function M.declare()
    return {
        source = "account",
        target = "account",
        source_fields = { "accountid", "name", "emailaddress1" },
        target_fields = { "accountid", "name", "emailaddress1" },
        source_entities = {
            contact = { "contactid", "fullname" },
        },
        target_entities = {
            systemuser = { "systemuserid", "internalemailaddress" },
        },
    }
end

return M
```

The pipeline uses this declaration to fetch the requested data and then passes entity-keyed tables to `M.resolve(source, target)`.

## Lua match strategy

Lua match strategy is configured on an entity mapping's Match Config.

Use it when matching cannot be expressed with Same ID or declarative Find conditions.

The Lua match script builds a source-record to target-record mapping. Matched records are compared normally afterwards: declarative field mappings still produce desired target fields, and the comparison engine still decides update/skip/error.

Use this for complex target lookup rules while keeping field transformation declarative.

## Lua Find transform

A Find transform can run in Lua mode. It is still a transform: it receives the current source record, fetched target cache data, and returns the target record ID or fallback behavior.

Use Lua Find when only one lookup resolution is complex, but the rest of the mapping is declarative.

A Lua Find transform can use `M.declare()` to request source fields and target fields needed by the script.

## Entity-level Lua mapping

An entity mapping can be switched to Lua mode. In this mode, the entity script replaces declarative variables, field mappings, and match configuration for that entity mapping.

The script receives source and target entity tables and returns desired fields per source record. It may also return an explicit target GUID for a source record.

Entity Lua result shape:

```lua
function M.resolve(source, target)
    return {
        results = {
            ["source-guid"] = {
                target = "target-guid", -- optional; nil/missing means create
                fields = {
                    name = "New name",
                    emailaddress1 = "hello@example.com",
                },
            },
        },
    }
end
```

Rules:

- A result keyed by a source record ID represents that source record.
- `target = "..."` means update/compare against that target record.
- missing or nil `target` means create if the no-match behavior allows it.
- a source record missing from `results` is intentionally ignored.
- `{ error = "message" }` on a result marks that source record as an error.

Entity Lua can also return independent creates: `results` entries keyed by GUIDs that are not source record IDs. These become create/update/skip comparisons against the target environment by that GUID.

Entity Lua may return optional exports:

```lua
return {
    results = results,
    exports = {
        {
            name = "review",
            headers = { "id", "name" },
            rows = {
                { "...", "Contoso" },
            },
        },
    },
}
```

## Phase-level Lua

A phase can be switched to Lua mode. Phase-level Lua bypasses the declarative comparison pipeline entirely.

The script returns explicit operations:

```lua
function M.resolve(source, target)
    return {
        operations = {
            {
                op = "create",
                entity = "account",
                id = "11111111-1111-1111-1111-111111111111",
                fields = { name = "New account" },
            },
            {
                op = "update",
                entity = "account",
                id = "22222222-2222-2222-2222-222222222222",
                fields = { name = "Updated account" },
            },
        },
    }
end
```

Supported phase-level operations:

- `create`
- `update`
- `activate`
- `deactivate`
- `delete`
- `associate`
- `disassociate`

Associate/disassociate shape:

```lua
{
    op = "associate",
    entity1 = "account",
    id1 = "11111111-1111-1111-1111-111111111111",
    entity2 = "contact",
    id2 = "22222222-2222-2222-2222-222222222222",
    relationship = "account_primary_contact",
}
```

Phase-level Lua operations are grouped into the same execution passes as declarative operations, then submitted to the Queue.

## Lookup values in Lua fields

Lua field values can include Dataverse lookup/entity binding values when parsed by the Lua runtime. The execution layer converts lookup-like values to Dataverse `@odata.bind` payloads using target metadata.

For phase-level create/update operations, lookup fields are included directly. They are not deferred to the later Update pass the way declarative Create operations are.

## Choosing a Lua level

Use the smallest Lua scope that solves the problem:

- **Lua Find** — only lookup resolution is custom.
- **Lua match strategy** — matching is custom, field transformations are still declarative.
- **Entity-level Lua** — one entity's desired output requires custom logic.
- **Phase-level Lua** — the phase is operation-driven rather than source-to-target comparison-driven.
