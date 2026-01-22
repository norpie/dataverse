//! Loading overlay widget for blocking operations.
//!
//! Displays a centered spinner with message over a dimmed backdrop.
//! Used for operations where the user should wait (e.g., loading metadata).

use tuidom::{Align, Backdrop, Color, Edges, Element, Justify, Position, Size, Style};

use super::Spinner;

/// Create a loading overlay element.
///
/// The overlay is absolutely positioned, fills the parent, and blocks interaction
/// with elements behind it via interaction_scope.
pub fn loading_overlay(id: &str, message: &str) -> Element {
    let spinner_id = format!("{}-spinner", id);

    Element::col()
        .id(id)
        .position(Position::Absolute)
        .left(0)
        .top(0)
        .width(Size::Fill)
        .height(Size::Fill)
        .backdrop(Backdrop::Dim(0.5))
        .justify(Justify::Center)
        .align(Align::Center)
        .interaction_scope(true)
        .child(
            Element::col()
                .align(Align::Center)
                .margin(Edges::symmetric(2, 1))
                .style(Style::new().background(Color::var("surface")))
                .child(Element::text(message).style(Style::new().foreground(Color::var("muted"))))
                .child(Spinner::default().id(&spinner_id).build_standalone()),
        )
}
