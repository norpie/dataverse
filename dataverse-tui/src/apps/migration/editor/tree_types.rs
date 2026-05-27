//! Type computation for the migration editor tree.
//!
//! Resolves types for transform chains using field metadata caches,
//! variable types, and path resolution through lookups.

use std::collections::HashMap;

use dataverse_lib::model::FieldType;
use dataverse_lib::model::ValueType;
use dataverse_lib::model::metadata::AttributeType;

use crate::apps::migration::types::ChainTypeResult;
use crate::apps::migration::types::ParentType;
use crate::apps::migration::types::SystemVar;
use crate::apps::migration::types::Transform;
use crate::apps::migration::types::TransformData;
use crate::apps::migration::types::propagate_chain_types;
use crate::apps::migration::types::resolve_branch_union;
use crate::apps::migration::validation::FieldPath;
use crate::apps::migration::validation::PathExpr;
use crate::apps::migration::validation::parse_path;

use super::tree_builder::TreeBuildContext;

/// Compute chain types using the current type tracking context.
///
/// Resolves `copy($var)` references using already-computed variable types,
/// and `copy(field)` paths using the field type cache from entity metadata.
///
/// `initial_type` sets the starting `#value` type for this chain. Top-level
/// chains use `ValueType::Null`; match/coalesce branch chains pass the
/// parent transform's input type so `copy #value` resolves correctly.
pub(super) fn compute_chain_types(
    transforms: &[Transform],
    source_entity: &str,
    ctx: &mut TreeBuildContext,
    initial_type: ValueType,
) -> ChainTypeResult {
    let variable_types = ctx.types.variable_types.clone();
    let _field_types = ctx.field_type_cache.get(source_entity);

    propagate_chain_types(
        transforms,
        initial_type,
        |transform_id, data, current_type| {
            match data {
                TransformData::Copy { path } => {
                    // Use the structured parser for all path resolution
                    match parse_path(path) {
                        Ok(PathExpr::Variable(var_name)) => Some(
                            variable_types
                                .get(&var_name)
                                .cloned()
                                .unwrap_or(ValueType::Null),
                        ),
                        Ok(PathExpr::VariableNavigation {
                            name,
                            target,
                            path: field_path,
                            ..
                        }) => resolve_variable_navigation(
                            &name,
                            target.as_deref(),
                            &field_path,
                            &variable_types,
                            ctx,
                        ),
                        Ok(PathExpr::SystemVar(sys_var)) => match sys_var {
                            SystemVar::Value => Some(current_type.clone()),
                            SystemVar::Index => Some(ValueType::simple(AttributeType::Integer)),
                            SystemVar::Type | SystemVar::SourceEntity | SystemVar::TargetEntity => {
                                Some(ValueType::simple(AttributeType::String))
                            }
                        },
                        Ok(PathExpr::SystemVarNavigation {
                            var,
                            path: field_path,
                            ..
                        }) => {
                            // #value.field — navigate from #value's type
                            if var != SystemVar::Value {
                                return None;
                            }
                            // current_type is #value's type; resolve the entity then walk the path
                            let targets = match current_type {
                                ValueType::Known(FieldType::Lookup { targets, .. }) => targets,
                                _ => return None,
                            };
                            let target_entity = if targets.len() == 1 {
                                &targets[0]
                            } else {
                                return None;
                            };
                            resolve_variable_navigation_from_entity(target_entity, &field_path, ctx)
                        }
                        Ok(PathExpr::Field(_)) => {
                            // Field path - resolve from metadata cache.
                            resolve_field_path(path, source_entity, ctx)
                        }
                        Ok(PathExpr::EntityRef { entity, .. }) => {
                            // Entity ref produces a Lookup targeting the specified entity
                            Some(ValueType::lookup(
                                AttributeType::Lookup,
                                vec![entity.clone()],
                            ))
                        }
                        Err(_) => None,
                    }
                }
                TransformData::Match { has_default } => {
                    // Compute branch union by recursively resolving each branch's chain
                    let mut branch_output_types = Vec::new();

                    let branches = ctx.lookup.get_match_branches(transform_id);
                    for mb in &branches {
                        let branch_transforms =
                            ctx.lookup.get_transforms(ParentType::MatchBranch, mb.id);
                        if !branch_transforms.is_empty() {
                            let branch_result = compute_chain_types(
                                &branch_transforms,
                                source_entity,
                                ctx,
                                current_type.clone(),
                            );
                            branch_output_types.push(branch_result.output_type.clone());
                            ctx.types.merge(&branch_result);
                        }
                    }

                    if *has_default {
                        let default_transforms = ctx
                            .lookup
                            .get_transforms(ParentType::MatchDefault, transform_id);
                        if !default_transforms.is_empty() {
                            let default_result = compute_chain_types(
                                &default_transforms,
                                source_entity,
                                ctx,
                                current_type.clone(),
                            );
                            branch_output_types.push(default_result.output_type.clone());
                            ctx.types.merge(&default_result);
                        }
                    }

                    if branch_output_types.is_empty() {
                        None // Passthrough
                    } else {
                        Some(resolve_branch_union(&branch_output_types))
                    }
                }
                TransformData::Coalesce => {
                    // Compute chain union by recursively resolving each fallback chain
                    let chains = ctx.lookup.get_coalesce_chains(transform_id);
                    let mut chain_output_types = Vec::new();
                    for cc in &chains {
                        let chain_transforms =
                            ctx.lookup.get_transforms(ParentType::CoalesceChain, cc.id);
                        if !chain_transforms.is_empty() {
                            let chain_result = compute_chain_types(
                                &chain_transforms,
                                source_entity,
                                ctx,
                                current_type.clone(),
                            );
                            chain_output_types.push(chain_result.output_type.clone());
                            ctx.types.merge(&chain_result);
                        }
                    }

                    if chain_output_types.is_empty() {
                        None // Passthrough
                    } else {
                        Some(resolve_branch_union(&chain_output_types))
                    }
                }
                _ => None, // Passthrough for all other dynamic cases (Guard, etc.)
            }
        },
    )
}

