//! Tests for page! macro parsing.
//!
//! These tests verify the parsing phase works correctly.

use rafter_derive::page;

// Basic element parsing
#[test]
fn test_simple_element() {
    let _: tuidom::Element = page! {
        column {}
    };
}

#[test]
fn test_element_with_layout_attrs() {
    let _: tuidom::Element = page! {
        column (padding: 1, gap: 2) {}
    };
}

#[test]
fn test_element_with_style_attrs() {
    let _: tuidom::Element = page! {
        column style (bg: primary, bold: true) {}
    };
}

#[test]
fn test_element_with_layout_and_style() {
    let _: tuidom::Element = page! {
        column (padding: 1) style (bg: primary) {}
    };
}

#[test]
fn test_element_with_children() {
    let _: tuidom::Element = page! {
        column {
            row {}
            row {}
        }
    };
}

#[test]
fn test_element_with_attrs_and_children() {
    let _: tuidom::Element = page! {
        column (gap: 2) style (bg: surface) {
            row (padding: 1) {}
        }
    };
}

// Handler parsing tests - commented out until widgets are implemented (Chunk 5)
// These tests verify parsing works but generation requires widget types
// #[test]
// fn test_element_with_handler() {
//     let _: tuidom::Element = page! {
//         button (label: "Click") on_click: handle_click() {}
//     };
// }
// #[test]
// fn test_element_with_handler_args() {
//     let _: tuidom::Element = page! {
//         button (label: "Delete") on_click: delete_item(item_id, cx) {}
//     };
// }
// #[test]
// fn test_element_with_multiple_handlers() {
//     let _: tuidom::Element = page! {
//         input (value: "test") on_change: handle_change(cx) on_submit: handle_submit(gx) {}
//     };
// }

// For loop parsing
#[test]
fn test_for_loop() {
    let _items = vec![1, 2, 3];
    let _: tuidom::Element = page! {
        for _item in _items {
            row {}
        }
    };
}

// If/else parsing
#[test]
fn test_if_statement() {
    let _show = true;
    let _: tuidom::Element = page! {
        if _show {
            column {}
        }
    };
}

#[test]
fn test_if_else() {
    let _show = false;
    let _: tuidom::Element = page! {
        if _show {
            column {}
        } else {
            row {}
        }
    };
}

#[test]
fn test_if_else_if() {
    let _value = 1;
    let _: tuidom::Element = page! {
        if _value == 0 {
            column {}
        } else if _value == 1 {
            row {}
        } else {
            column {}
        }
    };
}

// Match parsing
#[test]
fn test_match() {
    let _opt: Option<i32> = Some(1);
    let _: tuidom::Element = page! {
        match _opt {
            Some(_) => column {},
            None => row {},
        }
    };
}

// Braced expression
#[test]
fn test_braced_expr() {
    let _elem = tuidom::Element::col();
    let _: tuidom::Element = page! {
        { _elem }
    };
}

// Attribute value types
#[test]
fn test_attr_ident_value() {
    let _: tuidom::Element = page! {
        column (align: center) {}
    };
}

#[test]
fn test_attr_lit_value() {
    let _: tuidom::Element = page! {
        column (padding: 5) {}
    };
}

#[test]
fn test_attr_string_value() {
    let _: tuidom::Element = page! {
        text (content: "hello") {}
    };
}

#[test]
fn test_attr_expr_value() {
    let _padding = 3;
    let _: tuidom::Element = page! {
        column (padding: {_padding}) {}
    };
}

// Nested structure with new syntax
#[test]
fn test_complex_nested() {
    let _items = vec![1, 2];
    let _show_header = true;
    let _: tuidom::Element = page! {
        column (padding: 1, gap: 2) {
            if _show_header {
                row style (bg: primary) {
                    text (content: "Header") {}
                }
            }
            for _item in _items {
                row {
                    column {}
                }
            }
        }
    };
}

// Style-only elements
#[test]
fn test_style_only_element() {
    let _: tuidom::Element = page! {
        row style (bg: surface) {}
    };
}

// Layout-only elements
#[test]
fn test_layout_only_element() {
    let _: tuidom::Element = page! {
        column (padding: 2, gap: 1) {}
    };
}

// Transition parsing
#[test]
fn test_transition_single() {
    let _: tuidom::Element = page! {
        column transition (bg: 200) {}
    };
}

#[test]
fn test_transition_with_easing() {
    let _: tuidom::Element = page! {
        column transition (bg: 200 ease_out) {}
    };
}

#[test]
fn test_transition_multiple() {
    let _: tuidom::Element = page! {
        column transition (bg: 200 ease_out, fg: 100 linear) {}
    };
}

#[test]
fn test_transition_all() {
    let _: tuidom::Element = page! {
        column transition (all: 300 ease_in_out) {}
    };
}

#[test]
fn test_layout_style_transition() {
    let _: tuidom::Element = page! {
        column (padding: 1) style (bg: primary) transition (bg: 200) {}
    };
}

#[test]
fn test_transition_one_second() {
    let _: tuidom::Element = page! {
        column transition (bg: 1000) {}
    };
}
