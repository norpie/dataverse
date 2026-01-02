//! Validator builder for fluent validation API.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use super::result::{FieldError, ValidationResult};
use super::validatable::Validatable;

/// Type alias for boxed futures used in async validation.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Type alias for sync validation rule closures.
type SyncRule<V> = Box<dyn Fn(&V) -> Result<(), String> + Send + Sync>;

/// Type alias for async validation rule closures.
type AsyncRule<V> = Box<dyn Fn(V) -> BoxFuture<'static, Result<(), String>> + Send + Sync>;

/// Internal representation of a field being validated.
struct FieldEntry {
    name: String,
    widget_id: String,
    set_error: Box<dyn Fn(Option<String>) + Send + Sync>,
    validate_sync: Box<dyn Fn() -> Vec<String> + Send + Sync>,
    validate_async: Box<dyn Fn() -> BoxFuture<'static, Vec<String>> + Send + Sync>,
}

/// Builder for validating multiple form fields.
///
/// # Example
///
/// ```ignore
/// let result = Validator::new()
///     .field(&self.name, "name")
///         .required("Name is required")
///     .field(&self.email, "email")
///         .required("Email is required")
///         .email("Invalid email format")
///     .validate();
///
/// if result.is_valid() {
///     // Submit form
/// }
/// ```
pub struct Validator {
    fields: Vec<FieldEntry>,
}

impl Validator {
    /// Create a new validator.
    pub fn new() -> Self {
        Self { fields: Vec::new() }
    }

    /// Add a field to validate.
    pub fn field<W: Validatable + Clone + 'static>(
        self,
        widget: &W,
        name: impl Into<String>,
    ) -> FieldBuilder<W>
    where
        W::Value: Clone + Send + 'static,
    {
        FieldBuilder {
            validator: self,
            widget: widget.clone(),
            name: name.into(),
            sync_rules: Vec::new(),
            async_rules: Vec::new(),
        }
    }

    /// Run all synchronous validations.
    pub fn validate(self) -> ValidationResult {
        let mut errors = Vec::new();

        for field in &self.fields {
            let field_errors = (field.validate_sync)();
            if let Some(first_error) = field_errors.first() {
                (field.set_error)(Some(first_error.clone()));
                errors.push(FieldError {
                    field_name: field.name.clone(),
                    widget_id: field.widget_id.clone(),
                    message: first_error.clone(),
                });
            } else {
                (field.set_error)(None);
            }
        }

        if errors.is_empty() {
            ValidationResult::Valid
        } else {
            ValidationResult::Invalid(errors)
        }
    }

    /// Run all validations including async rules.
    pub async fn validate_async(self) -> ValidationResult {
        let mut errors = Vec::new();

        for field in &self.fields {
            let field_errors = (field.validate_async)().await;
            if let Some(first_error) = field_errors.first() {
                (field.set_error)(Some(first_error.clone()));
                errors.push(FieldError {
                    field_name: field.name.clone(),
                    widget_id: field.widget_id.clone(),
                    message: first_error.clone(),
                });
            } else {
                (field.set_error)(None);
            }
        }

        if errors.is_empty() {
            ValidationResult::Valid
        } else {
            ValidationResult::Invalid(errors)
        }
    }
}