/// Resolve a variable navigation path (`$var.field` or `$var[target].field`).
///
/// Looks up the variable's declared type, determines the target entity from
/// its Lookup type, then resolves the remaining field path against that entity.
fn resolve_variable_navigation(
    var_name: &str,
    target: Option<&str>,
    field_path: &FieldPath,
    variable_types: &HashMap<String, ValueType>,
    ctx: &TreeBuildContext,
) -> Option<ValueType> {
    let var_type = variable_types.get(var_name)?;

    // The variable must be a lookup to navigate into it
    let targets = match var_type {
        ValueType::Known(FieldType::Lookup { targets, .. }) => targets,
        ValueType::Union(types) => {
            // Find the first lookup in the union

            types.iter().find_map(|ft| match ft {
                FieldType::Lookup { targets, .. } => Some(targets),
                _ => None,
            })?
        }
        _ => {
            log::debug!(
                "type_tracking: variable ${} is not a lookup, cannot navigate into it",
                var_name,
            );
            return None;
        }
    };

    // Determine target entity
    let target_entity = if let Some(specified) = target {
        // Explicit target: $var[account].name
        if !targets.is_empty() && !targets.contains(&specified.to_string()) {
            log::debug!(
                "type_tracking: specified target '{}' not in targets {:?} for variable ${}",
                specified,
                targets,
                var_name,
            );
            return None;
        }
        specified.to_string()
    } else if targets.len() == 1 {
        targets[0].clone()
    } else if targets.is_empty() {
        log::debug!(
            "type_tracking: variable ${} lookup has no known targets, cannot navigate",
            var_name,
        );
        return None;
    } else {
        log::debug!(
            "type_tracking: variable ${} is polymorphic (targets: {:?}), use ${}[target].field syntax",
            var_name,
            targets,
            var_name,
        );
        return None;
    };

    log::debug!(
        "type_tracking: resolving ${}.{} via entity '{}'",
        var_name,
        field_path
            .segments
            .iter()
            .map(|s| s.field.as_str())
            .collect::<Vec<_>>()
            .join("."),
        target_entity,
    );

    // Resolve the field path starting from the target entity
    if field_path.segments.len() == 1 {
        let segment = &field_path.segments[0];
        let fields = ctx.field_type_cache.get(&target_entity)?;
        let field_type = fields.get(&segment.field)?;
        Some(ValueType::Known(field_type.clone()))
    } else {
        resolve_dotted_field_path(
            field_path,
            &target_entity,
            ctx,
            &format!(
                "${}.{}",
                var_name,
                field_path
                    .segments
                    .iter()
                    .map(|s| s.field.as_str())
                    .collect::<Vec<_>>()
                    .join(".")
            ),
        )
    }
}

/// Resolve a field path starting from a known entity.
///
/// Used for `#value.field` navigation where the starting entity is derived
/// from `#value`'s type.
fn resolve_variable_navigation_from_entity(
    target_entity: &str,
    field_path: &FieldPath,
    ctx: &TreeBuildContext,
) -> Option<ValueType> {
    if field_path.segments.len() == 1 {
        let segment = &field_path.segments[0];
        let fields = ctx.field_type_cache.get(target_entity)?;
        let field_type = fields.get(&segment.field)?;
        Some(ValueType::Known(field_type.clone()))
    } else {
        let path_str = format!(
            "#value.{}",
            field_path
                .segments
                .iter()
                .map(|s| s.field.as_str())
                .collect::<Vec<_>>()
                .join(".")
        );
        resolve_dotted_field_path(field_path, target_entity, ctx, &path_str)
    }
}

