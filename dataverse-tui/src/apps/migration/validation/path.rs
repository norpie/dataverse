//! Path expression parsing and validation for transforms.
//!
//! Supports three types of paths:
//! - Field paths: `name`, `parentaccountid.name`, `ownerid[systemuser].fullname`
//! - Variables: `$my_var`
//! - System variables: `#value`, `#type`, `#index`

use std::collections::HashMap;

use dataverse_lib::DataverseClient;
use dataverse_lib::model::FieldType;
use dataverse_lib::model::ValueType;
use dataverse_lib::model::metadata::AttributeType;

use crate::apps::migration::types::SystemVar;

// =============================================================================
// Path AST
// =============================================================================

/// A parsed path expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathExpr {
    /// Field path: `name`, `parentaccountid.name`, `ownerid[systemuser].fullname`
    Field(FieldPath),
    /// Variable reference: `$my_var`
    Variable(String),
    /// Variable with field navigation: `$my_var.field`, `$my_var[account].name`
    VariableNavigation {
        /// The variable name (without `$` prefix).
        name: String,
        /// For polymorphic lookups, the target entity (e.g., `$var[account].name`).
        target: Option<String>,
        /// The field path to navigate after resolving the variable.
        path: FieldPath,
    },
    /// System variable: `#value`, `#type`, `#index`
    SystemVar(SystemVar),
    /// System variable with field navigation: `#value.field`, `#value.lookup.field`
    SystemVarNavigation {
        /// The system variable (only `Value` supports navigation).
        var: SystemVar,
        /// The field path to navigate after resolving the system variable.
        path: FieldPath,
    },
    /// Entity reference construction: `/contact($var)`, `/account(parentaccountid)`
    ///
    /// Resolves the inner path to a UUID and wraps it in an EntityReference.
    EntityRef {
        /// The target entity logical name.
        entity: String,
        /// The inner path expression that resolves to a UUID.
        inner: Box<PathExpr>,
    },
}

/// A field path with dot-separated segments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldPath {
    pub segments: Vec<FieldSegment>,
}

/// A single segment in a field path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldSegment {
    /// The field name (e.g., "parentcustomerid").
    pub field: String,
    /// For polymorphic lookups, the target entity (e.g., "account").
    pub target: Option<String>,
    /// Whether this segment allows null propagation (`?` suffix).
    pub optional: bool,
}

// =============================================================================
// Parse Errors
// =============================================================================

/// Error from parsing a path expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Empty path.
    Empty,
    /// Invalid variable name (empty after `$`).
    EmptyVariable,
    /// Invalid system variable name (empty after `#`).
    EmptySystemVar,
    /// Unknown system variable.
    UnknownSystemVar(String),
    /// Empty field name in path.
    EmptyFieldName,
    /// Unclosed bracket in polymorphic target.
    UnclosedBracket,
    /// Empty target in brackets.
    EmptyTarget,
    /// Empty entity name in entity ref (after `/`).
    EmptyEntityName,
    /// Missing opening parenthesis in entity ref.
    MissingOpenParen,
    /// Missing closing parenthesis in entity ref.
    UnclosedParen,
    /// Empty inner path in entity ref.
    EmptyInnerPath,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Empty => write!(f, "Path cannot be empty"),
            ParseError::EmptyVariable => write!(f, "Variable name cannot be empty"),
            ParseError::EmptySystemVar => write!(f, "System variable name cannot be empty"),
            ParseError::UnknownSystemVar(name) => {
                write!(f, "Unknown system variable '#{}'", name)
            }
            ParseError::EmptyFieldName => write!(f, "Field name cannot be empty"),
            ParseError::UnclosedBracket => write!(f, "Unclosed bracket in target specifier"),
            ParseError::EmptyTarget => write!(f, "Target entity in brackets cannot be empty"),
            ParseError::EmptyEntityName => write!(f, "Entity name cannot be empty after '/'"),
            ParseError::MissingOpenParen => {
                write!(f, "Expected '(' after entity name in entity ref")
            }
            ParseError::UnclosedParen => write!(f, "Unclosed parenthesis in entity ref"),
            ParseError::EmptyInnerPath => {
                write!(f, "Inner path cannot be empty in entity ref")
            }
        }
    }
}

