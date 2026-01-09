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

pub mod autocomplete;
pub mod button;
pub mod card;
pub mod checkbox;
pub mod collapsible;
pub mod input;
pub mod list;
pub mod radio;
pub mod scroll;
pub mod select;
pub mod selection;
pub mod text;

pub use autocomplete::{Autocomplete, AutocompleteState};
pub use button::Button;
pub use card::Card;
pub use checkbox::{Checkbox, CheckboxVariant};
pub use collapsible::Collapsible;
pub use input::Input;
pub use list::{List, ListItem, ListState};
pub use radio::{RadioGroup, RadioState};
pub use scroll::{Orientation, Scrollbar, ScrollbarStyle, ScrollRequest, ScrollState};
pub use select::{Select, SelectState};
pub use selection::{Selection, SelectionMode};
pub use text::Text;
