//! Live find cache — in-memory record cache for find() resolution.
//!
//! Built from `ODataFetchModal` results. Each find cache entity's records
//! are stored and searched during transform execution.

use std::collections::HashMap;

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

/// In-memory find cache for resolving find() transforms against real data.
///
/// Records are indexed by entity name. Populated from fetch modal results
/// before transform execution begins.
#[derive(Default)]
pub struct LiveFindCache {
    /// Records indexed by entity logical name.
    records: HashMap<String, Vec<Record>>,
}

impl LiveFindCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self {
            records: HashMap::new(),
        }
    }

    /// Insert records for an entity into the cache.
    ///
    /// If records already exist for this entity, they are replaced.
    pub fn insert_records(&mut self, entity: impl Into<String>, records: Vec<Record>) {
        self.records.insert(entity.into(), records);
    }
}

impl FindCache for LiveFindCache {
    fn find_where(
        &self,
        entity: &str,
        conditions: &[(String, Value)],
    ) -> Result<Record, FindError> {
        let records = self
            .records
            .get(entity)
            .ok_or_else(|| FindError::NotCached(entity.to_string()))?;

        let mut matches: Vec<&Record> = Vec::new();

        for record in records {
            let all_match = conditions.iter().all(|(field, expected)| {
                match traverse_path(record, field) {
                    Some(actual) => values_equal(actual, expected),
                    // Null field matches Null condition
                    None => matches!(expected, Value::Null),
                }
            });

            if all_match {
                matches.push(record);
            }
        }

        match matches.len() {
            0 => Err(FindError::NotFound),
            1 => Ok(matches[0].clone()),
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
            let record_json =
                serde_json::to_value(record).map_err(|e| FindError::LuaError(e.to_string()))?;
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
        self.records
            .get(entity)?
            .iter()
            .find(|r| r.id() == Some(id))
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
                        Value::Record(Box::new(nested_contact.clone())),
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
                vec![("primarycontactid", Value::Record(Box::new(nested_contact)))],
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
