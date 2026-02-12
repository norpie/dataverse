//! Live find cache — in-memory record cache for find() resolution.
//!
//! Built from `ODataFetchModal` results. Each find cache entity's records
//! are stored and searched during transform execution.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use dataverse_lib::model::Record;
use dataverse_lib::model::Value;
use mlua::Table;
use uuid::Uuid;

use crate::apps::migration::engine::util::traverse_path;
use crate::apps::migration::engine::util::values_equal;
use crate::apps::migration::engine::FindCache;
use crate::apps::migration::engine::FindError;
use crate::lua::runtime::LuaRuntime;

// =============================================================================
// LiveFindCache
// =============================================================================

/// Key for the single-field index: an owned Value that can be hashed.
///
/// We wrap the common matchable value types into a hashable key.
/// Unsupported types fall back to linear scan.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum IndexKey {
    Null,
    Int(i32),
    Long(i64),
    Bool(bool),
    String(String),
    Guid(Uuid),
    OptionSet(i32),
}

impl IndexKey {
    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Null => Some(Self::Null),
            Value::Int(v) => Some(Self::Int(*v)),
            Value::Long(v) => Some(Self::Long(*v)),
            Value::Bool(v) => Some(Self::Bool(*v)),
            Value::String(v) => Some(Self::String(v.clone())),
            Value::Guid(v) => Some(Self::Guid(*v)),
            Value::OptionSet(v) => Some(Self::OptionSet(v.value)),
            _ => None,
        }
    }

    /// Check if this key matches a Value, including cross-type equality.
    fn matches(&self, value: &Value) -> bool {
        match (self, value) {
            (Self::Null, Value::Null) => true,
            (Self::Int(a), Value::Int(b)) => a == b,
            (Self::Int(a), Value::OptionSet(b)) => *a == b.value,
            (Self::Int(a), Value::Decimal(b)) => {
                b.is_integer() && rust_decimal::Decimal::from(*a) == *b
            }
            (Self::Long(a), Value::Long(b)) => a == b,
            (Self::Long(a), Value::Int(b)) => *a == (*b as i64),
            (Self::Int(a), Value::Long(b)) => (*a as i64) == *b,
            (Self::Bool(a), Value::Bool(b)) => a == b,
            (Self::String(a), Value::String(b)) => a == b,
            (Self::Guid(a), Value::Guid(b)) => a == b,
            (Self::OptionSet(a), Value::OptionSet(b)) => *a == b.value,
            (Self::OptionSet(a), Value::Int(b)) => *a == *b,
            (Self::OptionSet(a), Value::Decimal(b)) => {
                b.is_integer() && rust_decimal::Decimal::from(*a) == *b
            }
            _ => false,
        }
    }
}

/// Lazily-built single-field index for an entity.
type FieldIndex = HashMap<IndexKey, Vec<usize>>;

/// In-memory find cache for resolving find() transforms against real data.
///
/// Records are stored as `Arc<Record>` to avoid expensive deep clones
/// when returning matches from `find_where`. The Arc is cheaply cloneable,
/// and the underlying Record data is shared across all references.
///
/// Indexed by entity name with a secondary UUID index for O(1)
/// lookups by ID and lazy single-field indexes for fast `find_where`.
/// Populated from fetch modal results before transform execution begins.
pub struct LiveFindCache {
    /// Records indexed by entity logical name, wrapped in Arc for cheap sharing.
    records: HashMap<String, Vec<Arc<Record>>>,
    /// Secondary index: entity → (UUID → index into records vec).
    id_index: HashMap<String, HashMap<Uuid, usize>>,
    /// Lazy single-field indexes: (entity, field) → (value → record indices).
    /// Built on first `find_where` call for a given entity+field combo.
    field_indexes: Mutex<HashMap<(String, String), FieldIndex>>,
}