impl std::error::Error for ParseError {}

// =============================================================================
// Parser
// =============================================================================

/// Parse a path string into a PathExpr.
///
/// # Examples
///
/// ```ignore
/// parse_path("name")                         // Field path, single segment
/// parse_path("parentaccountid.name")         // Field path, lookup navigation
/// parse_path("ownerid[systemuser].fullname") // Field path, polymorphic lookup
/// parse_path("lookup?.field")                // Field path, optional lookup
/// parse_path("$my_var")                      // Variable
/// parse_path("#value")                       // System variable
/// ```
pub fn parse_path(input: &str) -> Result<PathExpr, ParseError> {
    let input = input.trim();

    if input.is_empty() {
        return Err(ParseError::Empty);
    }

    // Entity reference: /entity(path)
    if let Some(rest) = input.strip_prefix('/') {
        return parse_entity_ref(rest);
    }

    // Variable: $name, $name.field, $name[target].field
    if let Some(rest) = input.strip_prefix('$') {
        let rest = rest.trim();
        if rest.is_empty() {
            return Err(ParseError::EmptyVariable);
        }
        return parse_variable_path(rest);
    }

    // System variable: #name or #name.field (navigation)
    if let Some(rest) = input.strip_prefix('#') {
        let rest = rest.trim();
        if rest.is_empty() {
            return Err(ParseError::EmptySystemVar);
        }

        // Check for dot navigation: #value.field
        if let Some(dot_pos) = rest.find('.') {
            let var_name = &rest[..dot_pos];
            let sys_var = parse_system_var(var_name)?;
            let field_part = &rest[dot_pos + 1..];
            if field_part.is_empty() {
                return Err(ParseError::EmptyFieldName);
            }
            let segments = parse_field_path(field_part)?;
            return Ok(PathExpr::SystemVarNavigation {
                var: sys_var,
                path: FieldPath { segments },
            });
        }

        let sys_var = parse_system_var(rest)?;
        return Ok(PathExpr::SystemVar(sys_var));
    }

    // Field path: field.field.field
    let segments = parse_field_path(input)?;
    Ok(PathExpr::Field(FieldPath { segments }))
}

/// Parse an entity reference: `entity(inner_path)`.
///
/// `input` is everything after the `/` prefix.
fn parse_entity_ref(input: &str) -> Result<PathExpr, ParseError> {
    let input = input.trim();

    // Find opening paren
    let paren_pos = input.find('(').ok_or(ParseError::MissingOpenParen)?;
    let entity = input[..paren_pos].trim();
    if entity.is_empty() {
        return Err(ParseError::EmptyEntityName);
    }

    // Find closing paren (must be at the end)
    let after_paren = &input[paren_pos + 1..];
    let close_pos = after_paren.rfind(')').ok_or(ParseError::UnclosedParen)?;
    let inner_str = after_paren[..close_pos].trim();
    if inner_str.is_empty() {
        return Err(ParseError::EmptyInnerPath);
    }

    // Parse the inner path recursively (supports $var, field, $var.field, etc.)
    let inner = parse_path(inner_str)?;

    Ok(PathExpr::EntityRef {
        entity: entity.to_string(),
        inner: Box::new(inner),
    })
}

