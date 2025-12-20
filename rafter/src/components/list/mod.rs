//! List component - a virtualized, selectable list with item rendering.
//!
//! The List component provides:
//! - Virtualization (only visible items are rendered)
//! - Single and multi-selection with Ctrl+click and Shift+range
//! - Cursor navigation with keyboard
//! - Activation (Enter/click) and selection (Space/Ctrl+click) distinction
//!
//! # Example
//!
//! ```ignore
//! use rafter::prelude::*;
//!
//! #[derive(Clone)]
//! struct FileItem {
//!     name: String,
//!     size: u64,
//! }
//!
//! impl ListItem for FileItem {
//!     fn render(&self, focused: bool, selected: bool) -> Node {
//!         let bg = if focused { Some("surface") } else { None };
//!         let prefix = if selected { "[x] " } else { "[ ] " };
//!         view! {
//!             row (bg: bg) {
//!                 text { format!("{}{}", prefix, self.name) }
//!             }
//!         }
//!     }
//! }
//!
//! #[app]
//! struct MyApp {
//!     files: List<FileItem>,
//! }
//!
//! #[app_impl]
//! impl MyApp {
//!     fn view(&self) -> Node {
//!         view! {
//!             list(bind: self.files)
//!         }
//!     }
//!
//!     #[handler]
//!     async fn on_activate(&self, cx: &AppContext, event: ActivateEvent) {
//!         let file = self.files.get(event.index);
//!         // Open file...
//!     }
//! }
//! ```

pub mod events;
pub mod render;
mod state;

pub use state::{AnyList, List, ListId, ListItem, Selection, SelectionMode};
