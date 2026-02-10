//! Config analysis — statically walk materialized chains to determine fetch requirements.
//!
//! Given an entity mapping's materialized chain trees, this module determines:
//! - Which source fields need to be fetched (`$select` / `$expand`)
//! - Which find cache entities and fields are needed
//! - Which target fields are needed (for match config)
//!
//! This is pure, sync analysis — no data fetching occurs here.

use std::collections::HashMap;
use std::collections::HashSet;

use crate::apps::migration::engine::BranchItem;
use crate::apps::migration::engine::ChainChildren;
use crate::apps::migration::engine::ChainItem;
use crate::apps::migration::engine::FindConditionItem;
use crate::apps::migration::types::Condition;
use crate::apps::migration::types::Expr;
use crate::apps::migration::types::FindMode;
use crate::apps::migration::types::TransformData;
use crate::apps::migration::validation::parse_path;
use crate::apps::migration::validation::FieldPath;
use crate::apps::migration::validation::FieldSegment;
use crate::apps::migration::validation::PathExpr;

use super::ExpandSpec;
use super::FetchPlan;
use super::FindCacheSpec;
use super::SourceFetchSpec;
use super::TargetFetchSpec;

// =============================================================================
// Public API
// =============================================================================

/// Input for analyzing a single entity mapping.
pub struct AnalysisInput<'a> {
    /// Source entity logical name.
    pub source_entity: &'a str,
    /// Target entity logical name.
    pub target_entity: &'a str,
    /// Primary key field for the source entity (from metadata).
    pub source_primary_key: &'a str,
    /// Primary key field for the target entity (from metadata).
    pub target_primary_key: &'a str,
    /// Materialized field mapping chains: (target_field, chain).
    pub field_mappings: &'a [(String, Vec<ChainItem>)],
    /// Materialized variable chains: (variable_name, chain).
    pub variables: &'a [(String, Vec<ChainItem>)],
    /// Materialized match config chain (if match strategy is Find).
    pub match_config_chain: Option<&'a [ChainItem]>,
}

/// Analyze an entity mapping to determine what data needs to be fetched.
///
/// Walks all materialized chain trees to collect source field paths,
/// find cache requirements, and target field needs.
pub fn analyze_mapping(input: &AnalysisInput<'_>) -> FetchPlan {
    let mut collector = Collector::new(input.source_entity, input.target_entity);

    // Always include primary keys
    collector
        .source
        .select
        .insert(input.source_primary_key.to_string());

    // Track which variables come from find transforms
    let mut variable_find_entities: HashMap<String, String> = HashMap::new();

    // 1. Analyze variable chains first — they may contain find() transforms
    //    that establish variable → entity mappings
    for (var_name, chain) in input.variables {
        analyze_chain(chain, &variable_find_entities, &mut collector);
        // After analyzing, check if this variable's chain ends with a find
        if let Some(find_entity) = find_entity_from_chain(chain) {
            variable_find_entities.insert(var_name.clone(), find_entity);
        }
    }

    // 2. Analyze field mapping chains
    for (_target_field, chain) in input.field_mappings {
        analyze_chain(chain, &variable_find_entities, &mut collector);
    }

    // 3. Analyze match config chain (if present)
    if let Some(chain) = input.match_config_chain {
        analyze_chain(chain, &variable_find_entities, &mut collector);
    }

    // 4. Target always needs the primary key + all field mapping target fields
    //    so comparison can diff transformed values against existing target records.
    collector
        .target
        .select
        .insert(input.target_primary_key.to_string());
    for (target_field, _) in input.field_mappings {
        collector.target.select.insert(target_field.clone());
    }

    // Build the fetch plan
    let target = if collector.target.has_fields() {
        Some(collector.target.build_target())
    } else {
        None
    };

    FetchPlan {
        source: collector.source.build_source(),
        target,
        find_caches: collector.find_caches.build(),
    }
}

// =============================================================================
// Collector — accumulates fetch requirements during analysis
// =============================================================================

/// Accumulates fetch requirements as we walk chains.
struct Collector {
    source: EntityFetchBuilder,
    target: EntityFetchBuilder,
    find_caches: FindCacheBuilder,
}

impl Collector {
    fn new(source_entity: &str, target_entity: &str) -> Self {
        Self {
            source: EntityFetchBuilder::new(source_entity.to_string()),
            target: EntityFetchBuilder::new(target_entity.to_string()),
            find_caches: FindCacheBuilder::new(),
        }
    }
}

/// Builds a `SourceFetchSpec` or `TargetFetchSpec` by accumulating select/expand.
struct EntityFetchBuilder {
    entity: String,
    select: HashSet<String>,
    expands: HashMap<String, ExpandBuilder>,
}