impl Default for LiveFindCache {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveFindCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self {
            records: HashMap::new(),
            id_index: HashMap::new(),
            field_indexes: Mutex::new(HashMap::new()),
        }
    }

    /// Insert records for an entity into the cache.
    ///
    /// Wraps each record in `Arc` for cheap sharing. Builds a UUID index
    /// for O(1) lookups by ID. Clears any lazily-built field indexes for
    /// this entity. If records already exist for this entity, they are replaced.
    pub fn insert_records(&mut self, entity: impl Into<String>, records: Vec<Record>) {
        let entity = entity.into();
        let arc_records: Vec<Arc<Record>> = records.into_iter().map(Arc::new).collect();
        let mut index = HashMap::with_capacity(arc_records.len());
        for (i, record) in arc_records.iter().enumerate() {
            if let Some(id) = record.id() {
                index.insert(id, i);
            }
        }
        self.id_index.insert(entity.clone(), index);
        // Clear stale field indexes for this entity
        if let Ok(mut fi) = self.field_indexes.lock() {
            fi.retain(|(e, _), _| e != &entity);
        }
        self.records.insert(entity, arc_records);
    }

    /// Get or build a single-field index for an entity+field combination.
    ///
    /// Returns candidate record indices for the given value, or None if the
    /// field contains non-indexable types (falls back to linear scan).
    fn indexed_lookup(&self, entity: &str, field: &str, value: &Value) -> Option<Vec<usize>> {
        let key = (entity.to_string(), field.to_string());
        let mut fi = self.field_indexes.lock().ok()?;

        if !fi.contains_key(&key) {
            // Build the index
            let records = self.records.get(entity)?;
            let mut index: FieldIndex = HashMap::new();
            for (i, record) in records.iter().enumerate() {
                let val = record.get(field).unwrap_or(&Value::Null);
                if let Some(ik) = IndexKey::from_value(val) {
                    index.entry(ik).or_default().push(i);
                } else {
                    // Non-indexable value type in this field — abandon indexing
                    return None;
                }
            }
            fi.insert(key.clone(), index);
        }

        let index = fi.get(&key)?;
        let lookup_key = IndexKey::from_value(value)?;

        // Look for exact match first, then cross-type matches
        if let Some(indices) = index.get(&lookup_key) {
            return Some(indices.clone());
        }

        // Cross-type: scan index keys for matches (e.g., Int vs OptionSet)
        let mut results = Vec::new();
        for (ik, indices) in index {
            if ik.matches(value) {
                results.extend(indices);
            }
        }
        if results.is_empty() {
            None
        } else {
            Some(results)
        }
    }
}

impl FindCache for LiveFindCache {
    fn find_where(
        &self,
        entity: &str,
        conditions: &[(String, Value)],
    ) -> Result<Arc<Record>, FindError> {
        let records = self
            .records
            .get(entity)
            .ok_or_else(|| FindError::NotCached(entity.to_string()))?;

        // Try to use a field index for the first simple (non-dotted) condition
        let indexed_condition = conditions
            .iter()
            .position(|(field, _)| !field.contains('.'));

        let candidate_indices: Option<Vec<usize>> = indexed_condition.and_then(|pos| {
            let (field, value) = &conditions[pos];
            self.indexed_lookup(entity, field, value)
        });

        let mut matches: Vec<usize> = Vec::new();

        log::trace!(
            "find_where: entity={}, conditions={:?}, indexed_condition={:?}, candidates={:?}",
            entity,
            conditions
                .iter()
                .map(|(f, v)| format!("{}={:?}", f, v))
                .collect::<Vec<_>>(),
            indexed_condition,
            candidate_indices.as_ref().map(|c| c.len()),
        );

        if let Some(candidates) = candidate_indices {
            // Filter candidates against remaining conditions
            for &idx in &candidates {
                let record = &records[idx];
                let all_match = conditions.iter().enumerate().all(|(i, (field, expected))| {
                    if Some(i) == indexed_condition {
                        return true; // already matched by index
                    }
                    let actual = traverse_path(record, field);
                    let eq = match actual {
                        Some(actual) => values_equal(actual, expected),
                        None => matches!(expected, Value::Null),
                    };
                    if !eq {
                        log::trace!(
                            "find_where: candidate {} field={} expected={:?} actual={:?} → mismatch",
                            idx, field, expected, actual
                        );
                    }
                    eq
                });
                if all_match {
                    matches.push(idx);
                }
            }
        } else {
            // No index available — full linear scan
            for (i, record) in records.iter().enumerate() {
                let all_match =
                    conditions
                        .iter()
                        .all(|(field, expected)| match traverse_path(record, field) {
                            Some(actual) => values_equal(actual, expected),
                            None => matches!(expected, Value::Null),
                        });
                if all_match {
                    matches.push(i);
                }
            }
        }

        match matches.len() {
            0 => Err(FindError::NotFound),
            1 => Ok(Arc::clone(&records[matches[0]])),
            n => Err(FindError::Multiple(n)),
        }
    }

