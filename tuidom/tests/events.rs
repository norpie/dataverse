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
// Hit Testing: Overflow Clipping
// ============================================================================

#[test]
fn test_hit_test_overflow_hidden_clips_children() {
    // Parent has overflow: Hidden and is 100x20.
    // Child extends beyond the parent (positioned at y=25, outside the parent's 0..20 range).
    // Click on the child's layout rect but outside the parent's clip → no hit.
    use tuidom::Overflow;

    let root = Element::col()
        .id("root")
        .overflow_y(Overflow::Hidden)
        .child(Element::text("Visible").id("visible-btn").clickable(true))
        .child(Element::text("Clipped").id("clipped-btn").clickable(true));

    let layout = create_layout(&[
        ("root", Rect::new(0, 0, 100, 20)),
        ("visible-btn", Rect::new(0, 5, 100, 3)),
        ("clipped-btn", Rect::new(0, 25, 100, 3)), // Beyond parent's height
    ]);

    // Click on visible child → hit
    assert_eq!(
        hit_test(&layout, &root, 50, 6),
        Some("visible-btn".to_string())
    );

    // Click at y=26 which is inside clipped-btn's rect but outside root's clip
    assert_eq!(hit_test(&layout, &root, 50, 26), None);
}

#[test]
fn test_hit_test_overflow_hidden_allows_visible_area() {
    // Children within the clipped area should still be clickable
    use tuidom::Overflow;

    let root = Element::col()
        .id("root")
        .overflow_y(Overflow::Hidden)
        .child(Element::text("Inside").id("btn-inside").clickable(true));

    let layout = create_layout(&[
        ("root", Rect::new(5, 5, 50, 30)),
        ("btn-inside", Rect::new(5, 10, 50, 3)),
    ]);

    // Click inside both the child and the parent's clip → hit
    assert_eq!(
        hit_test(&layout, &root, 20, 11),
        Some("btn-inside".to_string())
    );
}

#[test]
fn test_hit_test_overflow_visible_does_not_clip() {
    // When overflow is Visible (default), children outside parent rect ARE hittable
    let root = Element::col()
        .id("root")
        .child(Element::text("Outside").id("btn-outside").clickable(true));

    let layout = create_layout(&[
        ("root", Rect::new(0, 0, 100, 20)),
        ("btn-outside", Rect::new(0, 25, 100, 3)), // Beyond parent
    ]);

    // With overflow: Visible, clicking outside parent still hits the child
    assert_eq!(
        hit_test(&layout, &root, 50, 26),
        Some("btn-outside".to_string())
    );
}

#[test]
fn test_hit_test_overflow_scroll_clips_children() {
    // Overflow::Scroll should clip children the same as Hidden
    use tuidom::Overflow;

    let root = Element::col()
        .id("root")
        .overflow_y(Overflow::Scroll)
        .child(Element::text("Visible").id("visible").clickable(true))
        .child(Element::text("Clipped").id("clipped").clickable(true));

    let layout = create_layout(&[
        ("root", Rect::new(0, 0, 100, 20)),
        ("visible", Rect::new(0, 5, 100, 3)),
        ("clipped", Rect::new(0, 25, 100, 3)),
    ]);

    assert_eq!(hit_test(&layout, &root, 50, 6), Some("visible".to_string()));
    assert_eq!(hit_test(&layout, &root, 50, 26), None);
}

#[test]
fn test_hit_test_nested_overflow_clips() {
    // Nested clipping: outer clips to 0..20, inner clips to 5..15
    // Child at y=17 is inside outer but outside inner → no hit
    use tuidom::Overflow;

    let inner = Element::col()
        .id("inner")
        .overflow_y(Overflow::Hidden)
        .child(Element::text("Deep").id("deep-btn").clickable(true));

    let root = Element::col()
        .id("outer")
        .overflow_y(Overflow::Hidden)
        .child(inner);

    let layout = create_layout(&[
        ("outer", Rect::new(0, 0, 100, 20)),
        ("inner", Rect::new(0, 5, 100, 10)), // Clips to y: 5..15
        ("deep-btn", Rect::new(0, 12, 100, 3)),
    ]);

    // y=13 is inside inner (5..15) and outer (0..20) → hit
    assert_eq!(
        hit_test(&layout, &root, 50, 13),
        Some("deep-btn".to_string())
    );

    // Now test a child outside the inner clip
    let inner2 = Element::col()
        .id("inner2")
        .overflow_y(Overflow::Hidden)
        .child(Element::text("Deep2").id("deep-btn2").clickable(true));

    let root2 = Element::col()
        .id("outer2")
        .overflow_y(Overflow::Hidden)
        .child(inner2);

    let layout2 = create_layout(&[
        ("outer2", Rect::new(0, 0, 100, 20)),
        ("inner2", Rect::new(0, 5, 100, 10)), // Clips to y: 5..15
        ("deep-btn2", Rect::new(0, 17, 100, 3)), // Outside inner's clip
    ]);

    // y=18 is inside outer (0..20) but outside inner (5..15) → no hit
    assert_eq!(hit_test(&layout2, &root2, 50, 18), None);
}