/// Parse a variable path: `name`, `name.field`, `name[target].field`.
fn parse_variable_path(input: &str) -> Result<PathExpr, ParseError> {
    // Check for bracket target on the variable itself: $var[target].field
    let (name, target, remainder) = if let Some(bracket_start) = input.find('[') {
        let name = &input[..bracket_start];
        if name.is_empty() {
            return Err(ParseError::EmptyVariable);
        }
        let after_bracket = &input[bracket_start + 1..];
        let bracket_end = after_bracket.find(']').ok_or(ParseError::UnclosedBracket)?;
        let target = &after_bracket[..bracket_end];
        if target.is_empty() {
            return Err(ParseError::EmptyTarget);
        }
        let remainder = &after_bracket[bracket_end + 1..];
        (name, Some(target.to_string()), remainder)
    } else if let Some(dot_pos) = input.find('.') {
        let name = &input[..dot_pos];
        if name.is_empty() {
            return Err(ParseError::EmptyVariable);
        }
        (name, None, &input[dot_pos..])
    } else {
        // Plain variable, no navigation
        return Ok(PathExpr::Variable(input.to_string()));
    };

    // Must have a dot followed by field path
    let remainder = remainder
        .strip_prefix('.')
        .ok_or(ParseError::EmptyFieldName)?;
    if remainder.is_empty() {
        return Err(ParseError::EmptyFieldName);
    }

    let segments = parse_field_path(remainder)?;
    Ok(PathExpr::VariableNavigation {
        name: name.to_string(),
        target,
        path: FieldPath { segments },
    })
}

/// Parse a system variable name.
fn parse_system_var(name: &str) -> Result<SystemVar, ParseError> {
    match name.to_lowercase().as_str() {
        "value" => Ok(SystemVar::Value),
        "type" => Ok(SystemVar::Type),
        "index" => Ok(SystemVar::Index),
        "source_entity" | "sourceentity" => Ok(SystemVar::SourceEntity),
        "target_entity" | "targetentity" => Ok(SystemVar::TargetEntity),
        _ => Err(ParseError::UnknownSystemVar(name.to_string())),
    }
}

/// Parse a dot-separated field path.
fn parse_field_path(input: &str) -> Result<Vec<FieldSegment>, ParseError> {
    let mut segments = Vec::new();

    for part in input.split('.') {
        let segment = parse_field_segment(part)?;
        segments.push(segment);
    }

    if segments.is_empty() {
        return Err(ParseError::EmptyFieldName);
    }

    Ok(segments)
}

/// Parse a single field segment.
///
/// Formats:
/// - `field` - simple field
/// - `field?` - optional field
/// - `field[target]` - polymorphic with target
/// - `field[target]?` - polymorphic with target, optional
fn parse_field_segment(input: &str) -> Result<FieldSegment, ParseError> {
    let input = input.trim();

    if input.is_empty() {
        return Err(ParseError::EmptyFieldName);
    }

    // Check for optional suffix
    let (input, optional) = if let Some(rest) = input.strip_suffix('?') {
        (rest, true)
    } else {
        (input, false)
    };

    // Check for bracket target
    if let Some(bracket_start) = input.find('[') {
        let field = &input[..bracket_start];
        if field.is_empty() {
            return Err(ParseError::EmptyFieldName);
        }

        let rest = &input[bracket_start + 1..];
        let bracket_end = rest.find(']').ok_or(ParseError::UnclosedBracket)?;
        let target = &rest[..bracket_end];

        if target.is_empty() {
            return Err(ParseError::EmptyTarget);
        }

        Ok(FieldSegment {
            field: field.to_string(),
            target: Some(target.to_string()),
            optional,
        })
    } else {
        Ok(FieldSegment {
            field: input.to_string(),
            target: None,
            optional,
        })
    }
}

// =============================================================================
// Validation
// =============================================================================

/// Context for path validation.
pub struct ValidationContext {
    /// The source entity logical name.
    pub source_entity: String,
    /// Variable types keyed by name (without `$` prefix).
    /// Used for both existence checks and navigation validation.
    pub variable_types: HashMap<String, ValueType>,
}

/// Result of path validation.
#[derive(Debug, Clone)]
pub enum ValidationResult {
    /// Path is valid.
    Valid(ValidPath),
    /// Path is invalid.
    Invalid(String),
    /// Validation is in progress.
    Loading,
}

/// Information about a valid path.
#[derive(Debug, Clone)]
pub struct ValidPath {
    /// Human-readable description of the path.
    /// e.g., "contact → parentcustomerid (account) → name (String)"
    pub description: String,
    /// The final value type, if known.
    pub value_type: Option<AttributeType>,
}

