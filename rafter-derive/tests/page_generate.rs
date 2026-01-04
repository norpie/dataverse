//! Tests for page! macro code generation.
//!
//! These tests verify the generated code compiles and produces valid tuidom::Element values.
//! Since Element's internals are private, we mainly verify compilation succeeds.

use rafter_derive::page;
use tuidom::Element;

// Basic element generation
#[test]
fn test_generate_column() {
    let _elem: Element = page! {
        column {}
    };
}

#[test]
fn test_generate_col_alias() {
    let _elem: Element = page! {
        col {}
    };
}

#[test]
fn test_generate_row() {
    let _elem: Element = page! {
        row {}
    };
}

#[test]
fn test_generate_box() {
    let _elem: Element = page! {
        box_ {}
    };
}

#[test]
fn test_generate_text() {
    let _elem: Element = page! {
        text (content: "Hello") {}
    };
}

#[test]
fn test_generate_text_dynamic() {
    let msg = "Dynamic";
    let _elem: Element = page! {
        text (content: {msg}) {}
    };
}

// Layout attributes (in main parens)
#[test]
fn test_generate_with_gap() {
    let _elem: Element = page! {
        column (gap: 5) {}
    };
}

#[test]
fn test_generate_with_padding() {
    let _elem: Element = page! {
        column (padding: 2) {}
    };
}

#[test]
fn test_generate_with_width_fixed() {
    let _elem: Element = page! {
        column (width: 100) {}
    };
}

#[test]
fn test_generate_with_width_fill() {
    let _elem: Element = page! {
        column (width: fill) {}
    };
}

#[test]
fn test_generate_with_width_auto() {
    let _elem: Element = page! {
        column (width: auto) {}
    };
}

#[test]
fn test_generate_with_justify() {
    let _elem: Element = page! {
        column (justify: center) {}
    };
}

#[test]
fn test_generate_with_align() {
    let _elem: Element = page! {
        column (align: stretch) {}
    };
}

#[test]
fn test_generate_with_overflow() {
    let _elem: Element = page! {
        column (overflow: scroll) {}
    };
}

// Nested elements
#[test]
fn test_generate_nested() {
    let _elem: Element = page! {
        column {
            row {}
            row {}
        }
    };
}

#[test]
fn test_generate_deeply_nested() {
    let _elem: Element = page! {
        column {
            row {
                column {
                    text (content: "deep") {}
                }
            }
        }
    };
}

// Control flow
#[test]
fn test_generate_if_true() {
    let show = true;
    let _elem: Element = page! {
        if show {
            text (content: "visible") {}
        }
    };
}

#[test]
fn test_generate_if_false() {
    let show = false;
    let _elem: Element = page! {
        if show {
            text (content: "visible") {}
        }
    };
}

#[test]
fn test_generate_if_else() {
    let show = false;
    let _elem: Element = page! {
        if show {
            text (content: "yes") {}
        } else {
            text (content: "no") {}
        }
    };
}

#[test]
fn test_generate_if_else_if() {
    let value = 2;
    let _elem: Element = page! {
        if value == 0 {
            text (content: "zero") {}
        } else if value == 1 {
            text (content: "one") {}
        } else {
            text (content: "other") {}
        }
    };
}

#[test]
fn test_generate_for_loop() {
    let items = vec![1, 2, 3];
    let _elem: Element = page! {
        for _i in items {
            row {}
        }
    };
}

#[test]
fn test_generate_for_loop_with_value() {
    let items = vec!["a", "b", "c"];
    let _elem: Element = page! {
        for item in items {
            text (content: {item}) {}
        }
    };
}

#[test]
fn test_generate_match() {
    let opt: Option<i32> = Some(42);
    let _elem: Element = page! {
        match opt {
            Some(_) => text (content: "some") {},
            None => text (content: "none") {},
        }
    };
}

#[test]
fn test_generate_match_with_guard() {
    let value = 5;
    let _elem: Element = page! {
        match value {
            n if n < 0 => text (content: "negative") {},
            0 => text (content: "zero") {},
            _ => text (content: "positive") {},
        }
    };
}

// Braced expression
#[test]
fn test_generate_braced_expr() {
    let inner = Element::text("dynamic");
    let _elem: Element = page! {
        { inner }
    };
}

// Style attributes (in style parens)
#[test]
fn test_generate_with_bg() {
    let _elem: Element = page! {
        column style (bg: primary) {}
    };
}

#[test]
fn test_generate_with_fg() {
    let _elem: Element = page! {
        text (content: "colored") style (fg: secondary) {}
    };
}

#[test]
fn test_generate_with_bold() {
    let _elem: Element = page! {
        text (content: "bold") style (bold: true) {}
    };
}

// Multiple attributes
#[test]
fn test_generate_multiple_layout_attrs() {
    let _elem: Element = page! {
        column (padding: 1, gap: 2, justify: center, align: start) {}
    };
}

// Complex nested structure
#[test]
fn test_generate_complex() {
    let items = vec!["a", "b"];
    let show_header = true;

    let _elem: Element = page! {
        column (padding: 1, gap: 2) {
            if show_header {
                row style (bg: primary) {
                    text (content: "Header") {}
                }
            }
            for item in items {
                row (gap: 1) {
                    text (content: {item}) {}
                }
            }
        }
    };
}

// ID attribute
#[test]
fn test_generate_with_id() {
    let _elem: Element = page! {
        column (id: "main-container") {}
    };
}

// Focusable/clickable
#[test]
fn test_generate_interactive() {
    let _elem: Element = page! {
        column (focusable: true, clickable: true) {}
    };
}

// Style merging - multiple style attrs should combine into one .style() call
#[test]
fn test_generate_merged_styles() {
    // This should generate a single .style(Style::new().background(...).foreground(...).bold())
    let _elem: Element = page! {
        text (content: "styled") style (bg: primary, fg: secondary, bold: true) {}
    };
}

#[test]
fn test_generate_all_text_styles() {
    let _elem: Element = page! {
        text (content: "all styles") style (bold: true, italic: true, underline: true, dim: true) {}
    };
}

#[test]
fn test_generate_color_variants() {
    // Theme variable
    let _elem1: Element = page! {
        column style (bg: primary) {}
    };

    // Hex color
    let _elem2: Element = page! {
        column style (bg: "#ff0000") {}
    };

    // Built-in color names are treated as theme vars
    let _elem3: Element = page! {
        column style (bg: red) {}
    };
}

#[test]
fn test_generate_layout_and_style_together() {
    // Layout and style attrs should both work
    let _elem: Element = page! {
        column (padding: 2, gap: 1) style (bg: primary, bold: true) {
            text (content: "test") style (fg: secondary) {}
        }
    };
}

// Style-only element (no layout attrs)
#[test]
fn test_generate_style_only() {
    let _elem: Element = page! {
        row style (bg: surface, fg: primary) {
            text (content: "styled") {}
        }
    };
}

// Layout-only element (no style attrs)
#[test]
fn test_generate_layout_only() {
    let _elem: Element = page! {
        column (padding: 4, gap: 2, width: fill, justify: center) {
            row (align: center) {}
        }
    };
}
