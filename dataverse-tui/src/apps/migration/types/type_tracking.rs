//! Design-time type tracking for transform chains.

use std::collections::HashMap;

use dataverse_lib::model::metadata::AttributeType;
use dataverse_lib::model::FieldType;
use dataverse_lib::model::ValueType;

use super::domain::Transform;
use super::transform::TransformData;

/// Type signature for a transform.
#[derive(Debug, Clone)]
pub struct TransformSignature {
    /// Expected input type. None = no input (chain starter).
    pub input: Option<ValueType>,
    /// Output type. None = passthrough (same as input) or dynamic.
    pub output: Option<ValueType>,
}

impl TransformData {
    /// Get the type signature for this transform.
    pub fn signature(&self) -> TransformSignature {
        match self {
            // Chain starters (no input required)
            TransformData::Copy { .. } => TransformSignature {
                input: None,
                output: None, // Dynamic - resolved from path
            },
            TransformData::Constant { value } => TransformSignature {
                input: None,
                output: Some(ValueType::from(value)),
            },
            TransformData::Guid => TransformSignature {
                input: None,
                output: Some(ValueType::simple(AttributeType::Uniqueidentifier)),
            },
            TransformData::Find { .. } => TransformSignature {
                input: None,
                output: Some(ValueType::lookup(AttributeType::Lookup, vec![])),
            },

            // String operations (expect String)
            TransformData::StringOps { .. } => TransformSignature {
                input: Some(ValueType::simple(AttributeType::String)),
                output: None, // Passthrough
            },
            TransformData::Format { .. } => TransformSignature {
                input: Some(ValueType::Any),
                output: Some(ValueType::simple(AttributeType::String)),
            },
            TransformData::Replace { .. } => TransformSignature {
                input: Some(ValueType::simple(AttributeType::String)),
                output: None, // Passthrough
            },

            // Parse transforms (String -> specific type)
            TransformData::ParseInt => TransformSignature {
                input: Some(ValueType::simple(AttributeType::String)),
                output: Some(ValueType::simple(AttributeType::Integer)),
            },
            TransformData::ParseDecimal => TransformSignature {
                input: Some(ValueType::simple(AttributeType::String)),
                output: Some(ValueType::simple(AttributeType::Decimal)),
            },
            TransformData::ParseDate { .. } => TransformSignature {
                input: Some(ValueType::simple(AttributeType::String)),
                output: Some(ValueType::simple(AttributeType::DateTime)),
            },

            // Convert (Any -> target type)
            TransformData::Convert { target_type } => {
                let output = match target_type.as_str() {
                    "int" | "integer" => AttributeType::Integer,
                    "decimal" | "number" => AttributeType::Decimal,
                    "string" | "text" => AttributeType::String,
                    "bool" | "boolean" => AttributeType::Boolean,
                    _ => AttributeType::String,
                };
                TransformSignature {
                    input: Some(ValueType::Any),
                    output: Some(ValueType::simple(output)),
                }
            }

            // ValueMap accepts any option set, outputs the target's type
            TransformData::ValueMap { target, .. } => TransformSignature {
                input: Some(ValueType::AnyOptionSet),
                output: Some(ValueType::option_set(target.kind, target.name.clone())),
            },

            // Math (numeric -> passthrough)
            TransformData::Math { .. } => TransformSignature {
                input: Some(ValueType::Union(vec![
                    FieldType::Simple(AttributeType::Integer),
                    FieldType::Simple(AttributeType::Decimal),
                ])),
                output: None, // Passthrough
            },

            // Control flow
            TransformData::Guard { .. } => TransformSignature {
                input: Some(ValueType::Any),
                output: None, // Passthrough
            },
            TransformData::Coalesce => TransformSignature {
                input: None,
                output: None, // Dynamic - union of branch outputs
            },
            TransformData::Match => TransformSignature {
                input: Some(ValueType::Any),
                output: None, // Dynamic - union of branch outputs
            },
        }
    }
}

// =============================================================================
// Type Warnings
// =============================================================================

/// A type mismatch warning for a transform.
#[derive(Debug, Clone)]
pub struct TypeWarning {
    /// ID of the transform with the warning.
    pub transform_id: i64,
    /// Expected input type.
    pub expected: ValueType,
    /// Actual input type received.
    pub actual: ValueType,
}

/// A type mismatch warning when a chain's output type doesn't match the expected type.
///
/// Used for both field mappings (chain output vs target field type) and
/// variables (chain output vs declared type).
#[derive(Debug, Clone)]
pub struct ChainOutputWarning {
    /// The chain output type.
    pub chain_output: ValueType,
    /// The expected target type (field type or declared type).
    pub target_type: ValueType,
}

// =============================================================================
// Chain Type Result
// =============================================================================

/// Result of type propagation through a transform chain.
#[derive(Debug, Clone, Default)]
pub struct ChainTypeResult {
    /// Output type of the chain.
    pub output_type: ValueType,
    /// Type at each transform (transform_id -> output type after that transform).
    pub transform_types: HashMap<i64, ValueType>,
    /// Input type at each transform (transform_id -> #value type going into that transform).
    pub transform_input_types: HashMap<i64, ValueType>,
    /// Type warnings for mismatches.
    pub warnings: Vec<TypeWarning>,
}

