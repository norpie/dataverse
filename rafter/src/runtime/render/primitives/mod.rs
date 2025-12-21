//! Primitive node rendering functions (containers, text, button).
//!
//! These are built-in node types without widget state.
//! Widget-specific rendering lives with each widget in `src/widgets/*/render.rs`.

mod button;
mod container;
mod text;

pub use button::render_button;
pub use container::{render_container, render_stack};
pub use text::render_text;