/// Validator for path expressions.
pub struct PathValidator {
    client: DataverseClient,
    /// Target environment client for entity ref validation.
    target_client: DataverseClient,
}

impl PathValidator {
    /// Create a new path validator.
    pub fn new(client: DataverseClient, target_client: DataverseClient) -> Self {
        Self {
            client,
            target_client,
        }
    }

    /// Fetch entity metadata, trying the source client first, then the target client.
    ///
    /// Handles entities that may only exist in the target environment
    /// (e.g., `#value` after a Find resolves to a target entity).
    async fn fetch_metadata(
        &self,
        entity: &str,
    ) -> Option<dataverse_lib::model::metadata::EntityMetadata> {
        match self.client.metadata().entity(entity).await {
            Ok(m) => Some(m),
            Err(_) => self.target_client.metadata().entity(entity).await.ok(),
        }
    }

    /// Validate a path expression.
    pub async fn validate(&self, path: &str, ctx: &ValidationContext) -> ValidationResult {
        // Parse the path
        let parsed = match parse_path(path) {
            Ok(p) => p,
            Err(e) => return ValidationResult::Invalid(e.to_string()),
        };

        match parsed {
            PathExpr::Variable(name) => self.validate_variable(&name, ctx),
            PathExpr::VariableNavigation { name, target, path } => {
                self.validate_variable_navigation(&name, target.as_deref(), &path, ctx)
                    .await
            }
            PathExpr::SystemVar(var) => self.validate_system_var(var),
            PathExpr::SystemVarNavigation { var, path } => {
                self.validate_system_var_navigation(var, &path, ctx).await
            }
            PathExpr::Field(field_path) => self.validate_field_path(&field_path, ctx).await,
            PathExpr::EntityRef { entity, inner } => {
                self.validate_entity_ref(&entity, &inner, ctx).await
            }
        }
    }

    /// Validate a variable reference.
    fn validate_variable(&self, name: &str, ctx: &ValidationContext) -> ValidationResult {
        match ctx.variable_types.get(name) {
            Some(vt) => {
                let attr_type = match vt {
                    ValueType::Known(ft) => Some(ft.attribute_type()),
                    _ => None,
                };
                ValidationResult::Valid(ValidPath {
                    description: format!("Variable: ${} ({})", name, vt),
                    value_type: attr_type,
                })
            }
            None => ValidationResult::Invalid(format!("Variable '{}' is not defined", name)),
        }
    }

    /// Validate a system variable.
    fn validate_system_var(&self, var: SystemVar) -> ValidationResult {
        let description = match var {
            SystemVar::Value => "System: #value (current pipeline value)",
            SystemVar::Type => "System: #type (value type)",
            SystemVar::Index => "System: #index (record index)",
            SystemVar::SourceEntity => "System: #source_entity",
            SystemVar::TargetEntity => "System: #target_entity",
        };
        ValidationResult::Valid(ValidPath {
            description: description.to_string(),
            value_type: None,
        })
    }

    /// Validate a system variable with field navigation: `#value.field`.
    async fn validate_system_var_navigation(
        &self,
        var: SystemVar,
        field_path: &FieldPath,
        ctx: &ValidationContext,
    ) -> ValidationResult {
        // Only #value supports navigation
        if var != SystemVar::Value {
            return ValidationResult::Invalid(format!(
                "System variable #{:?} does not support field navigation",
                var
            ));
        }

        // Look up #value's type from context (pushed as a VariableInfo)
        let value_type = match ctx.variable_types.get("#value") {
            Some(vt) => vt,
            None => {
                return ValidationResult::Invalid(
                    "#value type is unknown in this context — cannot navigate fields".to_string(),
                );
            }
        };

        // #value must be a Lookup to navigate into
        let targets = match value_type {
            ValueType::Known(FieldType::Lookup { targets, .. }) => targets,
            _ => {
                return ValidationResult::Invalid(format!(
                    "#value has type {} which is not a Lookup — cannot navigate fields",
                    value_type
                ));
            }
        };

        if targets.is_empty() {
            return ValidationResult::Invalid(
                "#value is a Lookup with no target entities".to_string(),
            );
        }

        let start_entity = if targets.len() == 1 {
            targets[0].clone()
        } else {
            return ValidationResult::Invalid(format!(
                "#value is polymorphic (targets: {}) — field navigation not supported",
                targets.join(", ")
            ));
        };

        let mut path_parts = vec![format!("#value ({})", start_entity)];

        self.validate_field_path_from(&start_entity, field_path, &mut path_parts)
            .await
    }

