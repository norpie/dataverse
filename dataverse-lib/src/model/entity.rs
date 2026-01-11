//! Entity enum (Logical/Set)

/// Represents an entity reference that can be either a logical name or an entity set name.
///
/// Dataverse Web API URLs use entity set names (e.g., `/api/data/v9.2/accounts`),
/// but users typically think in logical names (e.g., "account"). This enum lets
/// users specify either form.
///
/// # Examples
///
/// ```
/// use dataverse_lib::model::Entity;
///
/// // Using logical name - will be resolved via metadata
/// let entity = Entity::logical("account");
///
/// // Using entity set name directly - no resolution needed
/// let entity = Entity::set("accounts");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Entity {
    /// Logical name (e.g., "account") - resolved to entity set name via metadata
    Logical(String),
    /// Entity set name (e.g., "accounts") - used directly in API calls
    Set(String),
}

impl Entity {
    /// Creates an entity reference from a logical name.
    ///
    /// The logical name will be resolved to an entity set name via metadata
    /// when making API calls.
    pub fn logical(name: impl Into<String>) -> Self {
        Self::Logical(name.into())
    }

    /// Creates an entity reference from an entity set name.
    ///
    /// The entity set name is used directly in API calls without resolution.
    pub fn set(name: impl Into<String>) -> Self {
        Self::Set(name.into())
    }

    /// Returns the inner name, regardless of variant.
    pub fn name(&self) -> &str {
        match self {
            Self::Logical(name) | Self::Set(name) => name,
        }
    }

    /// Returns `true` if this is a logical name that needs resolution.
    pub fn needs_resolution(&self) -> bool {
        matches!(self, Self::Logical(_))
    }

    /// Returns the entity set name for use in API URLs.
    ///
    /// # Panics
    ///
    /// Panics if this is a `Logical` variant. Use `Entity::Set` directly
    /// or resolve the logical name via metadata first.
    pub fn set_name(&self) -> &str {
        match self {
            Self::Set(name) => name,
            Self::Logical(name) => {
                panic!(
                    "Entity::Logical('{}') requires metadata resolution. Use Entity::Set or resolve via metadata.",
                    name
                )
            }
        }
    }
}

impl std::fmt::Display for Entity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Logical(name) => write!(f, "{} (logical)", name),
            Self::Set(name) => write!(f, "{} (set)", name),
        }
    }
}