impl EntityFetchBuilder {
    fn new(entity: String) -> Self {
        Self {
            entity,
            select: HashSet::new(),
            expands: HashMap::new(),
        }
    }

    /// Whether any fields have been added.
    fn has_fields(&self) -> bool {
        !self.select.is_empty() || !self.expands.is_empty()
    }

    /// Add a field path to this entity's fetch requirements.
    fn add_field_path(&mut self, path: &FieldPath) {
        if path.segments.is_empty() {
            return;
        }

        if path.segments.len() == 1 {
            // Single segment → direct select
            self.select.insert(path.segments[0].field.clone());
        } else {
            // Multi-segment → first is nav property, rest goes into expand
            let nav = &path.segments[0];
            let expand = self
                .expands
                .entry(nav.field.clone())
                .or_insert_with(|| ExpandBuilder::new(nav.field.clone()));

            // Remaining segments: if just one more, it's a select on the expand.
            // If multiple, it's nested expands.
            expand.add_remaining_segments(&path.segments[1..]);
        }
    }

    fn build_source(self) -> SourceFetchSpec {
        SourceFetchSpec {
            entity: self.entity,
            select: self.select,
            expands: self.expands.into_values().map(|b| b.build()).collect(),
        }
    }

    fn build_target(self) -> TargetFetchSpec {
        TargetFetchSpec {
            entity: self.entity,
            select: self.select,
            expands: self.expands.into_values().map(|b| b.build()).collect(),
        }
    }
}

/// Builds an `ExpandSpec` by accumulating select/nested expands.
struct ExpandBuilder {
    nav_property: String,
    select: HashSet<String>,
    nested: HashMap<String, ExpandBuilder>,
}

impl ExpandBuilder {
    fn new(nav_property: String) -> Self {
        Self {
            nav_property,
            select: HashSet::new(),
            nested: HashMap::new(),
        }
    }

    /// Add remaining path segments (after the nav property).
    fn add_remaining_segments(&mut self, segments: &[FieldSegment]) {
        if segments.is_empty() {
            return;
        }

        if segments.len() == 1 {
            // Last segment → select on this expand
            self.select.insert(segments[0].field.clone());
        } else {
            // More segments → nested expand
            let nav = &segments[0];
            let nested = self
                .nested
                .entry(nav.field.clone())
                .or_insert_with(|| ExpandBuilder::new(nav.field.clone()));
            nested.add_remaining_segments(&segments[1..]);
        }
    }

    fn build(self) -> ExpandSpec {
        ExpandSpec {
            nav_property: self.nav_property,
            select: self.select,
            nested: self.nested.into_values().map(|b| b.build()).collect(),
        }
    }
}

/// Builds `FindCacheSpec`s by accumulating fields and expands per entity.
struct FindCacheBuilder {
    entities: HashMap<String, (HashSet<String>, HashMap<String, ExpandBuilder>)>,
}

impl FindCacheBuilder {
    fn new() -> Self {
        Self {
            entities: HashMap::new(),
        }
    }

    /// Add a field requirement for a find cache entity.
    fn add_field(&mut self, entity: &str, field: &str) {
        self.entities
            .entry(entity.to_string())
            .or_default()
            .0
            .insert(field.to_string());
    }

    /// Add a field path to a find cache entity's fetch requirements.
    ///
    /// Single-segment paths go into `select`, multi-segment paths into `expands`.
    fn add_field_path(&mut self, entity: &str, path: &FieldPath) {
        if path.segments.is_empty() {
            return;
        }

        if path.segments.len() == 1 {
            self.add_field(entity, &path.segments[0].field);
        } else {
            // Multi-segment: first is nav property, rest goes into expand
            let nav = &path.segments[0];
            let (_, expands) = self.entities.entry(entity.to_string()).or_default();
            let expand = expands
                .entry(nav.field.clone())
                .or_insert_with(|| ExpandBuilder::new(nav.field.clone()));
            expand.add_remaining_segments(&path.segments[1..]);
        }
    }

    /// Ensure an entity is tracked (even with no specific fields yet).
    fn ensure_entity(&mut self, entity: &str) {
        self.entities.entry(entity.to_string()).or_default();
    }

    fn build(self) -> Vec<FindCacheSpec> {
        self.entities
            .into_iter()
            .map(|(entity, (select, expands))| {
                let expands = expands.into_values().map(|b| b.build()).collect();
                FindCacheSpec {
                    entity,
                    select,
                    expands,
                }
            })
            .collect()
    }
}

// =============================================================================
// Chain Analysis — recursive walk of ChainItem trees
// =============================================================================

/// Analyze a chain of transforms, collecting fetch requirements.
fn analyze_chain(
    chain: &[ChainItem],
    var_find_entities: &HashMap<String, String>,
    collector: &mut Collector,
) {
    for item in chain {
        analyze_item(item, var_find_entities, collector);
    }
}