    /// Validate a variable navigation path: `$var.field`, `$var[target].field`.
    async fn validate_variable_navigation(
        &self,
        name: &str,
        target: Option<&str>,
        field_path: &FieldPath,
        ctx: &ValidationContext,
    ) -> ValidationResult {
        // Look up the variable's type to determine the starting entity
        let var_type = match ctx.variable_types.get(name) {
            Some(vt) => vt,
            None => {
                return ValidationResult::Invalid(format!("Variable '{}' is not defined", name));
            }
        };

        // Variable must be a Lookup to navigate into
        let targets = match var_type {
            ValueType::Known(FieldType::Lookup { targets, .. }) => targets,
            _ => {
                return ValidationResult::Invalid(format!(
                    "Variable '{}' has type {} which is not a Lookup — cannot navigate fields",
                    name, var_type
                ));
            }
        };

        if targets.is_empty() {
            return ValidationResult::Invalid(format!(
                "Variable '{}' is a Lookup with no target entities",
                name
            ));
        }

        // Resolve the start entity (handle polymorphic disambiguation)
        let start_entity = if let Some(specified) = target {
            if targets.contains(&specified.to_string()) {
                specified.to_string()
            } else {
                return ValidationResult::Invalid(format!(
                    "'{}' is not a valid target for variable '{}'. Valid targets: {}",
                    specified,
                    name,
                    targets.join(", ")
                ));
            }
        } else if targets.len() == 1 {
            targets[0].clone()
        } else {
            return ValidationResult::Invalid(format!(
                "Variable '{}' is polymorphic. Specify target with ${}[{}]",
                name,
                name,
                targets.join("|")
            ));
        };

        // Build path description prefix
        let target_label = target.map(|t| format!("[{}]", t)).unwrap_or_default();
        let mut path_parts = vec![format!("${}{} ({})", name, target_label, start_entity)];

        // Validate the field path starting from the resolved entity
        self.validate_field_path_from(&start_entity, field_path, &mut path_parts)
            .await
    }

    /// Validate an entity reference: `/entity(inner_path)`.
    async fn validate_entity_ref(
        &self,
        entity: &str,
        inner: &PathExpr,
        ctx: &ValidationContext,
    ) -> ValidationResult {
        // Validate the entity exists in the target environment
        if let Err(e) = self.target_client.metadata().entity(entity).await {
            return ValidationResult::Invalid(format!(
                "Entity '{}' not found: {}",
                entity, e
            ));
        }

        // Validate the inner path
        let inner_result = match inner {
            PathExpr::Variable(name) => self.validate_variable(name, ctx),
            PathExpr::VariableNavigation { name, target, path } => {
                self.validate_variable_navigation(name, target.as_deref(), path, ctx)
                    .await
            }
            PathExpr::SystemVar(var) => self.validate_system_var(*var),
            PathExpr::SystemVarNavigation { var, path } => {
                self.validate_system_var_navigation(*var, path, ctx).await
            }
            PathExpr::Field(field_path) => self.validate_field_path(field_path, ctx).await,
            PathExpr::EntityRef { entity: e, inner: i } => {
                Box::pin(self.validate_entity_ref(e, i, ctx)).await
            }
        };

        // Check the inner path resolved to a Guid-compatible type
        match inner_result {
            ValidationResult::Valid(valid_inner) => {
                let is_guid_compatible = match valid_inner.value_type {
                    Some(AttributeType::Uniqueidentifier) => true,
                    Some(AttributeType::Lookup) => true,
                    Some(AttributeType::Customer) => true,
                    Some(AttributeType::Owner) => true,
                    None => true, // Unknown type (variables, system vars) — allow
                    _ => false,
                };

                if !is_guid_compatible {
                    return ValidationResult::Invalid(format!(
                        "Inner path resolves to {:?}, expected a Guid or Lookup type",
                        valid_inner.value_type,
                    ));
                }

                ValidationResult::Valid(ValidPath {
                    description: format!(
                        "EntityRef: /{} → {}",
                        entity, valid_inner.description
                    ),
                    value_type: Some(AttributeType::Lookup),
                })
            }
            other => other,
        }
    }

