use tuidom::{
    collect_focusable, hit_test, hit_test_any, hit_test_focusable, Element, FocusState,
    LayoutResult, Rect,
};

fn create_layout(elements: &[(&str, Rect)]) -> LayoutResult {
    let mut layout = LayoutResult::new();
    for (id, rect) in elements {
        layout.insert(id.to_string(), *rect);
    }
    layout
}

// ============================================================================
// Hit Testing
// ============================================================================

#[test]
fn test_hit_test_point_inside() {
    let root = Element::box_()
        .id("root")
        .clickable(true)
        .child(Element::text("Click me").id("btn").clickable(true));

    let layout = create_layout(&[
        ("root", Rect::new(0, 0, 100, 50)),
        ("btn", Rect::new(10, 10, 30, 3)),
    ]);

    // Click inside btn
    assert_eq!(hit_test(&layout, &root, 15, 11), Some("btn".to_string()));

    // Click inside root but outside btn
    assert_eq!(hit_test(&layout, &root, 5, 5), Some("root".to_string()));

    // Click outside everything
    assert_eq!(hit_test(&layout, &root, 150, 150), None);
}

#[test]
fn test_hit_test_overlapping_elements() {
    // Later children should be "on top"
    let root = Element::box_()
        .id("root")
        .child(Element::box_().id("bottom").clickable(true))
        .child(Element::box_().id("top").clickable(true));

    let layout = create_layout(&[
        ("root", Rect::new(0, 0, 100, 100)),
        ("bottom", Rect::new(10, 10, 50, 50)),
        ("top", Rect::new(30, 30, 50, 50)), // Overlaps with bottom
    ]);

    // Click in overlapping region - top should win
    assert_eq!(hit_test(&layout, &root, 40, 40), Some("top".to_string()));

    // Click only in bottom (before overlap)
    assert_eq!(hit_test(&layout, &root, 15, 15), Some("bottom".to_string()));
}

#[test]
fn test_hit_test_only_clickable() {
    let root = Element::box_()
        .id("root")
        .child(Element::text("Not clickable").id("text"));

    let layout = create_layout(&[
        ("root", Rect::new(0, 0, 100, 50)),
        ("text", Rect::new(10, 10, 30, 3)),
    ]);

    // Click on non-clickable element returns None
    assert_eq!(hit_test(&layout, &root, 15, 11), None);
}

#[test]
fn test_hit_test_any() {
    let root = Element::box_()
        .id("root")
        .child(Element::text("Not clickable").id("text"));

    let layout = create_layout(&[
        ("root", Rect::new(0, 0, 100, 50)),
        ("text", Rect::new(10, 10, 30, 3)),
    ]);

    // hit_test_any returns element even if not clickable
    assert_eq!(
        hit_test_any(&layout, &root, 15, 11),
        Some("text".to_string())
    );
}

#[test]
fn test_hit_test_focusable() {
    let root = Element::box_()
        .id("root")
        .child(Element::text("Focusable").id("input").focusable(true))
        .child(Element::text("Not focusable").id("text"));

    let layout = create_layout(&[
        ("root", Rect::new(0, 0, 100, 50)),
        ("input", Rect::new(10, 10, 30, 3)),
        ("text", Rect::new(10, 20, 30, 3)),
    ]);

    // Hit focusable element
    assert_eq!(
        hit_test_focusable(&layout, &root, 15, 11),
        Some("input".to_string())
    );

    // Hit non-focusable element
    assert_eq!(hit_test_focusable(&layout, &root, 15, 21), None);
}

// ============================================================================
// Focus State
// ============================================================================

#[test]
fn test_focus_state_focus_blur() {
    let mut focus = FocusState::new();

    assert_eq!(focus.focused(), None);

    // Focus an element
    assert!(focus.focus("input1"));
    assert_eq!(focus.focused(), Some("input1"));

    // Focus same element - no change
    assert!(!focus.focus("input1"));

    // Focus different element
    assert!(focus.focus("input2"));
    assert_eq!(focus.focused(), Some("input2"));

    // Blur
    assert!(focus.blur());
    assert_eq!(focus.focused(), None);

    // Blur when nothing focused
    assert!(!focus.blur());
}

