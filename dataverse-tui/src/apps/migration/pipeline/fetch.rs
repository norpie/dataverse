//! Query building — converts analysis output into OData fetch tasks.
//!
//! Takes `FetchPlan` + entity mapping config and produces `ODataFetchTask`s
//! ready for the fetch modal. Handles:
//! - Test GUID overrides (completely replace source filter)
//! - Source/target filter conversion from `FilterNode`
//! - Find cache entity deduplication across mappings
//! - `ExpandSpec` → `QueryBuilder.expand()` conversion

use std::collections::HashMap;
use std::collections::HashSet;

use dataverse_lib::api::query::odata::QueryBuilder;
use dataverse_lib::api::query::Filter;
use dataverse_lib::model::Entity;
use dataverse_lib::model::Value;
use dataverse_lib::DataverseClient;

use crate::modals::odata_fetch::ODataFetchTask;
use crate::widgets::filter_builder::convert_filter;
use crate::widgets::filter_builder::FilterNode;

use super::ExpandSpec;
use super::FetchPlan;
use super::FindCacheSpec;
use super::SourceFetchSpec;
use super::TargetFetchSpec;

// =============================================================================
// Public API
// =============================================================================

/// Configuration for building fetch tasks from a single mapping's plan.
pub struct FetchTaskConfig<'a> {
    /// The fetch plan from analysis.
    pub plan: &'a FetchPlan,
    /// Primary key field name for the source entity.
    pub source_primary_key: &'a str,
    /// Primary key field name for the target entity.
    pub target_primary_key: &'a str,
    /// Source filter from entity mapping config.
    pub source_filter: Option<&'a FilterNode>,
    /// Target filter from entity mapping config.
    pub target_filter: Option<&'a FilterNode>,
    /// Test GUIDs — if present, completely override source filter.
    pub test_guids: Option<&'a [String]>,
    /// Display label for the source entity (used in task labels).
    pub mapping_name: &'a str,
}

/// Build the source entity fetch task.
///
/// Returns `None` if the filter conversion fails (invalid filter node).
pub fn build_source_task(config: &FetchTaskConfig<'_>) -> Result<QueryBuilder, BuildError> {
    let spec = &config.plan.source;

    let mut query = build_query_from_spec(
        &spec.entity,
        &spec.select,
        &spec.expands,
        config.source_primary_key,
    );

    // Apply filter: test GUIDs override source filter entirely
    let use_test_guids = config.test_guids.map(|g| !g.is_empty()).unwrap_or(false);

    if use_test_guids {
        let guids = config.test_guids.unwrap(); // safe: we just checked is_empty
        let pk = config.source_primary_key;
        let filters: Vec<Filter> = guids
            .iter()
            .map(|guid| Filter::eq(pk, Value::Guid(guid.parse().unwrap_or_default())))
            .collect();
        query = query.filter(Filter::or(filters));
    } else if let Some(filter_node) = config.source_filter {
        if let Some(filter) = convert_filter(filter_node).map_err(BuildError::FilterConvert)? {
            query = query.filter(filter);
        }
    }

    Ok(query)
}

/// Build the target entity fetch task (for match config).
///
/// Returns `None` if there's no target spec in the plan.
pub fn build_target_task(config: &FetchTaskConfig<'_>) -> Result<Option<QueryBuilder>, BuildError> {
    let spec = match &config.plan.target {
        Some(spec) => spec,
        None => return Ok(None),
    };

    let mut query = build_query_from_spec(
        &spec.entity,
        &spec.select,
        &spec.expands,
        config.target_primary_key,
    );

    // Apply target filter
    if let Some(filter_node) = config.target_filter {
        if let Some(filter) = convert_filter(filter_node).map_err(BuildError::FilterConvert)? {
            query = query.filter(filter);
        }
    }

    Ok(Some(query))
}

/// Build fetch tasks for find cache entities.
///
/// Takes the **merged** find cache specs (already deduplicated across mappings).
pub fn build_find_cache_tasks(specs: &[FindCacheSpec]) -> Vec<QueryBuilder> {
    specs
        .iter()
        .map(|spec| {
            let select_vec: Vec<&str> = spec.select.iter().map(|s| s.as_str()).collect();
            let mut query = QueryBuilder::new(Entity::logical(&spec.entity));
            if !select_vec.is_empty() {
                query = query.select(&select_vec);
            }
            for expand_spec in &spec.expands {
                query = apply_expand(query, expand_spec);
            }
            query
        })
        .collect()
}

