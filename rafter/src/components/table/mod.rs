//! Table component - a virtualized table with columns, row selection, and sorting.
//!
//! The Table component provides:
//! - Column-based layout with sticky headers
//! - Bi-directional virtualization (only visible rows and columns rendered)
//! - Row selection (single or multi-select)
//! - Sortable columns (app-controlled)
//! - Horizontal and vertical scrolling
//!
//! # Example
//!
//! ```ignore
//! use rafter::prelude::*;
//!
//! #[derive(Clone, Debug)]
//! struct User {
//!     id: String,
//!     name: String,
//!     email: String,
//! }
//!
//! impl TableRow for User {
//!     fn id(&self) -> String { self.id.clone() }
//!     fn column_count(&self) -> usize { 2 }
//!     
//!     fn render_cell(&self, col_idx: usize, focused: bool, selected: bool) -> Option<Node> {
//!         match col_idx {
//!             0 => Some(view! { text { self.name.clone() } }),
//!             1 => Some(view! { text { self.email.clone() } }),
//!             _ => None,
//!         }
//!     }
//! }
//!
//! let columns = vec![
//!     Column::new("Name", 30).sortable(),
//!     Column::new("Email", 40),
//! ];
//! let table = Table::with_rows(columns, users);
//! ```

mod any_table;
mod events;
mod item;
mod state;

pub use any_table::AnyTable;
pub use item::{Alignment, Column, TableRow};
pub use state::{Table, TableId};
