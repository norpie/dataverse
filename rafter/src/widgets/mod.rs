//! Built-in widgets for rafter.
//!
//! Widgets are interactive UI components that can respond to user input.
//! Each widget is a builder that produces a tuidom Element.
//!
//! Widgets store `Option<HandlerId>` for event handlers. When an event occurs,
//! the runtime extracts the HandlerId and dispatches through the app/modal's
//! `dispatch` method.
//!
//! # Handler ID Pattern
//!
//! Instead of storing closures, widgets store handler IDs:
//!
//! ```ignore
//! // page! macro emits:
//! button::new()
//!     .label("Click")
//!     .on_click_id(HandlerId::new("my_handler"))
//!     .element()
//!
//! // The element stores the handler ID as data
//! // When clicked, runtime calls app.dispatch("my_handler", cx, gx)
//! ```
//!
//! This pattern:
//! - Keeps widgets simple (no complex closure types)
//! - Allows impl macros to generate dispatch with full context knowledge
//! - Works consistently for apps and modals

pub mod button;

pub use button::Button;
