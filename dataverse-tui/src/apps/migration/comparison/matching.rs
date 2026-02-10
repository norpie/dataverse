//! Target matching — match each source record to a target record.
//!
//! Supports two strategies:
//! - **SameID**: Match by primary key (source ID == target ID).
//! - **Find**: Execute match condition source chains, then scan target records
//!   for matches using `values_equal` + `traverse_path`.

use dataverse_lib::model::Entity;
use dataverse_lib::model::Record;
use dataverse_lib::model::Value;

use crate::apps::migration::engine::execute_chain;
use crate::apps::migration::engine::util::traverse_path;
use crate::apps::migration::engine::util::values_equal;
use crate::apps::migration::engine::ChainItem;
use crate::apps::migration::engine::FindCache;
use crate::apps::migration::engine::SystemVars;
use crate::apps::migration::engine::TransformContext;
use crate::apps::migration::engine::TransformResult;
use crate::apps::migration::types::MatchStrategy;

// =============================================================================
// Types
// =============================================================================

/// Input for target matching.
pub struct MatchInput<'a> {
    /// The source record being matched.
    pub source_record: &'a Record,
    /// Match strategy (SameId or Find).
    pub strategy: MatchStrategy,
    /// Source entity primary key field name.
    pub source_primary_key: &'a str,
    /// Target entity primary key field name.
    pub target_primary_key: &'a str,
    /// For Find strategy: (target_field, source chain) pairs.
    /// The source chain is executed against the source record to get the
    /// expected value, which is then compared against the target_field
    /// (possibly dotted) on each target record.
    pub match_conditions: &'a [(String, Vec<ChainItem>)],
    /// Source entity logical name.
    pub source_entity: &'a str,
    /// Target entity logical name.
    pub target_entity: &'a str,
    /// Find cache for resolving find() transforms within match condition chains.
    pub find_cache: &'a dyn FindCache,
}

/// Result of attempting to match a source record to a target record.
#[derive(Debug)]
pub enum MatchResult {
    /// Exactly one target record matched.
    Found(Record),
    /// No target record matched.
    NotFound,
    /// Multiple target records matched (ambiguous).
    Multiple(usize),
    /// An error occurred during matching.
    Error(String),
}

// =============================================================================
// Matching Logic
// =============================================================================

/// Match a source record to a target record.
///
/// Uses the strategy specified in `input`:
/// - **SameId**: Look up source primary key in target records.
/// - **Find**: Execute match condition chains, scan targets for matches.
pub fn match_target(input: &MatchInput<'_>, target_records: &[Record]) -> MatchResult {
    match input.strategy {
        MatchStrategy::SameId => match_same_id(input, target_records),
        MatchStrategy::Find => match_find(input, target_records),
    }
}

/// SameID matching: find a target record with the same primary key as the source.
fn match_same_id(input: &MatchInput<'_>, target_records: &[Record]) -> MatchResult {
    // Get source record's primary key value
    let source_pk_value = match input.source_record.id() {
        Some(id) => id,
        None => {
            // Try getting from field name
            match input.source_record.get(input.source_primary_key) {
                Some(Value::Guid(id)) => *id,
                _ => {
                    return MatchResult::Error(format!(
                        "Source record missing primary key '{}'",
                        input.source_primary_key
                    ));
                }
            }
        }
    };

    // Scan target records for matching ID
    for target in target_records {
        let target_id = match target.id() {
            Some(id) => id,
            None => match target.get(input.target_primary_key) {
                Some(Value::Guid(id)) => *id,
                _ => continue,
            },
        };

        if source_pk_value == target_id {
            return MatchResult::Found(target.clone());
        }
    }

    MatchResult::NotFound
}