/// Resolve a field path (possibly dotted) to its `ValueType` using the field type cache.
///
/// For simple paths like `name`, looks up the field directly on the source entity.
/// For dotted paths like `parentaccountid.name`, walks segment-by-segment:
/// each navigation segment must be a lookup, and its target entity is used to
/// resolve the next segment.
fn resolve_field_path(
    path: &str,
    source_entity: &str,
    ctx: &TreeBuildContext,
) -> Option<ValueType> {
    // Parse the path to get structured segments
    let field_path = match parse_path(path) {
        Ok(PathExpr::Field(fp)) => fp,
        _ => {
            // Not a field path (variable/system var handled elsewhere), or parse error
            log::debug!("type_tracking: failed to parse field path '{}'", path,);
            return None;
        }
    };

    if field_path.segments.is_empty() {
        return None;
    }

    // Simple (non-dotted) path: single segment lookup
    if field_path.segments.len() == 1 {
        let segment = &field_path.segments[0];
        let fields = ctx.field_type_cache.get(source_entity)?;
        let field_type = fields.get(&segment.field)?;
        log::debug!(
            "type_tracking: resolved field '{}' -> {:?}",
            path,
            field_type,
        );
        return Some(ValueType::Known(field_type.clone()));
    }

    // Dotted path: walk segment-by-segment
    resolve_dotted_field_path(&field_path, source_entity, ctx, path)
}

/// Walk a dotted field path segment-by-segment through the field type cache.
fn resolve_dotted_field_path(
    field_path: &FieldPath,
    source_entity: &str,
    ctx: &TreeBuildContext,
    original_path: &str,
) -> Option<ValueType> {
    let mut current_entity = source_entity.to_string();

    for (i, segment) in field_path.segments.iter().enumerate() {
        let is_last = i == field_path.segments.len() - 1;

        let fields = match ctx.field_type_cache.get(&current_entity) {
            Some(f) => f,
            None => {
                log::debug!(
                    "type_tracking: no metadata cached for entity '{}' while resolving '{}'",
                    current_entity,
                    original_path,
                );
                return None;
            }
        };

        let field_type = match fields.get(&segment.field) {
            Some(ft) => ft,
            None => {
                log::debug!(
                    "type_tracking: field '{}' not found on '{}' while resolving '{}'",
                    segment.field,
                    current_entity,
                    original_path,
                );
                return None;
            }
        };

        if is_last {
            // Last segment — this is the leaf field, return its type
            log::debug!(
                "type_tracking: resolved dotted path '{}' -> {:?}",
                original_path,
                field_type,
            );
            return Some(ValueType::Known(field_type.clone()));
        }

        // Navigation segment — must be a lookup
        let targets = match field_type {
            FieldType::Lookup { targets, .. } => targets,
            FieldType::Simple(_) | FieldType::OptionSet { .. } => {
                log::debug!(
                    "type_tracking: field '{}' on '{}' is not a lookup, cannot navigate in '{}'",
                    segment.field,
                    current_entity,
                    original_path,
                );
                return None;
            }
        };

        // Determine the target entity to navigate to
        let next_entity = if let Some(specified) = &segment.target {
            // Polymorphic lookup with explicit target: ownerid[systemuser]
            if !targets.is_empty() && !targets.contains(specified) {
                log::debug!(
                    "type_tracking: specified target '{}' not in targets {:?} for '{}' on '{}'",
                    specified,
                    targets,
                    segment.field,
                    current_entity,
                );
                return None;
            }
            specified.clone()
        } else if targets.len() == 1 {
            // Single-target lookup
            targets[0].clone()
        } else if targets.is_empty() {
            // Unknown targets — can't navigate
            log::debug!(
                "type_tracking: lookup '{}' on '{}' has no known targets, cannot navigate in '{}'",
                segment.field,
                current_entity,
                original_path,
            );
            return None;
        } else {
            // Polymorphic lookup without explicit target — ambiguous
            log::debug!(
                "type_tracking: polymorphic lookup '{}' on '{}' requires target specifier (targets: {:?}) in '{}'",
                segment.field,
                current_entity,
                targets,
                original_path,
            );
            return None;
        };

        current_entity = next_entity;
    }

    None
}