#[test]
fn test_focus_next_navigation() {
    let root = Element::col()
        .child(Element::text("Input 1").id("input1").focusable(true))
        .child(Element::text("Input 2").id("input2").focusable(true))
        .child(Element::text("Input 3").id("input3").focusable(true));

    let mut focus = FocusState::new();

    // Focus first when nothing focused
    assert_eq!(focus.focus_next(&root), Some("input1".to_string()));
    assert_eq!(focus.focused(), Some("input1"));

    // Focus next
    assert_eq!(focus.focus_next(&root), Some("input2".to_string()));
    assert_eq!(focus.focused(), Some("input2"));

    // Focus next
    assert_eq!(focus.focus_next(&root), Some("input3".to_string()));
    assert_eq!(focus.focused(), Some("input3"));

    // Wrap around
    assert_eq!(focus.focus_next(&root), Some("input1".to_string()));
    assert_eq!(focus.focused(), Some("input1"));
}

#[test]
fn test_focus_prev_navigation() {
    let root = Element::col()
        .child(Element::text("Input 1").id("input1").focusable(true))
        .child(Element::text("Input 2").id("input2").focusable(true))
        .child(Element::text("Input 3").id("input3").focusable(true));

    let mut focus = FocusState::new();

    // Focus last when nothing focused
    assert_eq!(focus.focus_prev(&root), Some("input3".to_string()));
    assert_eq!(focus.focused(), Some("input3"));

    // Focus prev
    assert_eq!(focus.focus_prev(&root), Some("input2".to_string()));
    assert_eq!(focus.focused(), Some("input2"));

    // Focus prev
    assert_eq!(focus.focus_prev(&root), Some("input1".to_string()));
    assert_eq!(focus.focused(), Some("input1"));

    // Wrap around
    assert_eq!(focus.focus_prev(&root), Some("input3".to_string()));
    assert_eq!(focus.focused(), Some("input3"));
}

#[test]
fn test_focus_no_focusable_elements() {
    let root = Element::col()
        .child(Element::text("Not focusable").id("text1"))
        .child(Element::text("Also not").id("text2"));

    let mut focus = FocusState::new();

    assert_eq!(focus.focus_next(&root), None);
    assert_eq!(focus.focus_prev(&root), None);
}

#[test]
fn test_focus_single_element() {
    let root = Element::col().child(Element::text("Only one").id("input1").focusable(true));

    let mut focus = FocusState::new();

    // Focus it
    assert_eq!(focus.focus_next(&root), Some("input1".to_string()));

    // Next returns None (already focused, can't change to same)
    assert_eq!(focus.focus_next(&root), None);

    // Same for prev
    assert_eq!(focus.focus_prev(&root), None);
}

// ============================================================================
// Collect Focusable
// ============================================================================

#[test]
fn test_collect_focusable_order() {
    let root = Element::col()
        .id("root")
        .focusable(true)
        .child(
            Element::col()
                .id("group1")
                .child(Element::text("A").id("a").focusable(true))
                .child(Element::text("B").id("b").focusable(true)),
        )
        .child(Element::text("C").id("c").focusable(true));

    let focusable = collect_focusable(&root);
    assert_eq!(focusable, vec!["root", "a", "b", "c"]);
}

#[test]
fn test_collect_focusable_nested() {
    let root = Element::col().child(
        Element::col()
            .child(Element::col().child(Element::text("Deep").id("deep").focusable(true))),
    );

    let focusable = collect_focusable(&root);
    assert_eq!(focusable, vec!["deep"]);
}

#[test]
fn test_collect_focusable_empty() {
    let root = Element::col()
        .child(Element::text("Not focusable"))
        .child(Element::text("Also not"));

    let focusable = collect_focusable(&root);
    assert!(focusable.is_empty());
}
