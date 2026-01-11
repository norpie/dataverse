//! Option set value types

use serde::Deserialize;
use serde::Serialize;

/// A single option set (picklist) value.
///
/// Represents a choice field in Dataverse. The numeric value is stored in the
/// database, while the label is the human-readable display text.
///
/// # Example
///
/// ```
/// use dataverse_lib::model::types::OptionSetValue;
///
/// // Just the value (for writes)
/// let status = OptionSetValue::new(1);
///
/// // With label (typically from reads)
/// let status = OptionSetValue::with_label(1, "Active");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OptionSetValue {
    /// The numeric value of the option.
    pub value: i32,
    /// The display label, if available (from formatted values).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl OptionSetValue {
    /// Creates a new option set value.
    pub fn new(value: i32) -> Self {
        Self { value, label: None }
    }

    /// Creates a new option set value with a label.
    pub fn with_label(value: i32, label: impl Into<String>) -> Self {
        Self {
            value,
            label: Some(label.into()),
        }
    }
}

impl From<i32> for OptionSetValue {
    fn from(value: i32) -> Self {
        Self::new(value)
    }
}

impl From<OptionSetValue> for i32 {
    fn from(opt: OptionSetValue) -> Self {
        opt.value
    }
}

/// A multi-select option set value.
///
/// Represents a multi-select choice field in Dataverse. Multiple values can be
/// selected simultaneously.
///
/// # Example
///
/// ```
/// use dataverse_lib::model::types::MultiSelectOptionSetValue;
///
/// // Just the values (for writes)
/// let categories = MultiSelectOptionSetValue::new(vec![1, 3, 5]);
///
/// // With labels (typically from reads)
/// let categories = MultiSelectOptionSetValue::with_labels(vec![
///     (1, "Category A"),
///     (3, "Category C"),
///     (5, "Category E"),
/// ]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MultiSelectOptionSetValue {
    /// The numeric values of the selected options.
    pub values: Vec<i32>,
    /// The display labels, if available (from formatted values).
    /// Corresponds positionally to `values`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
}

impl MultiSelectOptionSetValue {
    /// Creates a new multi-select option set value.
    pub fn new(values: Vec<i32>) -> Self {
        Self {
            values,
            labels: None,
        }
    }

    /// Creates a new multi-select option set value with labels.
    pub fn with_labels(values_and_labels: Vec<(i32, impl Into<String>)>) -> Self {
        let (values, labels): (Vec<_>, Vec<_>) = values_and_labels
            .into_iter()
            .map(|(v, l)| (v, l.into()))
            .unzip();
        Self {
            values,
            labels: Some(labels),
        }
    }

    /// Returns `true` if no options are selected.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Returns the number of selected options.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns `true` if the given value is selected.
    pub fn contains(&self, value: i32) -> bool {
        self.values.contains(&value)
    }
}

impl From<Vec<i32>> for MultiSelectOptionSetValue {
    fn from(values: Vec<i32>) -> Self {
        Self::new(values)
    }
}

impl From<MultiSelectOptionSetValue> for Vec<i32> {
    fn from(opt: MultiSelectOptionSetValue) -> Self {
        opt.values
    }
}