/// Analyze a single chain item and its children.
fn analyze_item(
    item: &ChainItem,
    var_find_entities: &HashMap<String, String>,
    collector: &mut Collector,
) {
    // Special handling: if this is a Find(Where), collect target_fields from conditions
    if let TransformData::Find {
        entity,
        mode: FindMode::Where,
        ..
    } = &item.data
    {
        if let ChainChildren::FindConditions(conditions, _) = &item.children {
            for cond in conditions {
                add_target_field_to_find_cache(&cond.target_field, entity, collector);
            }
        }
    }

    // Analyze the transform data itself
    analyze_transform_data(&item.data, var_find_entities, collector);

    // Analyze children recursively
    analyze_children(&item.children, var_find_entities, collector);
}

/// Analyze the transform data to extract path references.
fn analyze_transform_data(
    data: &TransformData,
    var_find_entities: &HashMap<String, String>,
    collector: &mut Collector,
) {
    match data {
        TransformData::Copy { path } => {
            analyze_path_string(path, var_find_entities, collector);
        }
        TransformData::Format { template } => {
            analyze_template(template, var_find_entities, collector);
        }
        TransformData::Guard { condition } => {
            analyze_condition(condition, var_find_entities, collector);
        }
        TransformData::Find { entity, mode, .. } => {
            analyze_find(entity, mode, collector);
        }
        // These transforms don't reference source fields:
        TransformData::Constant { .. }
        | TransformData::Match { .. }
        | TransformData::Replace { .. }
        | TransformData::StringOps { .. }
        | TransformData::ValueMap { .. }
        | TransformData::Math { .. }
        | TransformData::Coalesce
        | TransformData::Convert { .. }
        | TransformData::ParseInt
        | TransformData::ParseDecimal
        | TransformData::ParseDate { .. }
        | TransformData::Guid => {}
    }
}

/// Analyze child chains recursively.
fn analyze_children(
    children: &ChainChildren,
    var_find_entities: &HashMap<String, String>,
    collector: &mut Collector,
) {
    match children {
        ChainChildren::None => {}
        ChainChildren::Fallback(chain) => {
            analyze_chain(chain, var_find_entities, collector);
        }
        ChainChildren::Branches(branches, default) => {
            for branch in branches {
                analyze_branch(branch, var_find_entities, collector);
            }
            if let Some(default_chain) = default {
                analyze_chain(default_chain, var_find_entities, collector);
            }
        }
        ChainChildren::Alternatives(alternatives) => {
            for alt in alternatives {
                analyze_chain(alt, var_find_entities, collector);
            }
        }
        ChainChildren::FindConditions(conditions, default) => {
            for cond in conditions {
                analyze_find_condition(cond, var_find_entities, collector);
            }
            if let Some(default_chain) = default {
                analyze_chain(default_chain, var_find_entities, collector);
            }
        }
    }
}

/// Analyze a match branch (condition + chain).
fn analyze_branch(
    branch: &BranchItem,
    var_find_entities: &HashMap<String, String>,
    collector: &mut Collector,
) {
    analyze_condition(&branch.condition, var_find_entities, collector);
    analyze_chain(&branch.chain, var_find_entities, collector);
}

/// Analyze a find condition (target_field + source_chain).
fn analyze_find_condition(
    cond: &FindConditionItem,
    var_find_entities: &HashMap<String, String>,
    collector: &mut Collector,
) {
    // The source chain produces a value to match against — analyze it for source paths
    analyze_chain(&cond.source_chain, var_find_entities, collector);
    // Note: target_field is added to the find cache by analyze_find(),
    // not here — we don't know the entity from the condition alone.
}

// =============================================================================
// Path Analysis — parse and classify path references
// =============================================================================

/// Parse a path string and add it to the appropriate fetch requirement.
fn analyze_path_string(
    path: &str,
    var_find_entities: &HashMap<String, String>,
    collector: &mut Collector,
) {
    let parsed = match parse_path(path) {
        Ok(p) => p,
        Err(_) => return, // Invalid paths are ignored at analysis time
    };

    match parsed {
        PathExpr::Field(field_path) => {
            collector.source.add_field_path(&field_path);
        }
        PathExpr::Variable(_) => {
            // Plain variable reference (no navigation) — no fields to fetch
        }
        PathExpr::VariableNavigation { name, path, .. } => {
            // $var.field — if var came from a find, add to find cache
            if let Some(find_entity) = var_find_entities.get(&name) {
                add_field_path_to_find_cache(&path, find_entity, collector);
            }
            // If var didn't come from a find, it's a variable with a Record value
            // set by other means — nothing to fetch
        }
        PathExpr::SystemVar(_) => {
            // System vars don't require fetching
        }
    }
}