#[test]
fn test_hit_test_overflow_x_clips_horizontally() {
    use tuidom::Overflow;

    let root = Element::col()
        .id("root")
        .overflow_x(Overflow::Hidden)
        .child(Element::text("Wide").id("wide-btn").clickable(true));

    let layout = create_layout(&[
        ("root", Rect::new(0, 0, 50, 20)),
        ("wide-btn", Rect::new(0, 5, 100, 3)), // Extends beyond parent width
    ]);

    // Click inside clip → hit
    assert_eq!(
        hit_test(&layout, &root, 25, 6),
        Some("wide-btn".to_string())
    );

    // Click outside clip (x=60, beyond parent's width of 50) → no hit
    assert_eq!(hit_test(&layout, &root, 60, 6), None);
}

#[test]
fn test_hit_test_scrolled_out_element_not_hittable() {
    // Simulates what happens when scroll offset pushes a child's layout rect
    // to y=0 (via saturating_sub), but the parent clips to its own area.
    use tuidom::Overflow;

    let root = Element::col()
        .id("root")
        .overflow_y(Overflow::Hidden)
        .child(
            Element::text("Scrolled out")
                .id("scrolled-out")
                .clickable(true),
        )
        .child(
            Element::text("Visible item")
                .id("visible-item")
                .clickable(true),
        );

    // After scroll offset, "scrolled-out" has been clamped to y=0 (saturating_sub).
    // The root starts at y=5, so this child is above the clip region.
    let layout = create_layout(&[
        ("root", Rect::new(0, 5, 100, 15)),
        ("scrolled-out", Rect::new(0, 0, 100, 3)), // Clamped above root
        ("visible-item", Rect::new(0, 8, 100, 3)),
    ]);

    // Click at y=1 is inside scrolled-out's rect but above root's clip → no hit
    assert_eq!(hit_test(&layout, &root, 50, 1), None);

    // Click at y=9 is inside visible-item and inside root's clip → hit
    assert_eq!(
        hit_test(&layout, &root, 50, 9),
        Some("visible-item".to_string())
    );
}

// ============================================================================
// Focus State
// ============================================================================

#[test]
fn test_focus_state_focus_blur() {
    let mut focus = FocusState::new();

    // Create a simple root with focusable elements
    let root = Element::col()
        .child(Element::text("Input 1").id("input1").focusable(true))
        .child(Element::text("Input 2").id("input2").focusable(true));

    assert_eq!(focus.focused(), None);

    // Focus an element - should return Focus event
    let events = focus.focus("input1", &root);
    assert_eq!(events.len(), 1);
    assert_eq!(focus.focused(), Some("input1"));

    // Focus same element - no change, empty events
    let events = focus.focus("input1", &root);
    assert!(events.is_empty());

    // Focus different element - should return Blur + Focus events
    let events = focus.focus("input2", &root);
    assert_eq!(events.len(), 2);
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

    let focusable = collect_focusable(&root, None);
    assert_eq!(focusable, vec!["root", "a", "b", "c"]);
}

#[test]
fn test_collect_focusable_nested() {
    let root = Element::col().child(
        Element::col()
            .child(Element::col().child(Element::text("Deep").id("deep").focusable(true))),
    );

    let focusable = collect_focusable(&root, None);
    assert_eq!(focusable, vec!["deep"]);
}

#[test]
fn test_collect_focusable_empty() {
    let root = Element::col()
        .child(Element::text("Not focusable"))
        .child(Element::text("Also not"));

    let focusable = collect_focusable(&root, None);
    assert!(focusable.is_empty());
}