impl Default for Validator {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for adding validation rules to a single field.
pub struct FieldBuilder<W: Validatable> {
    validator: Validator,
    widget: W,
    name: String,
    sync_rules: Vec<SyncRule<W::Value>>,
    async_rules: Vec<AsyncRule<W::Value>>,
}

impl<W: Validatable + Clone + 'static> FieldBuilder<W>
where
    W::Value: Clone + Send + 'static,
{
    /// Add a custom synchronous validation rule.
    pub fn rule<F>(mut self, f: F, msg: impl Into<String>) -> Self
    where
        F: Fn(&W::Value) -> bool + Send + Sync + 'static,
    {
        let msg = msg.into();
        self.sync_rules
            .push(Box::new(move |v| if f(v) { Ok(()) } else { Err(msg.clone()) }));
        self
    }

    /// Add a custom asynchronous validation rule.
    pub fn rule_async<F, Fut>(mut self, f: F, msg: impl Into<String>) -> Self
    where
        F: Fn(W::Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = bool> + Send + 'static,
    {
        let msg = msg.into();
        self.async_rules.push(Box::new(move |v| {
            let fut = f(v);
            let msg = msg.clone();
            Box::pin(async move { if fut.await { Ok(()) } else { Err(msg) } })
        }));
        self
    }

    /// Continue to the next field.
    pub fn field<W2: Validatable + Clone + 'static>(
        self,
        widget: &W2,
        name: impl Into<String>,
    ) -> FieldBuilder<W2>
    where
        W2::Value: Clone + Send + 'static,
    {
        let validator = self.finalize();
        validator.field(widget, name)
    }

    /// Finalize and run all synchronous validations.
    pub fn validate(self) -> ValidationResult {
        self.finalize().validate()
    }

    /// Finalize and run all validations including async rules.
    pub async fn validate_async(self) -> ValidationResult {
        self.finalize().validate_async().await
    }

    /// Finalize this field and return the validator.
    fn finalize(self) -> Validator {
        let widget_id = self.widget.widget_id();
        let name = self.name;

        let widget_for_sync = self.widget.clone();
        let widget_for_async = self.widget.clone();
        let widget_for_error = self.widget;

        let sync_rules = Arc::new(self.sync_rules);
        let async_rules = Arc::new(self.async_rules);

        let sync_rules_for_sync = Arc::clone(&sync_rules);

        let validate_sync: Box<dyn Fn() -> Vec<String> + Send + Sync> = Box::new(move || {
            let value = widget_for_sync.validation_value();
            let mut errors = Vec::new();
            for rule in sync_rules_for_sync.iter() {
                if let Err(msg) = rule(&value) {
                    errors.push(msg);
                }
            }
            errors
        });

        let sync_rules_for_async = Arc::clone(&sync_rules);
        let async_rules_for_async = Arc::clone(&async_rules);

        let validate_async: Box<dyn Fn() -> BoxFuture<'static, Vec<String>> + Send + Sync> =
            Box::new(move || {
                let value = widget_for_async.validation_value();
                let sync_rules = Arc::clone(&sync_rules_for_async);
                let async_rules = Arc::clone(&async_rules_for_async);

                Box::pin(async move {
                    let mut errors = Vec::new();

                    for rule in sync_rules.iter() {
                        if let Err(msg) = rule(&value) {
                            errors.push(msg);
                        }
                    }

                    for rule in async_rules.iter() {
                        if let Err(msg) = rule(value.clone()).await {
                            errors.push(msg);
                        }
                    }

                    errors
                })
            });

        let set_error: Box<dyn Fn(Option<String>) + Send + Sync> = Box::new(move |msg| {
            if let Some(msg) = msg {
                widget_for_error.set_error(msg);
            } else {
                widget_for_error.clear_error();
            }
        });

        let mut validator = self.validator;
        validator.fields.push(FieldEntry {
            name,
            widget_id,
            set_error,
            validate_sync,
            validate_async,
        });

        validator
    }
}

// Built-in rules for String values
impl<W: Validatable<Value = String> + Clone + 'static> FieldBuilder<W> {
    /// Require the field to be non-empty.
    pub fn required(self, msg: impl Into<String>) -> Self {
        let msg = msg.into();
        self.rule(|v| !v.trim().is_empty(), msg)
    }

    /// Require minimum length (in characters).
    pub fn min_length(self, min: usize, msg: impl Into<String>) -> Self {
        let msg = msg.into();
        self.rule(move |v| v.chars().count() >= min, msg)
    }

    /// Require maximum length (in characters).
    pub fn max_length(self, max: usize, msg: impl Into<String>) -> Self {
        let msg = msg.into();
        self.rule(move |v| v.chars().count() <= max, msg)
    }

    /// Require the value to match a regex pattern.
    pub fn pattern(self, pattern: &str, msg: impl Into<String>) -> Self {
        let msg = msg.into();
        let re = regex::Regex::new(pattern).expect("Invalid regex pattern");
        self.rule(move |v| re.is_match(v), msg)
    }

    /// Require a valid email address.
    pub fn email(self, msg: impl Into<String>) -> Self {
        let msg = msg.into();
        self.rule(
            |v| {
                if v.is_empty() {
                    true // Empty is valid; use required() for non-empty
                } else {
                    email_address::EmailAddress::is_valid(v)
                }
            },
            msg,
        )
    }

    /// Require the value to equal another value.
    pub fn equals(self, other: String, msg: impl Into<String>) -> Self {
        let msg = msg.into();
        self.rule(move |v| v == &other, msg)
    }

    /// Require the value to contain a substring.
    pub fn contains(self, substr: impl Into<String>, msg: impl Into<String>) -> Self {
        let msg = msg.into();
        let substr = substr.into();
        self.rule(move |v| v.contains(&substr), msg)
    }
}

// Built-in rules for bool values
impl<W: Validatable<Value = bool> + Clone + 'static> FieldBuilder<W> {
    /// Require the checkbox to be checked.
    pub fn checked(self, msg: impl Into<String>) -> Self {
        let msg = msg.into();
        self.rule(|&v| v, msg)
    }

    /// Require the checkbox to be unchecked.
    pub fn unchecked(self, msg: impl Into<String>) -> Self {
        let msg = msg.into();
        self.rule(|&v| !v, msg)
    }
}

// Built-in rules for Option<usize> values
impl<W: Validatable<Value = Option<usize>> + Clone + 'static> FieldBuilder<W> {
    /// Require that an option is selected.
    pub fn selected(self, msg: impl Into<String>) -> Self {
        let msg = msg.into();
        self.rule(|v| v.is_some(), msg)
    }

    /// Require a specific option to be selected.
    pub fn selected_index(self, index: usize, msg: impl Into<String>) -> Self {
        let msg = msg.into();
        self.rule(move |v| *v == Some(index), msg)
    }
}
