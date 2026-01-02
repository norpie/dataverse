//! Form validation system.
//!
//! # Example
//!
//! ```ignore
//! use rafter::validation::Validator;
//!
//! let result = Validator::new()
//!     .field(&self.username, "username")
//!         .required("Username is required")
//!         .min_length(3, "Username must be at least 3 characters")
//!     .field(&self.email, "email")
//!         .required("Email is required")
//!         .email("Please enter a valid email")
//!     .field(&self.accept_terms, "terms")
//!         .checked("You must accept the terms")
//!     .validate();
//!
//! if result.is_valid() {
//!     // Proceed with form submission
//! }
//! ```

mod error_display;
mod result;
mod validatable;
mod validator;

pub use error_display::ErrorDisplay;
pub use result::{FieldError, ValidationResult};
pub use validatable::Validatable;
pub use validator::{FieldBuilder, Validator};