/// Find-based matching: execute match condition chains, scan targets.
fn match_find(input: &MatchInput<'_>, target_records: &[Record]) -> MatchResult {
    // Step 1: Execute each match condition's source chain to get expected values
    let mut conditions: Vec<(&str, Value)> = Vec::new();

    let system_vars = SystemVars::new(
        Entity::logical(input.source_entity),
        Entity::logical(input.target_entity),
        0,
    );

    for (target_field, source_chain) in input.match_conditions {
        let mut ctx = TransformContext {
            source_record: input.source_record,
            variables: &std::collections::HashMap::new(),
            system_vars: system_vars.clone(),
            find_cache: input.find_cache,
        };

        match execute_chain(source_chain, &mut ctx) {
            TransformResult::Value(v) | TransformResult::Exit(v) => {
                conditions.push((target_field.as_str(), v));
            }
            TransformResult::Error(e) => {
                return MatchResult::Error(format!(
                    "Match condition chain for '{}' failed: {:?}",
                    target_field, e
                ));
            }
        }
    }

    // Step 2: Scan target records, checking all conditions
    let mut matches: Vec<&Record> = Vec::new();

    for target in target_records {
        let all_match =
            conditions
                .iter()
                .all(|(field, expected)| match traverse_path(target, field) {
                    Some(actual) => values_equal(actual, expected),
                    None => matches!(expected, Value::Null),
                });

        if all_match {
            matches.push(target);
        }
    }

    match matches.len() {
        0 => MatchResult::NotFound,
        1 => MatchResult::Found(matches[0].clone()),
        n => MatchResult::Multiple(n),
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apps::migration::engine::StubFindCache;
    use crate::apps::migration::types::TransformData;
    use uuid::Uuid;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn make_record(entity: &str, uuid: Uuid, fields: Vec<(&str, Value)>) -> Record {
        let mut record = Record::with_id(Entity::logical(entity), uuid);
        for (field, value) in fields {
            record = record.set(field, value);
        }
        record
    }

    fn default_input<'a>(
        source: &'a Record,
        strategy: MatchStrategy,
        conditions: &'a [(String, Vec<ChainItem>)],
    ) -> MatchInput<'a> {
        static STUB: StubFindCache = StubFindCache;
        MatchInput {
            source_record: source,
            strategy,
            source_primary_key: "accountid",
            target_primary_key: "accountid",
            match_conditions: conditions,
            source_entity: "account",
            target_entity: "account",
            find_cache: &STUB,
        }
    }

    // ---- SameID tests ----

    #[test]
    fn same_id_match() {
        let source = make_record("account", id(1), vec![("name", Value::from("Acme"))]);
        let targets = vec![
            make_record("account", id(2), vec![("name", Value::from("Other"))]),
            make_record("account", id(1), vec![("name", Value::from("Acme Target"))]),
        ];

        let input = default_input(&source, MatchStrategy::SameId, &[]);
        let result = match_target(&input, &targets);

        assert!(matches!(result, MatchResult::Found(ref r) if r.id() == Some(id(1))));
    }

    #[test]
    fn same_id_no_match() {
        let source = make_record("account", id(1), vec![]);
        let targets = vec![
            make_record("account", id(2), vec![]),
            make_record("account", id(3), vec![]),
        ];

        let input = default_input(&source, MatchStrategy::SameId, &[]);
        let result = match_target(&input, &targets);

        assert!(matches!(result, MatchResult::NotFound));
    }

    #[test]
    fn same_id_empty_targets() {
        let source = make_record("account", id(1), vec![]);
        let input = default_input(&source, MatchStrategy::SameId, &[]);
        let result = match_target(&input, &[]);

        assert!(matches!(result, MatchResult::NotFound));
    }

    // ---- Find tests ----

    #[test]
    fn find_single_condition_match() {
        let source = make_record("account", id(1), vec![("name", Value::from("Acme"))]);
        let targets = vec![
            make_record("account", id(10), vec![("name", Value::from("Other"))]),
            make_record("account", id(11), vec![("name", Value::from("Acme"))]),
        ];

        let conditions = vec![(
            "name".to_string(),
            vec![ChainItem::new(TransformData::Copy {
                path: "name".to_string(),
            })],
        )];

        let input = default_input(&source, MatchStrategy::Find, &conditions);
        let result = match_target(&input, &targets);

        assert!(matches!(result, MatchResult::Found(ref r) if r.id() == Some(id(11))));
    }

    #[test]
    fn find_multiple_conditions() {
        let source = make_record(
            "account",
            id(1),
            vec![("name", Value::from("Acme")), ("city", Value::from("NYC"))],
        );
        let targets = vec![
            make_record(
                "account",
                id(10),
                vec![("name", Value::from("Acme")), ("city", Value::from("LA"))],
            ),
            make_record(
                "account",
                id(11),
                vec![("name", Value::from("Acme")), ("city", Value::from("NYC"))],
            ),
        ];

        let conditions = vec![
            (
                "name".to_string(),
                vec![ChainItem::new(TransformData::Copy {
                    path: "name".to_string(),
                })],
            ),
            (
                "city".to_string(),
                vec![ChainItem::new(TransformData::Copy {
                    path: "city".to_string(),
                })],
            ),
        ];

        let input = default_input(&source, MatchStrategy::Find, &conditions);
        let result = match_target(&input, &targets);

        assert!(matches!(result, MatchResult::Found(ref r) if r.id() == Some(id(11))));
    }

    #[test]
    fn find_no_match() {
        let source = make_record("account", id(1), vec![("name", Value::from("Acme"))]);
        let targets = vec![
            make_record("account", id(10), vec![("name", Value::from("Other"))]),
            make_record("account", id(11), vec![("name", Value::from("Another"))]),
        ];

        let conditions = vec![(
            "name".to_string(),
            vec![ChainItem::new(TransformData::Copy {
                path: "name".to_string(),
            })],
        )];

        let input = default_input(&source, MatchStrategy::Find, &conditions);
        let result = match_target(&input, &targets);

        assert!(matches!(result, MatchResult::NotFound));
    }

    #[test]
    fn find_multiple_matches() {
        let source = make_record("account", id(1), vec![("name", Value::from("Acme"))]);
        let targets = vec![
            make_record("account", id(10), vec![("name", Value::from("Acme"))]),
            make_record("account", id(11), vec![("name", Value::from("Acme"))]),
        ];

        let conditions = vec![(
            "name".to_string(),
            vec![ChainItem::new(TransformData::Copy {
                path: "name".to_string(),
            })],
        )];

        let input = default_input(&source, MatchStrategy::Find, &conditions);
        let result = match_target(&input, &targets);

        assert!(matches!(result, MatchResult::Multiple(2)));
    }

    #[test]
    fn find_with_dotted_target_field() {
        // Target records have nested contact with email — match on dotted path
        let nested1 = Record::with_id(Entity::logical("contact"), id(100))
            .set("emailaddress1", Value::from("other@example.com"));
        let nested2 = Record::with_id(Entity::logical("contact"), id(101))
            .set("emailaddress1", Value::from("alice@example.com"));

        let source = make_record(
            "account",
            id(1),
            vec![("email", Value::from("alice@example.com"))],
        );
        let targets = vec![
            make_record(
                "account",
                id(10),
                vec![("primarycontactid", Value::Record(Box::new(nested1)))],
            ),
            make_record(
                "account",
                id(11),
                vec![("primarycontactid", Value::Record(Box::new(nested2)))],
            ),
        ];

        let conditions = vec![(
            "primarycontactid.emailaddress1".to_string(),
            vec![ChainItem::new(TransformData::Copy {
                path: "email".to_string(),
            })],
        )];

        let input = default_input(&source, MatchStrategy::Find, &conditions);
        let result = match_target(&input, &targets);

        assert!(matches!(result, MatchResult::Found(ref r) if r.id() == Some(id(11))));
    }

    #[test]
    fn find_with_constant_chain() {
        // Source chain produces a constant instead of copying from source
        let source = make_record("account", id(1), vec![]);
        let targets = vec![
            make_record("account", id(10), vec![("status", Value::Int(0))]),
            make_record("account", id(11), vec![("status", Value::Int(1))]),
        ];

        let conditions = vec![(
            "status".to_string(),
            vec![ChainItem::new(TransformData::Constant {
                value: Value::Int(1),
            })],
        )];

        let input = default_input(&source, MatchStrategy::Find, &conditions);
        let result = match_target(&input, &targets);

        assert!(matches!(result, MatchResult::Found(ref r) if r.id() == Some(id(11))));
    }

    #[test]
    fn find_chain_error_returns_error() {
        // Source chain references a missing field → error
        let source = make_record("account", id(1), vec![]);

        let conditions = vec![(
            "name".to_string(),
            vec![ChainItem::new(TransformData::Copy {
                path: "nonexistent".to_string(),
            })],
        )];

        let input = default_input(&source, MatchStrategy::Find, &conditions);
        let result = match_target(&input, &[]);

        assert!(matches!(result, MatchResult::Error(_)));
    }

    #[test]
    fn find_empty_targets() {
        let source = make_record("account", id(1), vec![("name", Value::from("Acme"))]);

        let conditions = vec![(
            "name".to_string(),
            vec![ChainItem::new(TransformData::Copy {
                path: "name".to_string(),
            })],
        )];

        let input = default_input(&source, MatchStrategy::Find, &conditions);
        let result = match_target(&input, &[]);

        assert!(matches!(result, MatchResult::NotFound));
    }
}