    /// Validate a field path starting from the source entity in ctx.
    async fn validate_field_path(
        &self,
        field_path: &FieldPath,
        ctx: &ValidationContext,
    ) -> ValidationResult {
        let mut path_parts: Vec<String> = vec![ctx.source_entity.clone()];
        self.validate_field_path_from(&ctx.source_entity, field_path, &mut path_parts)
            .await
    }

    /// Core field path validation: walk segments from a starting entity.
    async fn validate_field_path_from(
        &self,
        start_entity: &str,
        field_path: &FieldPath,
        path_parts: &mut Vec<String>,
    ) -> ValidationResult {
        let mut current_entity = start_entity.to_string();

        for (i, segment) in field_path.segments.iter().enumerate() {
            let is_last = i == field_path.segments.len() - 1;

            // Fetch entity metadata (try source, then target environment)
            let metadata = match self.fetch_metadata(current_entity.as_str()).await {
                Some(m) => m,
                None => {
                    return ValidationResult::Invalid(format!(
                        "Entity '{}' not found in either environment",
                        current_entity
                    ));
                }
            };

            // Find the attribute
            let attr = match metadata.attribute(&segment.field) {
                Some(a) => a,
                None => {
                    return ValidationResult::Invalid(format!(
                        "Field '{}' not found on entity '{}'",
                        segment.field, current_entity
                    ));
                }
            };

            if is_last {
                // Last segment - this is the leaf field
                let type_str = format!("{:?}", attr.attribute_type);
                path_parts.push(format!("{} ({})", segment.field, type_str));

                return ValidationResult::Valid(ValidPath {
                    description: path_parts.join(" → "),
                    value_type: Some(attr.attribute_type),
                });
            } else {
                // Not the last segment - must be a lookup
                if !is_lookup_type(attr.attribute_type) {
                    return ValidationResult::Invalid(format!(
                        "Field '{}' on '{}' is not a lookup (type: {:?})",
                        segment.field, current_entity, attr.attribute_type
                    ));
                }

                let targets = &attr.targets;
                if targets.is_empty() {
                    return ValidationResult::Invalid(format!(
                        "Lookup '{}' has no target entities defined",
                        segment.field
                    ));
                }

                // Determine the target entity
                let target_entity = if targets.len() > 1 {
                    // Polymorphic lookup - need target specifier
                    match &segment.target {
                        Some(specified) => {
                            if targets.contains(specified) {
                                specified.clone()
                            } else {
                                return ValidationResult::Invalid(format!(
                                    "'{}' is not a valid target for '{}'. Valid targets: {}",
                                    specified,
                                    segment.field,
                                    targets.join(", ")
                                ));
                            }
                        }
                        None => {
                            return ValidationResult::Invalid(format!(
                                "Lookup '{}' is polymorphic. Specify target with [{}]",
                                segment.field,
                                targets.join("|")
                            ));
                        }
                    }
                } else {
                    // Single target
                    targets[0].clone()
                };

                path_parts.push(format!("{} ({})", segment.field, target_entity));
                current_entity = target_entity;
            }
        }

        // Should not reach here, but just in case
        ValidationResult::Invalid("Empty field path".to_string())
    }
}

