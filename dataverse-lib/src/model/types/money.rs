//! Money type

use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Serialize;

/// A monetary value represented as a decimal.
///
/// This is a simple wrapper around `Decimal` for representing currency values
/// in Dataverse. Currency information (which currency) is stored separately
/// in Dataverse as a lookup field.
///
/// # Example
///
/// ```
/// use dataverse_lib::model::types::Money;
/// use rust_decimal::Decimal;
///
/// let price = Money::new(Decimal::new(1999, 2));  // 19.99
/// let price = Money::from(Decimal::new(1999, 2));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Money(pub Decimal);

impl Money {
    /// Creates a new Money value.
    pub fn new(value: Decimal) -> Self {
        Self(value)
    }

    /// Returns the inner decimal value.
    pub fn value(&self) -> Decimal {
        self.0
    }

    /// Creates a Money value from an integer (whole units).
    pub fn from_int(value: i64) -> Self {
        Self(Decimal::new(value, 0))
    }
}

impl From<Decimal> for Money {
    fn from(value: Decimal) -> Self {
        Self(value)
    }
}

impl From<Money> for Decimal {
    fn from(money: Money) -> Self {
        money.0
    }
}

impl std::fmt::Display for Money {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
