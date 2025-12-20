//! Primitive node rendering functions (containers, text, button).
//!
//! These are built-in node types without component state.
//! Component-specific rendering lives with each component in `src/components/*/render.rs`.

mod button;
mod container;
mod text;

pub use button::render_button;
pub use container::{render_container, render_stack};
pub use text::render_text;