/// Analyze a format template, extracting all path references.
fn analyze_template(
    template: &str,
    var_find_entities: &HashMap<String, String>,
    collector: &mut Collector,
) {
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' {
            let mut placeholder = String::new();
            let mut found_close = false;

            for inner in chars.by_ref() {
                if inner == '}' {
                    found_close = true;
                    break;
                }
                placeholder.push(inner);
            }

            if found_close && !placeholder.is_empty() {
                analyze_path_string(&placeholder, var_find_entities, collector);
            }
        }
    }
}

/// Analyze a condition expression, extracting path references.
fn analyze_condition(
    condition: &Condition,
    var_find_entities: &HashMap<String, String>,
    collector: &mut Collector,
) {
    match condition {
        Condition::And(conditions) | Condition::Or(conditions) => {
            for cond in conditions {
                analyze_condition(cond, var_find_entities, collector);
            }
        }
        Condition::Not(inner) => {
            analyze_condition(inner, var_find_entities, collector);
        }
        Condition::Compare { left, right, .. } => {
            analyze_expr(left, var_find_entities, collector);
            analyze_expr(right, var_find_entities, collector);
        }
        Condition::IsNull(expr) | Condition::IsNotNull(expr) => {
            analyze_expr(expr, var_find_entities, collector);
        }
        Condition::Contains { value, substring } => {
            analyze_expr(value, var_find_entities, collector);
            analyze_expr(substring, var_find_entities, collector);
        }
        Condition::StartsWith { value, prefix } => {
            analyze_expr(value, var_find_entities, collector);
            analyze_expr(prefix, var_find_entities, collector);
        }
        Condition::EndsWith { value, suffix } => {
            analyze_expr(value, var_find_entities, collector);
            analyze_expr(suffix, var_find_entities, collector);
        }
    }
}

/// Analyze a condition expression value.
fn analyze_expr(
    expr: &Expr,
    var_find_entities: &HashMap<String, String>,
    collector: &mut Collector,
) {
    match expr {
        Expr::Path(path) => {
            analyze_path_string(path, var_find_entities, collector);
        }
        Expr::Variable(name) => {
            // Plain variable reference — if it came from a find, ensure cache entity exists
            if let Some(find_entity) = var_find_entities.get(name) {
                collector.find_caches.ensure_entity(find_entity);
            }
        }
        Expr::SystemVar(_) | Expr::Literal(_) => {
            // No fields to fetch
        }
    }
}

// =============================================================================
// Find Analysis
// =============================================================================

/// Analyze a find transform — register the entity for cache and handle mode-specific needs.
fn analyze_find(entity: &str, mode: &FindMode, collector: &mut Collector) {
    // Ensure the find cache entity is tracked
    collector.find_caches.ensure_entity(entity);

    match mode {
        FindMode::Where => {
            // target_fields from FindConditionItems are handled in analyze_item(),
            // which has access to both the Find transform data and its children.
        }
        FindMode::Lua { .. } => {
            // Lua find: would need to call declare() to get field requirements.
            // For now, we ensure the entity is tracked; Lua declare() integration
            // will be added when the Lua runtime integration is implemented.
        }
    }
}

/// Extract the find entity from a chain's last find transform (if any).
///
/// Variables are often set by chains like: `[Copy { path: "name" }, Find { entity: "contact", .. }]`
/// The find at the end tells us this variable holds a Record from that entity.
fn find_entity_from_chain(chain: &[ChainItem]) -> Option<String> {
    // Walk the chain in reverse to find the last Find transform
    for item in chain.iter().rev() {
        if let TransformData::Find { entity, .. } = &item.data {
            return Some(entity.clone());
        }
    }
    None
}

/// Add field paths from a FieldPath to a find cache entity's requirements.
///
/// Single-segment paths go into select, multi-segment paths into expands.
fn add_field_path_to_find_cache(path: &FieldPath, entity: &str, collector: &mut Collector) {
    collector.find_caches.add_field_path(entity, path);
}

