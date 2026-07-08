# Migration Transforms

Transforms are the declarative building blocks for variables, field mappings, match conditions, find conditions, and child branches.

Transforms run as an ordered chain. Each transform receives the previous transform's output as `#value`, and its result becomes the next `#value`. Field mappings and variables start with `#value = null` unless a parent transform scope sets another value.

## Path and placeholder syntax

Several transforms resolve paths.

Supported path forms:

- Source fields: `name`, `emailaddress1`
- Lookup navigation: `primarycontactid.fullname`
- Optional lookup navigation: `secondarycontactid?.fullname`
- Polymorphic lookup target: `ownerid[systemuser].domainname`
- Variables: `$prefix`, `$owner`
- Variable navigation: `$owner.domainname`, `$customer[account].name`
- System variables: `#value`, `#type`, `#index`, `#source_entity`, `#target_entity`
- System variable navigation: `#value.name` when `#value` is a lookup/record-like value
- Entity references: `/contact($contact_id)`
- Coalesce paths: `emailaddress1 ?? emailaddress2 ?? $fallback`

Optional navigation returns `null` instead of failing when the lookup is empty.

Entity-reference expressions wrap a GUID-like value as a Dataverse entity reference, which is useful for target lookup fields.

## Value transforms

### Copy

Copies a value from a source field, variable, system variable, entity-reference expression, or coalesce expression.

```text
Source:    { name: "Contoso", email1: null, email2: "sales@contoso.com" }
Transform: Copy "email1 ?? email2"
Output:    "sales@contoso.com"
```

### Constant

Returns a configured static value. It ignores the current `#value`.

```text
Transform: Constant "Imported"
Output:    "Imported"
```

### GUID

Generates a new random GUID. It ignores the current `#value`.

```text
Transform: GUID
Output:    7d89f6b7-5c2c-4a23-a6e8-0b3d1d6df7a4
```

## String transforms

### String Operations

Applies one string operation to `#value`. Chain multiple String Operations transforms for multiple steps.

Supported operations:

- uppercase,
- lowercase,
- trim,
- trim start,
- trim end,
- truncate.

```text
Input:     "  Hello World  "
Transform: Trim
Transform: Lowercase
Output:    "hello world"
```

### Format

Builds a string from a template. Placeholders in `{...}` resolve the same path syntax as Copy.

Date/time placeholders can add a strftime format after `|`.

```text
Source:    { firstname: "Ada", lastname: "Lovelace", createdon: 2025-03-15T10:30:00Z }
Transform: Format "{firstname} {lastname} ({createdon|%Y-%m-%d})"
Output:    "Ada Lovelace (2025-03-15)"
```

### Replace

Replaces text in string `#value`. The `from` pattern can be literal text or a regular expression.

```text
Input:     "555-123-4567"
Transform: Replace from "-" to "" regex false
Output:    "5551234567"

Input:     "hello    world"
Transform: Replace from "\\s+" to " " regex true
Output:    "hello world"
```

## Type conversion transforms

### Convert

Converts `#value` to one of:

- `int`
- `decimal`
- `string`
- `bool`

Aliases are accepted: `integer`, `number`, `text`, and `boolean`.

```text
Input:     "45.7"
Transform: Convert int
Output:    45

Input:     "yes"
Transform: Convert bool
Output:    true
```

### Parse Integer

Parses string `#value` as a 32-bit integer. Invalid or non-string values return `null` and log a warning.

```text
Input:     "  -456  "
Transform: Parse Integer
Output:    -456
```

### Parse Decimal

Parses string `#value` as a decimal value. Invalid strings produce a parse error.

```text
Input:     "123.45"
Transform: Parse Decimal
Output:    123.45
```

### Parse Date

Parses string `#value` as a UTC datetime using a strftime format string. Date-only inputs become midnight UTC.

```text
Input:     "15/01/2024 14:30"
Transform: Parse Date "%d/%m/%Y %H:%M"
Output:    2024-01-15T14:30:00Z
```

## Data transforms

### Value Map

Maps option-set integer values from a source option set to target option-set integer values.

Unmapped single values return `null`; unmapped multi-select values are skipped.

```text
Input:     1
Mappings:  1 -> 100000000, 2 -> 100000001
Output:    100000000
```

### Math

Applies arithmetic to numeric `#value`.

Supported operations:

- add,
- subtract,
- multiply,
- divide,
- round.

Non-decimal numeric inputs return floating-point results; decimal inputs keep decimal arithmetic.

```text
Input:     10
Transform: Math Add 5
Output:    15.0

Input:     3.14159
Transform: Math Round 2
Output:    3.14
```

## Control-flow transforms

### Guard

Checks a condition. If the condition is true, it runs the guard fallback chain, returns that value, and exits the current chain early. If false, it passes `#value` through unchanged.

```text
Input:     null
Condition: IsNull(#value)
Fallback:  Constant "N/A"
Output:    "N/A"
```

### Coalesce

Runs alternative child chains in order and returns the first non-null result. Each alternative starts from the same saved `#value`. If every alternative returns `null`, the transform errors.

```text
Alternative 1: Copy "email1"        -> null
Alternative 2: Copy "email2"        -> "sales@contoso.com"
Output:       "sales@contoso.com"
```

### Match

Evaluates branch conditions in order. The first matching branch runs its child chain. If no branch matches, the default chain runs when configured; otherwise the transform errors.

```text
Input:     2
Branch 1:  #value == 1 -> Constant "Active"
Branch 2:  #value == 2 -> Constant "Inactive"
Default:   Constant "Unknown"
Output:    "Inactive"
```

### Find

Looks up a record in the target environment and returns it as a record value.

Find modes:

- **Where** — each condition has a target field and a source transform chain that produces the value to match.
- **Lua** — a Lua script returns the target record ID.

Find fallbacks:

- error,
- null,
- default transform chain.

```text
Entity:     systemuser
Mode:       Where
Condition:  target internalemailaddress = Copy "owner_email"
Source:     { owner_email: "ada@example.com" }
Output:     Record(systemuser, internalemailaddress = "ada@example.com")
```

## Conditions

Guards and Match branches use condition expressions.

Available condition shapes include:

- AND / OR / NOT,
- comparisons: equal, not equal, less/greater variants,
- is null / is not null,
- contains,
- starts with,
- ends with.

Expressions inside conditions can reference:

- source paths,
- variables,
- system variables,
- literal constants.

## Type checking in the editor

The editor tracks transform output types where possible. It uses metadata from the source and target environments, variable declared types, and lookup path navigation.

Warnings are shown when:

- a variable chain output is incompatible with the variable's declared type,
- a field mapping chain output is incompatible with the target field type,
- a path is ambiguous or cannot be resolved from cached metadata.

Type warnings help author mappings, but execution still depends on actual transform results and Dataverse validation.