impl ChainTypeResult {
    /// Create a new empty result.
    pub fn new() -> Self {
        Self {
            output_type: ValueType::Null,
            transform_types: HashMap::new(),
            transform_input_types: HashMap::new(),
            warnings: Vec::new(),
        }
    }

    /// Check if there are any warnings.
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    /// Check if a specific transform has a warning.
    pub fn has_warning_for(&self, transform_id: i64) -> bool {
        self.warnings.iter().any(|w| w.transform_id == transform_id)
    }

    /// Get the warning for a specific transform, if any.
    pub fn warning_for(&self, transform_id: i64) -> Option<&TypeWarning> {
        self.warnings
            .iter()
            .find(|w| w.transform_id == transform_id)
    }
}

// =============================================================================
// Type Propagation
// =============================================================================

/// Context for resolving dynamic types (copy paths, variables).
pub struct TypeResolutionContext {
    /// Variable name -> resolved type.
    pub variables: HashMap<String, ValueType>,
}

impl TypeResolutionContext {
    /// Create a new empty context.
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    /// Add a variable type.
    pub fn with_variable(mut self, name: impl Into<String>, value_type: ValueType) -> Self {
        self.variables.insert(name.into(), value_type);
        self
    }
}

impl Default for TypeResolutionContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Propagate types through a transform chain.
///
/// This performs synchronous type propagation. Dynamic types (copy paths)
/// should be pre-resolved and passed via the `resolve_fn` callback.
pub fn propagate_chain_types<F>(transforms: &[Transform], resolve_fn: F) -> ChainTypeResult
where
    F: Fn(&TransformData, &ValueType) -> Option<ValueType>,
{
    let mut result = ChainTypeResult::new();
    let mut current_type = ValueType::Null;

    for transform in transforms {
        let sig = transform.data.signature();
        log::debug!(
            "type_tracking: transform {} ({:?}) sig input={:?} output={:?}, current_type={:?}",
            transform.id,
            std::mem::discriminant(&transform.data),
            sig.input,
            sig.output,
            current_type,
        );

        // Store the input type (#value) for this transform
        result
            .transform_input_types
            .insert(transform.id, current_type.clone());

        // Check input compatibility
        if let Some(expected_input) = &sig.input {
            if !current_type.is_compatible_with(expected_input) {
                log::debug!(
                    "type_tracking: WARNING transform {} expected {:?} but got {:?}",
                    transform.id,
                    expected_input,
                    current_type,
                );
                result.warnings.push(TypeWarning {
                    transform_id: transform.id,
                    expected: expected_input.clone(),
                    actual: current_type.clone(),
                });
            }
        }

        // Compute output type
        current_type = match sig.output {
            Some(t) => {
                log::debug!(
                    "type_tracking: transform {} output from signature: {:?}",
                    transform.id,
                    t,
                );
                t
            }
            None => {
                // Dynamic or passthrough - try to resolve
                if let Some(resolved) = resolve_fn(&transform.data, &current_type) {
                    log::debug!(
                        "type_tracking: transform {} output resolved dynamically: {:?}",
                        transform.id,
                        resolved,
                    );
                    resolved
                } else {
                    log::debug!(
                        "type_tracking: transform {} output passthrough: {:?}",
                        transform.id,
                        current_type,
                    );
                    // Passthrough
                    current_type.clone()
                }
            }
        };

        result
            .transform_types
            .insert(transform.id, current_type.clone());
    }

    result.output_type = current_type.clone();
    log::debug!(
        "type_tracking: chain result output={:?}, warnings={}",
        current_type,
        result.warnings.len()
    );
    result
}

/// Simple propagation without dynamic resolution (uses passthrough for unknowns).
pub fn propagate_chain_types_simple(transforms: &[Transform]) -> ChainTypeResult {
    propagate_chain_types(transforms, |_, _| None)
}

// =============================================================================
// Branch Union
// =============================================================================

/// Resolve the output type from multiple branch outputs (for coalesce/match).
pub fn resolve_branch_union(branch_types: &[ValueType]) -> ValueType {
    if branch_types.is_empty() {
        return ValueType::Null;
    }

    // Collect all known field types
    let mut known_types: Vec<FieldType> = Vec::new();
    let mut has_null = false;

    for t in branch_types {
        match t {
            ValueType::Known(ft) => {
                if !known_types.contains(ft) {
                    known_types.push(ft.clone());
                }
            }
            ValueType::Null => has_null = true,
            ValueType::Union(types) => {
                for ft in types {
                    if !known_types.contains(ft) {
                        known_types.push(ft.clone());
                    }
                }
            }
            ValueType::Any | ValueType::AnyOptionSet => {
                // If any branch returns Any/AnyOptionSet, result is Any
                return ValueType::Any;
            }
        }
    }

    let result = match known_types.len() {
        0 => ValueType::Null,
        1 if !has_null => ValueType::Known(known_types.into_iter().next().unwrap()),
        _ => ValueType::Union(known_types),
    };
    log::debug!(
        "type_tracking: branch union from {} branches -> {:?}",
        branch_types.len(),
        result
    );
    result
}
