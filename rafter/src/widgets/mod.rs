//! Built-in widgets for rafter.
//!
//! Widgets are UI components that produce tuidom Elements. All widgets follow
//! the same builder pattern with a `build(registry, handlers)` method.
//!
//! # Widget Categories
//!
//! - **Stateless**: `Text`, `Button`, `Card` - no internal state
//! - **Stateful**: `Checkbox`, `Select`, `RadioGroup`, `Input` - use typestate
//!   pattern to enforce `state()` is called before `build()`
//!
//! # Usage
//!
//! Import widgets and use them in the `page!` macro:
//!
//! ```ignore
//! use rafter::widgets::{Text, Button, Checkbox};
//!
//! page! {
//!     text (content: "Hello", id: "greeting")
//!     button (label: "Click me", id: "btn")
//!         on_activate: handle_click()
//!     checkbox (state: self.agree, id: "agree", label: "I agree")
//!         on_change: agreement_changed()
//! }
//! ```
//!
//! The macro converts widget names from snake_case to PascalCase and generates
//! builder method calls for each prop.

pub mod button;
pub mod card;
pub mod checkbox;
pub mod input;
pub mod radio;
pub mod select;
pub mod text;

pub use button::Button;
pub use card::Card;
pub use checkbox::{Checkbox, CheckboxVariant};
pub use input::Input;
pub use radio::{RadioGroup, RadioState};
pub use select::{Select, SelectState};
pub use text::Text;