    fn find_lua(
        &self,
        entity: &str,
        script: &str,
        source_record: &Record,
    ) -> Result<Uuid, FindError> {
        let records = self
            .records
            .get(entity)
            .ok_or_else(|| FindError::NotCached(entity.to_string()))?;

        let runtime = LuaRuntime::new().map_err(|e| FindError::LuaError(e.to_string()))?;

        // Load the script — expects it to return a module table with M.resolve()
        let module: Table = runtime
            .load(script)
            .map_err(|e| FindError::LuaError(e.to_string()))?;

        // Convert source record to Lua table
        let source_json =
            serde_json::to_value(source_record).map_err(|e| FindError::LuaError(e.to_string()))?;
        let source_lua = runtime
            .json_to_lua(&source_json)
            .map_err(|e| FindError::LuaError(e.to_string()))?;

        // Convert cached records to a Lua array table
        let target_table = runtime
            .create_table()
            .map_err(|e| FindError::LuaError(e.to_string()))?;
        for (i, record) in records.iter().enumerate() {
            let record_json = serde_json::to_value(record.as_ref())
                .map_err(|e| FindError::LuaError(e.to_string()))?;
            let record_lua = runtime
                .json_to_lua(&record_json)
                .map_err(|e| FindError::LuaError(e.to_string()))?;
            target_table
                .set(i + 1, record_lua)
                .map_err(|e| FindError::LuaError(e.to_string()))?;
        }

        // Call M.resolve(source, target)
        let resolve: mlua::Function = module
            .get("resolve")
            .map_err(|e| FindError::LuaError(format!("Script missing M.resolve(): {e}")))?;

        let result: Table = resolve
            .call((source_lua, target_table))
            .map_err(|e| FindError::LuaError(format!("M.resolve() failed: {e}")))?;

        // Parse result: { target = "guid" } or { error = "msg" }
        if let Ok(error_msg) = result.get::<mlua::String>("error") {
            return Err(FindError::LuaError(
                error_msg
                    .to_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|_| "unknown".to_string()),
            ));
        }

        let guid_str: mlua::String = result
            .get("target")
            .map_err(|e| FindError::LuaError(format!("Result missing 'target' field: {e}")))?;

        let guid_str = guid_str
            .to_str()
            .map_err(|e| FindError::LuaError(format!("Invalid UTF-8 in target GUID: {e}")))?;