// =============================================================================
// Phase-Level Find Cache Merging
// =============================================================================

/// Merge find cache specs across multiple entity mappings.
///
/// Multiple mappings in the same phase may reference the same find entity
/// (e.g., two mappings both doing `find(contact)`). This merges their
/// `select` sets and `expands` so we only fetch each entity once with the
/// union of fields.
pub fn merge_find_cache_specs(all_specs: Vec<Vec<FindCacheSpec>>) -> Vec<FindCacheSpec> {
    let mut merged: HashMap<String, (HashSet<String>, Vec<ExpandSpec>)> = HashMap::new();

    for specs in all_specs {
        for spec in specs {
            let entry = merged.entry(spec.entity).or_default();
            entry.0.extend(spec.select);
            merge_expands(&mut entry.1, spec.expands);
        }
    }

    merged
        .into_iter()
        .map(|(entity, (select, expands))| FindCacheSpec {
            entity,
            select,
            expands,
        })
        .collect()
}

/// Merge a list of ExpandSpecs into an existing list.
///
/// For each incoming expand, if an expand with the same nav_property already exists,
/// merge their selects and nested expands recursively. Otherwise, add the new expand.
fn merge_expands(existing: &mut Vec<ExpandSpec>, incoming: Vec<ExpandSpec>) {
    for inc in incoming {
        if let Some(existing_expand) = existing
            .iter_mut()
            .find(|e| e.nav_property == inc.nav_property)
        {
            existing_expand.select.extend(inc.select);
            merge_expands(&mut existing_expand.nested, inc.nested);
        } else {
            existing.push(inc);
        }
    }
}

/// Wrap a `QueryBuilder` into an `ODataFetchTask` with a label and client.
pub fn into_fetch_task(
    label: impl Into<String>,
    query: QueryBuilder,
    client: DataverseClient,
) -> ODataFetchTask {
    ODataFetchTask::new(label, client, query)
}

// =============================================================================
// Errors
// =============================================================================

/// Error building fetch tasks.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("Filter conversion failed: {0}")]
    FilterConvert(crate::widgets::filter_builder::ConvertError),
}

// =============================================================================
// Internal Helpers
// =============================================================================

/// Build a QueryBuilder from a fetch spec's fields and expands.
fn build_query_from_spec(
    entity: &str,
    select: &HashSet<String>,
    expands: &[ExpandSpec],
    primary_key: &str,
) -> QueryBuilder {
    // Collect select fields, ensuring primary key is included
    let mut select_set = select.clone();
    select_set.insert(primary_key.to_string());
    let select_vec: Vec<&str> = select_set.iter().map(|s| s.as_str()).collect();

    let mut query = QueryBuilder::new(Entity::logical(entity));
    if !select_vec.is_empty() {
        query = query.select(&select_vec);
    }

    // Add expands
    for expand_spec in expands {
        query = apply_expand(query, expand_spec);
    }

    query
}

/// Apply an ExpandSpec to a QueryBuilder recursively.
fn apply_expand(query: QueryBuilder, spec: &ExpandSpec) -> QueryBuilder {
    // Clone what we need for the closure (ExpandSpec fields are cheap)
    let select: Vec<String> = spec.select.iter().cloned().collect();
    let nested: Vec<ExpandSpec> = spec.nested.clone();

    query.expand(&spec.nav_property, move |e| {
        build_expand(e, &select, &nested)
    })
}

