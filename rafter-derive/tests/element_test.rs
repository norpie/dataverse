//! Tests for element! macro
//!
//! Tests that element! works without self context and rejects handlers.

use rafter::element;
use rafter::widgets::Text;
use tuidom::Element;

#[test]
fn test_element_basic() {
    let _: Element = element! {
        col {
            text (content: "Hello")
        }
    };
}

#[test]
fn test_element_with_styling() {
    let _: Element = element! {
        row (gap: 2, padding: 1) style (bg: primary) {
            text (content: "Styled")
        }
    };
}

#[test]
fn test_element_nested() {
    let _: Element = element! {
        col (gap: 1) {
            row {
                text (content: "Left")
                text (content: "Right")
            }
            text (content: "Bottom")
        }
    };
}

// Test that should NOT compile (handler syntax not allowed)
// Uncomment to verify error message:
// #[test]
// fn test_element_rejects_handlers() {
//     let _: Element = element! {
//         button on_click: my_handler()
//     };
// }