/// Parse a target_field string (potentially dotted) and add to find cache requirements.
///
/// Used for find condition `target_field` and match condition `target_field` values.
fn add_target_field_to_find_cache(target_field: &str, entity: &str, collector: &mut Collector) {
    // Parse as a simple dotted path (no $variables, just field segments)
    let segments: Vec<FieldSegment> = target_field
        .split('.')
        .map(|s| FieldSegment {
            field: s.to_string(),
            target: None,
            optional: false,
        })
        .collect();
    let path = FieldPath { segments };
    collector.find_caches.add_field_path(entity, &path);
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use crate::apps::migration::types::CompareOp;
    use crate::apps::migration::types::FindFallback;
    use dataverse_lib::model::Value;

    use super::*;

    // ---- Helpers ----

    fn simple_input<'a>(
        field_mappings: &'a [(String, Vec<ChainItem>)],
        variables: &'a [(String, Vec<ChainItem>)],
    ) -> AnalysisInput<'a> {
        AnalysisInput {
            source_entity: "account",
            target_entity: "account",
            source_primary_key: "accountid",
            target_primary_key: "accountid",
            field_mappings,
            variables,
            match_config_chain: None,
        }
    }

    fn copy(path: &str) -> ChainItem {
        ChainItem::new(TransformData::Copy {
            path: path.to_string(),
        })
    }

    fn format(template: &str) -> ChainItem {
        ChainItem::new(TransformData::Format {
            template: template.to_string(),
        })
    }

    fn constant(value: Value) -> ChainItem {
        ChainItem::new(TransformData::Constant { value })
    }

    fn find_where(entity: &str, conditions: Vec<FindConditionItem>) -> ChainItem {
        ChainItem::with_find_conditions(
            TransformData::Find {
                entity: entity.to_string(),
                fallback: FindFallback::Error,
                mode: FindMode::Where,
            },
            conditions,
            None,
        )
    }

    fn find_condition(target_field: &str, source_chain: Vec<ChainItem>) -> FindConditionItem {
        FindConditionItem {
            target_field: target_field.to_string(),
            source_chain,
        }
    }

    fn guard(condition: Condition, fallback: Vec<ChainItem>) -> ChainItem {
        ChainItem::with_fallback(TransformData::Guard { condition }, fallback)
    }

    fn match_transform(branches: Vec<BranchItem>, default: Option<Vec<ChainItem>>) -> ChainItem {
        ChainItem::with_branches(
            TransformData::Match {
                has_default: default.is_some(),
            },
            branches,
            default,
        )
    }

    fn coalesce(alternatives: Vec<Vec<ChainItem>>) -> ChainItem {
        ChainItem::with_alternatives(TransformData::Coalesce, alternatives)
    }

    // ---- Tests ----

    #[test]
    fn primary_key_always_included() {
        let field_mappings = vec![];
        let variables = vec![];
        let plan = analyze_mapping(&simple_input(&field_mappings, &variables));

        assert!(plan.source.select.contains("accountid"));
    }

    #[test]
    fn simple_copy_adds_source_select() {
        let field_mappings = vec![
            ("name".to_string(), vec![copy("name")]),
            ("revenue".to_string(), vec![copy("annualrevenue")]),
        ];
        let variables = vec![];
        let plan = analyze_mapping(&simple_input(&field_mappings, &variables));

        assert!(plan.source.select.contains("name"));
        assert!(plan.source.select.contains("annualrevenue"));
        assert!(plan.source.select.contains("accountid")); // PK
    }

    #[test]
    fn lookup_navigation_creates_expand() {
        let field_mappings = vec![(
            "contactname".to_string(),
            vec![copy("primarycontactid.fullname")],
        )];
        let variables = vec![];
        let plan = analyze_mapping(&simple_input(&field_mappings, &variables));

        // Should NOT be in top-level select
        assert!(!plan.source.select.contains("primarycontactid"));
        // Should create an expand
        assert_eq!(plan.source.expands.len(), 1);
        let expand = &plan.source.expands[0];
        assert_eq!(expand.nav_property, "primarycontactid");
        assert!(expand.select.contains("fullname"));
    }

    #[test]
    fn multiple_fields_same_expand_merge() {
        let field_mappings = vec![
            (
                "contactname".to_string(),
                vec![copy("primarycontactid.fullname")],
            ),
            (
                "contactemail".to_string(),
                vec![copy("primarycontactid.emailaddress1")],
            ),
        ];
        let variables = vec![];
        let plan = analyze_mapping(&simple_input(&field_mappings, &variables));

        assert_eq!(plan.source.expands.len(), 1);
        let expand = &plan.source.expands[0];
        assert_eq!(expand.nav_property, "primarycontactid");
        assert!(expand.select.contains("fullname"));
        assert!(expand.select.contains("emailaddress1"));
    }

    #[test]
    fn three_level_path_creates_nested_expand() {
        let field_mappings = vec![(
            "parentname".to_string(),
            vec![copy("primarycontactid.parentcustomerid.name")],
        )];
        let variables = vec![];
        let plan = analyze_mapping(&simple_input(&field_mappings, &variables));

        assert_eq!(plan.source.expands.len(), 1);
        let expand = &plan.source.expands[0];
        assert_eq!(expand.nav_property, "primarycontactid");
        assert!(expand.select.is_empty()); // no direct fields on this level
        assert_eq!(expand.nested.len(), 1);
        let nested = &expand.nested[0];
        assert_eq!(nested.nav_property, "parentcustomerid");
        assert!(nested.select.contains("name"));
    }

    #[test]
    fn format_template_extracts_field_paths() {
        let field_mappings = vec![(
            "description".to_string(),
            vec![format("{name} - {accountnumber}")],
        )];
        let variables = vec![];
        let plan = analyze_mapping(&simple_input(&field_mappings, &variables));

        assert!(plan.source.select.contains("name"));
        assert!(plan.source.select.contains("accountnumber"));
    }

    #[test]
    fn format_template_with_lookup_creates_expand() {
        let field_mappings = vec![(
            "description".to_string(),
            vec![format("Contact: {primarycontactid.fullname}")],
        )];
        let variables = vec![];
        let plan = analyze_mapping(&simple_input(&field_mappings, &variables));

        assert_eq!(plan.source.expands.len(), 1);
        assert_eq!(plan.source.expands[0].nav_property, "primarycontactid");
        assert!(plan.source.expands[0].select.contains("fullname"));
    }

    #[test]
    fn system_vars_and_constants_dont_add_fields() {
        let field_mappings = vec![
            ("field1".to_string(), vec![copy("#value")]),
            (
                "field2".to_string(),
                vec![constant(Value::String("hello".to_string()))],
            ),
        ];
        let variables = vec![];
        let plan = analyze_mapping(&simple_input(&field_mappings, &variables));

        // Only the PK should be in select
        assert_eq!(plan.source.select.len(), 1);
        assert!(plan.source.select.contains("accountid"));
        assert!(plan.source.expands.is_empty());
    }

    #[test]
    fn find_where_collects_target_fields_for_cache() {
        // Variable chain: Copy("name") → Find(contact, Where, conditions: [name_cond])
        let variables = vec![(
            "matched_contact".to_string(),
            vec![find_where(
                "contact",
                vec![find_condition("fullname", vec![copy("name")])],
            )],
        )];
        let field_mappings = vec![];
        let plan = analyze_mapping(&simple_input(&field_mappings, &variables));

        // Source should include "name" (from Copy in find condition's source chain)
        assert!(plan.source.select.contains("name"));

        // Find cache should include contact entity with fullname field
        assert_eq!(plan.find_caches.len(), 1);
        let cache = &plan.find_caches[0];
        assert_eq!(cache.entity, "contact");
        assert!(cache.select.contains("fullname"));
    }

    #[test]
    fn variable_navigation_adds_to_find_cache() {
        // Variable: matched_contact = Find(contact, Where, [fullname = Copy(name)])
        // Field mapping: Copy("$matched_contact.emailaddress1")
        let variables = vec![(
            "matched_contact".to_string(),
            vec![find_where(
                "contact",
                vec![find_condition("fullname", vec![copy("name")])],
            )],
        )];
        let field_mappings = vec![(
            "email".to_string(),
            vec![copy("$matched_contact.emailaddress1")],
        )];
        let plan = analyze_mapping(&simple_input(&field_mappings, &variables));

        // Find cache for contact should have both fullname (from condition) and emailaddress1 (from nav)
        let cache = plan
            .find_caches
            .iter()
            .find(|c| c.entity == "contact")
            .unwrap();
        assert!(cache.select.contains("fullname"));
        assert!(cache.select.contains("emailaddress1"));
    }

    #[test]
    fn variable_navigation_in_format_template() {
        let variables = vec![(
            "capacity".to_string(),
            vec![find_where(
                "capacity",
                vec![find_condition("capacityid", vec![copy("capacityid")])],
            )],
        )];
        let field_mappings = vec![(
            "description".to_string(),
            vec![format("{$capacity.name} - {name}")],
        )];
        let plan = analyze_mapping(&simple_input(&field_mappings, &variables));

        // Source fields
        assert!(plan.source.select.contains("name"));
        assert!(plan.source.select.contains("capacityid"));

        // Find cache
        let cache = plan
            .find_caches
            .iter()
            .find(|c| c.entity == "capacity")
            .unwrap();
        assert!(cache.select.contains("capacityid")); // from condition target_field
        assert!(cache.select.contains("name")); // from template navigation
    }

    #[test]
    fn guard_condition_extracts_paths() {
        let condition = Condition::IsNotNull(Expr::Path("parentaccountid".to_string()));
        let field_mappings = vec![(
            "parentname".to_string(),
            vec![
                guard(condition, vec![constant(Value::Null)]),
                copy("parentaccountid.name"),
            ],
        )];
        let variables = vec![];
        let plan = analyze_mapping(&simple_input(&field_mappings, &variables));

        assert!(plan.source.select.contains("parentaccountid"));
        // Also the expand from the copy after the guard
        assert_eq!(plan.source.expands.len(), 1);
    }

    #[test]
    fn match_branches_extract_paths() {
        let branch = BranchItem {
            condition: Condition::Compare {
                left: Expr::Path("statuscode".to_string()),
                op: CompareOp::Equal,
                right: Expr::Literal(Value::Int(1)),
            },
            chain: vec![copy("name")],
        };
        let field_mappings = vec![(
            "result".to_string(),
            vec![match_transform(
                vec![branch],
                Some(vec![copy("description")]),
            )],
        )];
        let variables = vec![];
        let plan = analyze_mapping(&simple_input(&field_mappings, &variables));

        assert!(plan.source.select.contains("statuscode")); // from condition
        assert!(plan.source.select.contains("name")); // from branch chain
        assert!(plan.source.select.contains("description")); // from default chain
    }

    #[test]
    fn coalesce_alternatives_extract_paths() {
        let field_mappings = vec![(
            "result".to_string(),
            vec![coalesce(vec![
                vec![copy("preferredname")],
                vec![copy("firstname")],
                vec![copy("name")],
            ])],
        )];
        let variables = vec![];
        let plan = analyze_mapping(&simple_input(&field_mappings, &variables));

        assert!(plan.source.select.contains("preferredname"));
        assert!(plan.source.select.contains("firstname"));
        assert!(plan.source.select.contains("name"));
    }

    #[test]
    fn match_config_chain_analyzed() {
        let field_mappings = vec![];
        let variables = vec![];
        let match_chain = vec![copy("externalid")];
        let input = AnalysisInput {
            source_entity: "account",
            target_entity: "account",
            source_primary_key: "accountid",
            target_primary_key: "accountid",
            field_mappings: &field_mappings,
            variables: &variables,
            match_config_chain: Some(&match_chain),
        };
        let plan = analyze_mapping(&input);

        assert!(plan.source.select.contains("externalid"));
    }

    #[test]
    fn target_spec_includes_field_mapping_targets() {
        let field_mappings = vec![("name".to_string(), vec![copy("name")])];
        let variables = vec![];
        let plan = analyze_mapping(&simple_input(&field_mappings, &variables));

        let target = plan.target.expect("target spec should be present");
        assert!(target.select.contains("accountid")); // PK
        assert!(target.select.contains("name")); // from field mapping
    }

    #[test]
    fn no_target_spec_when_no_field_mappings() {
        let field_mappings = vec![];
        let variables = vec![];
        let plan = analyze_mapping(&simple_input(&field_mappings, &variables));

        // Only PK, which is always added — so target is present
        let target = plan.target.expect("target spec should be present");
        assert!(target.select.contains("accountid"));
    }

    #[test]
    fn empty_find_caches_when_no_finds() {
        let field_mappings = vec![("name".to_string(), vec![copy("name")])];
        let variables = vec![];
        let plan = analyze_mapping(&simple_input(&field_mappings, &variables));

        assert!(plan.find_caches.is_empty());
    }

    #[test]
    fn lua_find_creates_cache_entity_without_fields() {
        let variables = vec![(
            "matched".to_string(),
            vec![ChainItem::new(TransformData::Find {
                entity: "contact".to_string(),
                fallback: FindFallback::Error,
                mode: FindMode::Lua {
                    script: "-- lua script".to_string(),
                },
            })],
        )];
        let field_mappings = vec![];
        let plan = analyze_mapping(&simple_input(&field_mappings, &variables));

        assert_eq!(plan.find_caches.len(), 1);
        assert_eq!(plan.find_caches[0].entity, "contact");
        // Lua find doesn't declare fields statically (yet)
        assert!(plan.find_caches[0].select.is_empty());
    }

    #[test]
    fn deduplicate_fields_across_chains() {
        // Same field referenced multiple times should only appear once
        let field_mappings = vec![
            ("field1".to_string(), vec![copy("name")]),
            ("field2".to_string(), vec![copy("name")]),
            ("field3".to_string(), vec![format("{name} - {name}")]),
        ];
        let variables = vec![];
        let plan = analyze_mapping(&simple_input(&field_mappings, &variables));

        // "name" should only appear once (HashSet dedup)
        let name_count = plan.source.select.iter().filter(|f| *f == "name").count();
        assert_eq!(name_count, 1);
    }

    #[test]
    fn polymorphic_path_creates_expand() {
        let field_mappings = vec![(
            "ownername".to_string(),
            vec![copy("ownerid[systemuser].fullname")],
        )];
        let variables = vec![];
        let plan = analyze_mapping(&simple_input(&field_mappings, &variables));

        assert_eq!(plan.source.expands.len(), 1);
        assert_eq!(plan.source.expands[0].nav_property, "ownerid");
        assert!(plan.source.expands[0].select.contains("fullname"));
    }

    #[test]
    fn condition_compare_extracts_both_sides() {
        let condition = Condition::Compare {
            left: Expr::Path("statuscode".to_string()),
            op: CompareOp::Equal,
            right: Expr::Path("defaultstatuscode".to_string()),
        };
        let field_mappings = vec![(
            "result".to_string(),
            vec![guard(condition, vec![constant(Value::Null)])],
        )];
        let variables = vec![];
        let plan = analyze_mapping(&simple_input(&field_mappings, &variables));

        assert!(plan.source.select.contains("statuscode"));
        assert!(plan.source.select.contains("defaultstatuscode"));
    }

    #[test]
    fn condition_variable_ref_ensures_find_cache() {
        let variables = vec![(
            "matched".to_string(),
            vec![find_where(
                "contact",
                vec![find_condition("name", vec![copy("name")])],
            )],
        )];
        let condition = Condition::IsNotNull(Expr::Variable("matched".to_string()));
        let field_mappings = vec![(
            "result".to_string(),
            vec![guard(condition, vec![constant(Value::Null)])],
        )];
        let plan = analyze_mapping(&simple_input(&field_mappings, &variables));

        // contact should be in find caches
        assert!(plan.find_caches.iter().any(|c| c.entity == "contact"));
    }

    #[test]
    fn multiple_find_entities_tracked_separately() {
        let variables = vec![
            (
                "contact".to_string(),
                vec![find_where(
                    "contact",
                    vec![find_condition("fullname", vec![copy("contactname")])],
                )],
            ),
            (
                "capacity".to_string(),
                vec![find_where(
                    "capacity",
                    vec![find_condition("capacityid", vec![copy("capacityid")])],
                )],
            ),
        ];
        let field_mappings = vec![
            ("email".to_string(), vec![copy("$contact.emailaddress1")]),
            ("capname".to_string(), vec![copy("$capacity.name")]),
        ];
        let plan = analyze_mapping(&simple_input(&field_mappings, &variables));

        assert_eq!(plan.find_caches.len(), 2);

        let contact_cache = plan
            .find_caches
            .iter()
            .find(|c| c.entity == "contact")
            .unwrap();
        assert!(contact_cache.select.contains("fullname"));
        assert!(contact_cache.select.contains("emailaddress1"));

        let capacity_cache = plan
            .find_caches
            .iter()
            .find(|c| c.entity == "capacity")
            .unwrap();
        assert!(capacity_cache.select.contains("capacityid"));
        assert!(capacity_cache.select.contains("name"));
    }

    #[test]
    fn find_where_dotted_target_field_creates_expand() {
        // Find condition with target_field "contact.emailaddress1" should create
        // an expand on the find cache for "account", not a flat select.
        let variables = vec![(
            "matched".to_string(),
            vec![find_where(
                "account",
                vec![find_condition(
                    "primarycontactid.emailaddress1",
                    vec![copy("email")],
                )],
            )],
        )];
        let field_mappings = vec![];
        let plan = analyze_mapping(&simple_input(&field_mappings, &variables));

        let cache = plan
            .find_caches
            .iter()
            .find(|c| c.entity == "account")
            .unwrap();
        // "primarycontactid" should NOT be in flat select
        assert!(!cache.select.contains("primarycontactid"));
        // Should have an expand for "primarycontactid"
        assert_eq!(cache.expands.len(), 1);
        assert_eq!(cache.expands[0].nav_property, "primarycontactid");
        assert!(cache.expands[0].select.contains("emailaddress1"));
    }

    #[test]
    fn find_where_flat_target_field_stays_in_select() {
        // Single-segment target_field should remain in select (no expand)
        let variables = vec![(
            "matched".to_string(),
            vec![find_where(
                "contact",
                vec![find_condition("fullname", vec![copy("name")])],
            )],
        )];
        let field_mappings = vec![];
        let plan = analyze_mapping(&simple_input(&field_mappings, &variables));

        let cache = plan
            .find_caches
            .iter()
            .find(|c| c.entity == "contact")
            .unwrap();
        assert!(cache.select.contains("fullname"));
        assert!(cache.expands.is_empty());
    }

    #[test]
    fn find_where_mixed_flat_and_dotted_target_fields() {
        let variables = vec![(
            "matched".to_string(),
            vec![find_where(
                "account",
                vec![
                    find_condition("name", vec![copy("name")]),
                    find_condition("primarycontactid.emailaddress1", vec![copy("email")]),
                ],
            )],
        )];
        let field_mappings = vec![];
        let plan = analyze_mapping(&simple_input(&field_mappings, &variables));

        let cache = plan
            .find_caches
            .iter()
            .find(|c| c.entity == "account")
            .unwrap();
        // Flat field in select
        assert!(cache.select.contains("name"));
        // Dotted path in expand
        assert_eq!(cache.expands.len(), 1);
        assert_eq!(cache.expands[0].nav_property, "primarycontactid");
        assert!(cache.expands[0].select.contains("emailaddress1"));
    }
}