/// Check if an attribute type is a lookup type.
fn is_lookup_type(attr_type: AttributeType) -> bool {
    matches!(
        attr_type,
        AttributeType::Lookup | AttributeType::Customer | AttributeType::Owner
    )
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_field() {
        let result = parse_path("name").unwrap();
        assert_eq!(
            result,
            PathExpr::Field(FieldPath {
                segments: vec![FieldSegment {
                    field: "name".to_string(),
                    target: None,
                    optional: false,
                }]
            })
        );
    }

    #[test]
    fn parse_dotted_field() {
        let result = parse_path("parentaccountid.name").unwrap();
        assert_eq!(
            result,
            PathExpr::Field(FieldPath {
                segments: vec![
                    FieldSegment {
                        field: "parentaccountid".to_string(),
                        target: None,
                        optional: false,
                    },
                    FieldSegment {
                        field: "name".to_string(),
                        target: None,
                        optional: false,
                    }
                ]
            })
        );
    }

    #[test]
    fn parse_polymorphic_lookup() {
        let result = parse_path("ownerid[systemuser].fullname").unwrap();
        assert_eq!(
            result,
            PathExpr::Field(FieldPath {
                segments: vec![
                    FieldSegment {
                        field: "ownerid".to_string(),
                        target: Some("systemuser".to_string()),
                        optional: false,
                    },
                    FieldSegment {
                        field: "fullname".to_string(),
                        target: None,
                        optional: false,
                    }
                ]
            })
        );
    }

    #[test]
    fn parse_optional_field() {
        let result = parse_path("parentaccountid?.name").unwrap();
        assert_eq!(
            result,
            PathExpr::Field(FieldPath {
                segments: vec![
                    FieldSegment {
                        field: "parentaccountid".to_string(),
                        target: None,
                        optional: true,
                    },
                    FieldSegment {
                        field: "name".to_string(),
                        target: None,
                        optional: false,
                    }
                ]
            })
        );
    }

    #[test]
    fn parse_polymorphic_optional() {
        let result = parse_path("ownerid[team]?.name").unwrap();
        assert_eq!(
            result,
            PathExpr::Field(FieldPath {
                segments: vec![
                    FieldSegment {
                        field: "ownerid".to_string(),
                        target: Some("team".to_string()),
                        optional: true,
                    },
                    FieldSegment {
                        field: "name".to_string(),
                        target: None,
                        optional: false,
                    }
                ]
            })
        );
    }

    #[test]
    fn parse_variable() {
        let result = parse_path("$my_var").unwrap();
        assert_eq!(result, PathExpr::Variable("my_var".to_string()));
    }

    #[test]
    fn parse_variable_navigation() {
        let result = parse_path("$my_var.name").unwrap();
        assert_eq!(
            result,
            PathExpr::VariableNavigation {
                name: "my_var".to_string(),
                target: None,
                path: FieldPath {
                    segments: vec![FieldSegment {
                        field: "name".to_string(),
                        target: None,
                        optional: false,
                    }]
                },
            }
        );
    }

    #[test]
    fn parse_variable_navigation_dotted() {
        let result = parse_path("$my_var.parentaccountid.name").unwrap();
        assert_eq!(
            result,
            PathExpr::VariableNavigation {
                name: "my_var".to_string(),
                target: None,
                path: FieldPath {
                    segments: vec![
                        FieldSegment {
                            field: "parentaccountid".to_string(),
                            target: None,
                            optional: false,
                        },
                        FieldSegment {
                            field: "name".to_string(),
                            target: None,
                            optional: false,
                        },
                    ]
                },
            }
        );
    }

    #[test]
    fn parse_variable_navigation_polymorphic() {
        let result = parse_path("$my_var[account].name").unwrap();
        assert_eq!(
            result,
            PathExpr::VariableNavigation {
                name: "my_var".to_string(),
                target: Some("account".to_string()),
                path: FieldPath {
                    segments: vec![FieldSegment {
                        field: "name".to_string(),
                        target: None,
                        optional: false,
                    }]
                },
            }
        );
    }

    #[test]
    fn parse_system_var_value() {
        let result = parse_path("#value").unwrap();
        assert_eq!(result, PathExpr::SystemVar(SystemVar::Value));
    }

    #[test]
    fn parse_system_var_index() {
        let result = parse_path("#index").unwrap();
        assert_eq!(result, PathExpr::SystemVar(SystemVar::Index));
    }

    #[test]
    fn parse_empty_error() {
        let result = parse_path("");
        assert_eq!(result, Err(ParseError::Empty));
    }

    #[test]
    fn parse_empty_variable_error() {
        let result = parse_path("$");
        assert_eq!(result, Err(ParseError::EmptyVariable));
    }

    #[test]
    fn parse_unclosed_bracket_error() {
        let result = parse_path("field[target");
        assert_eq!(result, Err(ParseError::UnclosedBracket));
    }

    #[test]
    fn parse_empty_target_error() {
        let result = parse_path("field[].name");
        assert_eq!(result, Err(ParseError::EmptyTarget));
    }

    // =========================================================================
    // Entity ref paths
    // =========================================================================

    #[test]
    fn parse_entity_ref_with_variable() {
        let result = parse_path("/contact($my_var)").unwrap();
        assert_eq!(
            result,
            PathExpr::EntityRef {
                entity: "contact".to_string(),
                inner: Box::new(PathExpr::Variable("my_var".to_string())),
            }
        );
    }

    #[test]
    fn parse_entity_ref_with_field() {
        let result = parse_path("/account(parentaccountid)").unwrap();
        assert_eq!(
            result,
            PathExpr::EntityRef {
                entity: "account".to_string(),
                inner: Box::new(PathExpr::Field(FieldPath {
                    segments: vec![FieldSegment {
                        field: "parentaccountid".to_string(),
                        target: None,
                        optional: false,
                    }]
                })),
            }
        );
    }

    #[test]
    fn parse_entity_ref_with_dotted_field() {
        let result = parse_path("/account(parentaccountid.accountid)").unwrap();
        assert_eq!(
            result,
            PathExpr::EntityRef {
                entity: "account".to_string(),
                inner: Box::new(PathExpr::Field(FieldPath {
                    segments: vec![
                        FieldSegment {
                            field: "parentaccountid".to_string(),
                            target: None,
                            optional: false,
                        },
                        FieldSegment {
                            field: "accountid".to_string(),
                            target: None,
                            optional: false,
                        },
                    ]
                })),
            }
        );
    }

    #[test]
    fn parse_entity_ref_with_variable_navigation() {
        let result = parse_path("/contact($found.contactid)").unwrap();
        assert_eq!(
            result,
            PathExpr::EntityRef {
                entity: "contact".to_string(),
                inner: Box::new(PathExpr::VariableNavigation {
                    name: "found".to_string(),
                    target: None,
                    path: FieldPath {
                        segments: vec![FieldSegment {
                            field: "contactid".to_string(),
                            target: None,
                            optional: false,
                        }]
                    },
                }),
            }
        );
    }

    #[test]
    fn parse_entity_ref_with_system_var() {
        let result = parse_path("/contact(#value)").unwrap();
        assert_eq!(
            result,
            PathExpr::EntityRef {
                entity: "contact".to_string(),
                inner: Box::new(PathExpr::SystemVar(SystemVar::Value)),
            }
        );
    }

    #[test]
    fn parse_entity_ref_empty_entity_error() {
        let result = parse_path("/(foo)");
        assert_eq!(result, Err(ParseError::EmptyEntityName));
    }

    #[test]
    fn parse_entity_ref_missing_paren_error() {
        let result = parse_path("/contact");
        assert_eq!(result, Err(ParseError::MissingOpenParen));
    }

    #[test]
    fn parse_entity_ref_unclosed_paren_error() {
        let result = parse_path("/contact($var");
        assert_eq!(result, Err(ParseError::UnclosedParen));
    }

    #[test]
    fn parse_entity_ref_empty_inner_error() {
        let result = parse_path("/contact()");
        assert_eq!(result, Err(ParseError::EmptyInnerPath));
    }
}