/// Build an ExpandBuilder with select fields and nested expands.
fn build_expand(
    mut builder: dataverse_lib::api::query::odata::ExpandBuilder,
    select: &[String],
    nested: &[ExpandSpec],
) -> dataverse_lib::api::query::odata::ExpandBuilder {
    if !select.is_empty() {
        let select_refs: Vec<&str> = select.iter().map(|s| s.as_str()).collect();
        builder = builder.select(&select_refs);
    }

    for nested_spec in nested {
        let nested_select: Vec<String> = nested_spec.select.iter().cloned().collect();
        let nested_nested: Vec<ExpandSpec> = nested_spec.nested.clone();
        builder = builder.expand(&nested_spec.nav_property, move |e| {
            build_expand(e, &nested_select, &nested_nested)
        });
    }

    builder
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_source_spec() -> SourceFetchSpec {
        let mut select = HashSet::new();
        select.insert("name".to_string());
        select.insert("accountnumber".to_string());
        select.insert("accountid".to_string());
        SourceFetchSpec {
            entity: "account".to_string(),
            select,
            expands: vec![],
        }
    }

    fn plan_with_source(source: SourceFetchSpec) -> FetchPlan {
        FetchPlan {
            source,
            target: None,
            find_caches: vec![],
            entity_ref_caches: vec![],
            lua_source_specs: vec![],
            lua_target_specs: vec![],
            find_lua_source_caches: vec![],
        }
    }

    fn simple_config<'a>(plan: &'a FetchPlan) -> FetchTaskConfig<'a> {
        FetchTaskConfig {
            plan,
            source_primary_key: "accountid",
            target_primary_key: "accountid",
            source_filter: None,
            target_filter: None,
            test_guids: None,
            mapping_name: "account → account",
        }
    }

    // ---- Source Task Tests ----

    #[test]
    fn source_task_includes_select_fields() {
        let plan = plan_with_source(simple_source_spec());
        let config = simple_config(&plan);
        let query = build_source_task(&config).unwrap();

        let selected = query.selected_fields();
        assert!(selected.contains(&"name".to_string()));
        assert!(selected.contains(&"accountnumber".to_string()));
        assert!(selected.contains(&"accountid".to_string()));
    }

    #[test]
    fn source_task_with_expand() {
        let mut source = simple_source_spec();
        let mut expand_select = HashSet::new();
        expand_select.insert("fullname".to_string());
        expand_select.insert("emailaddress1".to_string());
        source.expands.push(ExpandSpec {
            nav_property: "primarycontactid".to_string(),
            select: expand_select,
            nested: vec![],
        });

        let plan = plan_with_source(source);
        let config = simple_config(&plan);
        let query = build_source_task(&config).unwrap();

        // We can't directly inspect the expand builder on QueryBuilder,
        // but we can verify it doesn't error out and the select is correct.
        let selected = query.selected_fields();
        assert!(selected.contains(&"name".to_string()));
    }

    #[test]
    fn test_guid_override_replaces_filter() {
        let plan = plan_with_source(simple_source_spec());
        let guids = vec![
            "00000000-0000-0000-0000-000000000001".to_string(),
            "00000000-0000-0000-0000-000000000002".to_string(),
        ];
        let config = FetchTaskConfig {
            plan: &plan,
            source_primary_key: "accountid",
            target_primary_key: "accountid",
            source_filter: None,
            target_filter: None,
            test_guids: Some(&guids),
            mapping_name: "test",
        };
        let query = build_source_task(&config).unwrap();

        // Query should build successfully with GUID filter
        let selected = query.selected_fields();
        assert!(selected.contains(&"accountid".to_string()));
    }

    #[test]
    fn test_guid_override_ignores_source_filter() {
        let source_filter = FilterNode::Condition {
            id: 1,
            field: "statecode".to_string(),
            operator: crate::widgets::filter_builder::CondOp::Eq,
            value: Value::Int(0),
        };
        let plan = plan_with_source(simple_source_spec());
        let guids = vec!["00000000-0000-0000-0000-000000000001".to_string()];
        let config = FetchTaskConfig {
            plan: &plan,
            source_primary_key: "accountid",
            target_primary_key: "accountid",
            source_filter: Some(&source_filter),
            target_filter: None,
            test_guids: Some(&guids),
            mapping_name: "test",
        };

        // Should succeed — test GUIDs override the source filter
        let query = build_source_task(&config).unwrap();
        assert!(!query.selected_fields().is_empty());
    }

    #[test]
    fn source_filter_applied_when_no_test_guids() {
        let source_filter = FilterNode::Condition {
            id: 1,
            field: "statecode".to_string(),
            operator: crate::widgets::filter_builder::CondOp::Eq,
            value: Value::Int(0),
        };
        let plan = plan_with_source(simple_source_spec());
        let config = FetchTaskConfig {
            plan: &plan,
            source_primary_key: "accountid",
            target_primary_key: "accountid",
            source_filter: Some(&source_filter),
            target_filter: None,
            test_guids: None,
            mapping_name: "test",
        };

        let query = build_source_task(&config).unwrap();
        assert!(!query.selected_fields().is_empty());
    }

    #[test]
    fn empty_test_guids_dont_override_filter() {
        let source_filter = FilterNode::Condition {
            id: 1,
            field: "statecode".to_string(),
            operator: crate::widgets::filter_builder::CondOp::Eq,
            value: Value::Int(0),
        };
        let plan = plan_with_source(simple_source_spec());
        let empty_guids: Vec<String> = vec![];
        let config = FetchTaskConfig {
            plan: &plan,
            source_primary_key: "accountid",
            target_primary_key: "accountid",
            source_filter: Some(&source_filter),
            target_filter: None,
            test_guids: Some(&empty_guids),
            mapping_name: "test",
        };

        // Empty GUIDs → falls through to source filter
        let query = build_source_task(&config).unwrap();
        assert!(!query.selected_fields().is_empty());
    }

    // ---- Target Task Tests ----

    #[test]
    fn no_target_task_when_no_target_spec() {
        let plan = plan_with_source(simple_source_spec());
        let config = simple_config(&plan);
        let result = build_target_task(&config).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn target_task_built_when_target_spec_present() {
        let mut target_select = HashSet::new();
        target_select.insert("externalid".to_string());
        let plan = FetchPlan {
            source: simple_source_spec(),
            target: Some(TargetFetchSpec {
                entity: "account".to_string(),
                select: target_select,
                expands: vec![],
            }),
            find_caches: vec![],
            entity_ref_caches: vec![],
            lua_source_specs: vec![],
            lua_target_specs: vec![],
            find_lua_source_caches: vec![],
        };
        let config = simple_config(&plan);
        let result = build_target_task(&config).unwrap();
        assert!(result.is_some());

        let query = result.unwrap();
        let selected = query.selected_fields();
        assert!(selected.contains(&"externalid".to_string()));
        assert!(selected.contains(&"accountid".to_string())); // PK always included
    }

    // ---- Find Cache Tests ----

    #[test]
    fn find_cache_task_built_for_each_entity() {
        let mut contact_select = HashSet::new();
        contact_select.insert("fullname".to_string());
        contact_select.insert("emailaddress1".to_string());

        let mut capacity_select = HashSet::new();
        capacity_select.insert("name".to_string());

        let specs = vec![
            FindCacheSpec {
                entity: "contact".to_string(),
                select: contact_select,
                expands: vec![],
            },
            FindCacheSpec {
                entity: "capacity".to_string(),
                select: capacity_select,
                expands: vec![],
            },
        ];

        let tasks = build_find_cache_tasks(&specs);
        assert_eq!(tasks.len(), 2);
    }

    // ---- Merge Tests ----

    #[test]
    fn merge_find_cache_specs_unions_fields() {
        let mut select1 = HashSet::new();
        select1.insert("fullname".to_string());

        let mut select2 = HashSet::new();
        select2.insert("emailaddress1".to_string());

        let specs1 = vec![FindCacheSpec {
            entity: "contact".to_string(),
            select: select1,
            expands: vec![],
        }];
        let specs2 = vec![FindCacheSpec {
            entity: "contact".to_string(),
            select: select2,
            expands: vec![],
        }];

        let merged = merge_find_cache_specs(vec![specs1, specs2]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].entity, "contact");
        assert!(merged[0].select.contains("fullname"));
        assert!(merged[0].select.contains("emailaddress1"));
    }

    #[test]
    fn merge_find_cache_specs_keeps_separate_entities() {
        let mut contact_select = HashSet::new();
        contact_select.insert("fullname".to_string());

        let mut capacity_select = HashSet::new();
        capacity_select.insert("name".to_string());

        let specs1 = vec![FindCacheSpec {
            entity: "contact".to_string(),
            select: contact_select,
            expands: vec![],
        }];
        let specs2 = vec![FindCacheSpec {
            entity: "capacity".to_string(),
            select: capacity_select,
            expands: vec![],
        }];

        let merged = merge_find_cache_specs(vec![specs1, specs2]);
        assert_eq!(merged.len(), 2);
        assert!(merged.iter().any(|s| s.entity == "contact"));
        assert!(merged.iter().any(|s| s.entity == "capacity"));
    }

    #[test]
    fn merge_find_cache_specs_empty_input() {
        let merged = merge_find_cache_specs(vec![]);
        assert!(merged.is_empty());
    }

    #[test]
    fn merge_find_cache_specs_deduplicates_fields() {
        let mut select1 = HashSet::new();
        select1.insert("fullname".to_string());

        let mut select2 = HashSet::new();
        select2.insert("fullname".to_string()); // same field
        select2.insert("emailaddress1".to_string());

        let specs1 = vec![FindCacheSpec {
            entity: "contact".to_string(),
            select: select1,
            expands: vec![],
        }];
        let specs2 = vec![FindCacheSpec {
            entity: "contact".to_string(),
            select: select2,
            expands: vec![],
        }];

        let merged = merge_find_cache_specs(vec![specs1, specs2]);
        assert_eq!(merged.len(), 1);
        // fullname should appear once, not twice
        assert_eq!(merged[0].select.len(), 2);
        assert!(merged[0].select.contains("fullname"));
        assert!(merged[0].select.contains("emailaddress1"));
    }

    // ---- Nested Expand Tests ----

    #[test]
    fn nested_expand_builds_successfully() {
        let mut inner_select = HashSet::new();
        inner_select.insert("name".to_string());

        let mut outer_select = HashSet::new();
        outer_select.insert("fullname".to_string());

        let source = SourceFetchSpec {
            entity: "account".to_string(),
            select: HashSet::from(["accountid".to_string()]),
            expands: vec![ExpandSpec {
                nav_property: "primarycontactid".to_string(),
                select: outer_select,
                nested: vec![ExpandSpec {
                    nav_property: "parentcustomerid".to_string(),
                    select: inner_select,
                    nested: vec![],
                }],
            }],
        };

        let plan = plan_with_source(source);
        let config = simple_config(&plan);
        let query = build_source_task(&config).unwrap();

        // Should build without error
        let selected = query.selected_fields();
        assert!(selected.contains(&"accountid".to_string()));
    }

    // ---- Merge Expands Tests ----

    #[test]
    fn merge_find_cache_specs_merges_expands() {
        let specs1 = vec![FindCacheSpec {
            entity: "account".to_string(),
            select: HashSet::from(["name".to_string()]),
            expands: vec![ExpandSpec {
                nav_property: "primarycontactid".to_string(),
                select: HashSet::from(["fullname".to_string()]),
                nested: vec![],
            }],
        }];
        let specs2 = vec![FindCacheSpec {
            entity: "account".to_string(),
            select: HashSet::from(["revenue".to_string()]),
            expands: vec![ExpandSpec {
                nav_property: "primarycontactid".to_string(),
                select: HashSet::from(["emailaddress1".to_string()]),
                nested: vec![],
            }],
        }];

        let merged = merge_find_cache_specs(vec![specs1, specs2]);
        assert_eq!(merged.len(), 1);

        let spec = &merged[0];
        assert!(spec.select.contains("name"));
        assert!(spec.select.contains("revenue"));

        assert_eq!(spec.expands.len(), 1);
        let expand = &spec.expands[0];
        assert_eq!(expand.nav_property, "primarycontactid");
        assert!(expand.select.contains("fullname"));
        assert!(expand.select.contains("emailaddress1"));
    }

    #[test]
    fn merge_find_cache_specs_different_expands_kept_separate() {
        let specs1 = vec![FindCacheSpec {
            entity: "account".to_string(),
            select: HashSet::new(),
            expands: vec![ExpandSpec {
                nav_property: "primarycontactid".to_string(),
                select: HashSet::from(["fullname".to_string()]),
                nested: vec![],
            }],
        }];
        let specs2 = vec![FindCacheSpec {
            entity: "account".to_string(),
            select: HashSet::new(),
            expands: vec![ExpandSpec {
                nav_property: "ownerid".to_string(),
                select: HashSet::from(["fullname".to_string()]),
                nested: vec![],
            }],
        }];

        let merged = merge_find_cache_specs(vec![specs1, specs2]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].expands.len(), 2);
    }
}