        guid_str
            .parse::<Uuid>()
            .map_err(|e| FindError::LuaError(format!("Invalid GUID '{guid_str}': {e}")))
    }

    fn get(&self, entity: &str, id: Uuid) -> Option<&Record> {
        let idx = *self.id_index.get(entity)?.get(&id)?;
        self.records.get(entity)?.get(idx).map(|arc| arc.as_ref())
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use dataverse_lib::model::types::OptionSetValue;
    use dataverse_lib::model::Entity;

    fn make_record(entity: &str, id: Uuid, fields: Vec<(&str, Value)>) -> Record {
        let mut record = Record::with_id(Entity::logical(entity), id);
        for (field, value) in fields {
            record = record.set(field, value);
        }
        record
    }

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    // ---- find_where tests ----

    #[test]
    fn find_where_single_match() {
        let mut cache = LiveFindCache::new();
        cache.insert_records(
            "contact",
            vec![
                make_record("contact", id(1), vec![("fullname", Value::from("Alice"))]),
                make_record("contact", id(2), vec![("fullname", Value::from("Bob"))]),
            ],
        );

        let result = cache.find_where("contact", &[("fullname".into(), Value::from("Bob"))]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id(), Some(id(2)));
    }

    #[test]
    fn find_where_no_match() {
        let mut cache = LiveFindCache::new();
        cache.insert_records(
            "contact",
            vec![make_record(
                "contact",
                id(1),
                vec![("fullname", Value::from("Alice"))],
            )],
        );

        let result = cache.find_where("contact", &[("fullname".into(), Value::from("Charlie"))]);
        assert!(matches!(result, Err(FindError::NotFound)));
    }

    #[test]
    fn find_where_multiple_matches() {
        let mut cache = LiveFindCache::new();
        cache.insert_records(
            "contact",
            vec![
                make_record("contact", id(1), vec![("city", Value::from("NYC"))]),
                make_record("contact", id(2), vec![("city", Value::from("NYC"))]),
            ],
        );

        let result = cache.find_where("contact", &[("city".into(), Value::from("NYC"))]);
        assert!(matches!(result, Err(FindError::Multiple(2))));
    }

    #[test]
    fn find_where_not_cached() {
        let cache = LiveFindCache::new();
        let result = cache.find_where("contact", &[("name".into(), Value::from("x"))]);
        assert!(matches!(result, Err(FindError::NotCached(_))));
    }

    #[test]
    fn find_where_multiple_conditions() {
        let mut cache = LiveFindCache::new();
        cache.insert_records(
            "contact",
            vec![
                make_record(
                    "contact",
                    id(1),
                    vec![
                        ("firstname", Value::from("Alice")),
                        ("lastname", Value::from("Smith")),
                    ],
                ),
                make_record(
                    "contact",
                    id(2),
                    vec![
                        ("firstname", Value::from("Alice")),
                        ("lastname", Value::from("Jones")),
                    ],
                ),
            ],
        );

        let result = cache.find_where(
            "contact",
            &[
                ("firstname".into(), Value::from("Alice")),
                ("lastname".into(), Value::from("Jones")),
            ],
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id(), Some(id(2)));
    }

    #[test]
    fn find_where_cross_type_equality() {
        let mut cache = LiveFindCache::new();
        cache.insert_records(
            "account",
            vec![make_record(
                "account",
                id(1),
                vec![(
                    "statecode",
                    Value::OptionSet(OptionSetValue {
                        value: 0,
                        label: Some("Active".into()),
                    }),
                )],
            )],
        );

        // Int matches OptionSet via values_equal
        let result = cache.find_where("account", &[("statecode".into(), Value::Int(0))]);
        assert!(result.is_ok());
    }

    #[test]
    fn find_where_null_condition_matches_missing_field() {
        let mut cache = LiveFindCache::new();
        cache.insert_records(
            "contact",
            vec![
                make_record("contact", id(1), vec![("name", Value::from("Alice"))]),
                make_record("contact", id(2), vec![]),
            ],
        );

        // Null condition should match record with missing field
        let result = cache.find_where("contact", &[("name".into(), Value::Null)]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id(), Some(id(2)));
    }

    #[test]
    fn find_where_empty_conditions_matches_all() {
        let mut cache = LiveFindCache::new();
        cache.insert_records(
            "contact",
            vec![
                make_record("contact", id(1), vec![]),
                make_record("contact", id(2), vec![]),
            ],
        );

        // No conditions → all match → Multiple
        let result = cache.find_where("contact", &[]);
        assert!(matches!(result, Err(FindError::Multiple(2))));
    }

    #[test]
    fn find_where_guid_condition() {
        let target_id = Uuid::new_v4();
        let mut cache = LiveFindCache::new();
        cache.insert_records(
            "account",
            vec![
                make_record("account", id(1), vec![("ownerid", Value::Guid(target_id))]),
                make_record(
                    "account",
                    id(2),
                    vec![("ownerid", Value::Guid(Uuid::new_v4()))],
                ),
            ],
        );

        let result = cache.find_where("account", &[("ownerid".into(), Value::Guid(target_id))]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id(), Some(id(1)));
    }

    // ---- get tests ----

    #[test]
    fn get_existing_record() {
        let mut cache = LiveFindCache::new();
        cache.insert_records(
            "contact",
            vec![
                make_record("contact", id(1), vec![("name", Value::from("Alice"))]),
                make_record("contact", id(2), vec![("name", Value::from("Bob"))]),
            ],
        );

        let record = cache.get("contact", id(2));
        assert!(record.is_some());
        assert_eq!(record.unwrap().id(), Some(id(2)));
    }

    #[test]
    fn get_missing_record() {
        let mut cache = LiveFindCache::new();
        cache.insert_records("contact", vec![make_record("contact", id(1), vec![])]);

        assert!(cache.get("contact", id(99)).is_none());
    }

    #[test]
    fn get_missing_entity() {
        let cache = LiveFindCache::new();
        assert!(cache.get("contact", id(1)).is_none());
    }

    // ---- insert_records tests ----

    #[test]
    fn insert_records_replaces_existing() {
        let mut cache = LiveFindCache::new();
        cache.insert_records(
            "contact",
            vec![make_record(
                "contact",
                id(1),
                vec![("name", Value::from("Alice"))],
            )],
        );
        cache.insert_records(
            "contact",
            vec![make_record(
                "contact",
                id(2),
                vec![("name", Value::from("Bob"))],
            )],
        );

        // Old record gone
        assert!(cache.get("contact", id(1)).is_none());
        // New record present
        assert!(cache.get("contact", id(2)).is_some());
    }

    // ---- find_lua tests ----

    #[test]
    fn find_lua_resolves_by_field_match() {
        let target_id = id(42);
        let mut cache = LiveFindCache::new();
        cache.insert_records(
            "contact",
            vec![
                make_record(
                    "contact",
                    id(1),
                    vec![
                        ("email", Value::from("a@x.com")),
                        ("contactid", Value::Guid(id(1))),
                    ],
                ),
                make_record(
                    "contact",
                    target_id,
                    vec![
                        ("email", Value::from("b@x.com")),
                        ("contactid", Value::Guid(target_id)),
                    ],
                ),
            ],
        );

        let source = make_record(
            "account",
            id(100),
            vec![("contact_email", Value::from("b@x.com"))],
        );

        let script = r#"
            local M = {}
            function M.resolve(source, target)
                for _, record in ipairs(target) do
                    if record.email == source.contact_email then
                        return { target = record.contactid }
                    end
                end
                return { error = "no match" }
            end
            return M
        "#;

        let result = cache.find_lua("contact", script, &source);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), target_id);
    }

    #[test]
    fn find_lua_returns_error_from_script() {
        let mut cache = LiveFindCache::new();
        cache.insert_records("contact", vec![]);

        let source = make_record("account", id(1), vec![]);

        let script = r#"
            local M = {}
            function M.resolve(source, target)
                return { error = "custom error message" }
            end
            return M
        "#;

        let result = cache.find_lua("contact", script, &source);
        assert!(matches!(result, Err(FindError::LuaError(msg)) if msg == "custom error message"));
    }

    #[test]
    fn find_lua_missing_resolve_function() {
        let mut cache = LiveFindCache::new();
        cache.insert_records("contact", vec![]);

        let source = make_record("account", id(1), vec![]);
        let script = "local M = {} return M";

        let result = cache.find_lua("contact", script, &source);
        assert!(matches!(result, Err(FindError::LuaError(msg)) if msg.contains("M.resolve()")));
    }

    #[test]
    fn find_lua_not_cached() {
        let cache = LiveFindCache::new();
        let source = make_record("account", id(1), vec![]);

        let result = cache.find_lua("contact", "return {}", &source);
        assert!(matches!(result, Err(FindError::NotCached(_))));
    }

    // ---- find_where with dotted paths ----

    #[test]
    fn find_where_dotted_path_traverses_nested_record() {
        let nested_contact = Record::with_id(Entity::logical("contact"), id(100))
            .set("emailaddress1", Value::from("alice@example.com"));

        let mut cache = LiveFindCache::new();
        cache.insert_records(
            "account",
            vec![
                make_record(
                    "account",
                    id(1),
                    vec![(
                        "primarycontactid",
                        Value::Record(Arc::new(nested_contact.clone())),
                    )],
                ),
                make_record("account", id(2), vec![("name", Value::from("NoContact"))]),
            ],
        );

        let result = cache.find_where(
            "account",
            &[(
                "primarycontactid.emailaddress1".into(),
                Value::from("alice@example.com"),
            )],
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id(), Some(id(1)));
    }

    #[test]
    fn find_where_dotted_path_no_match() {
        let nested_contact = Record::with_id(Entity::logical("contact"), id(100))
            .set("emailaddress1", Value::from("alice@example.com"));

        let mut cache = LiveFindCache::new();
        cache.insert_records(
            "account",
            vec![make_record(
                "account",
                id(1),
                vec![("primarycontactid", Value::Record(Arc::new(nested_contact)))],
            )],
        );

        let result = cache.find_where(
            "account",
            &[(
                "primarycontactid.emailaddress1".into(),
                Value::from("bob@example.com"),
            )],
        );
        assert!(matches!(result, Err(FindError::NotFound)));
    }

    #[test]
    fn find_where_dotted_path_missing_nested_record() {
        let mut cache = LiveFindCache::new();
        cache.insert_records(
            "account",
            vec![make_record(
                "account",
                id(1),
                vec![("name", Value::from("Acme"))],
            )],
        );

        // Account has no "primarycontactid" field → traversal returns None → no match
        let result = cache.find_where(
            "account",
            &[(
                "primarycontactid.emailaddress1".into(),
                Value::from("alice@example.com"),
            )],
        );
        assert!(matches!(result, Err(FindError::NotFound)));
    }

    #[test]
    fn find_where_dotted_path_null_condition_matches_missing() {
        let mut cache = LiveFindCache::new();
        cache.insert_records(
            "account",
            vec![make_record(
                "account",
                id(1),
                vec![("name", Value::from("Acme"))],
            )],
        );

        // Null condition on dotted path should match when nested record is missing
        let result = cache.find_where(
            "account",
            &[("primarycontactid.emailaddress1".into(), Value::Null)],
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id(), Some(id(1)));
    }
}
