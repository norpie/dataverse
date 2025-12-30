use tuidom::animation::AnimationState;
use tuidom::{Buffer, Color, Element, Oklch, Overflow, Position, Rect, Rgb, Size, Style};

fn render_to_buffer(root: &Element, width: u16, height: u16) -> Buffer {
    let layout = tuidom::layout::layout(root, Rect::new(0, 0, width, height));
    let mut buf = Buffer::new(width, height);
    let animation = AnimationState::new();
    tuidom::render::render_to_buffer(root, &layout, &mut buf, &animation);
    buf
}

/// Convert cell's Oklch color to RGB for comparison in tests.
fn to_rgb(oklch: Oklch) -> Rgb {
    oklch.to_rgb()
}

// ============================================================================
// z_index Tests
// ============================================================================

#[test]
fn test_higher_z_index_renders_on_top() {
    // Two overlapping boxes - higher z_index should be on top
    let root = Element::box_()
        .id("root")
        .width(Size::Fixed(20))
        .height(Size::Fixed(10))
        .child(
            Element::box_()
                .id("bottom")
                .width(Size::Fixed(10))
                .height(Size::Fixed(5))
                .position(Position::Absolute)
                .left(0)
                .top(0)
                .z_index(0)
                .style(Style::new().background(Color::rgb(255, 0, 0))), // Red
        )
        .child(
            Element::box_()
                .id("top")
                .width(Size::Fixed(10))
                .height(Size::Fixed(5))
                .position(Position::Absolute)
                .left(5)
                .top(2)
                .z_index(1)
                .style(Style::new().background(Color::rgb(0, 255, 0))), // Green
        );

    let buf = render_to_buffer(&root, 20, 10);

    // At position (7, 3) - overlap area - should be green (top element)
    let cell = buf.get(7, 3).unwrap();
    let bg = to_rgb(cell.bg);
    assert_eq!(bg.g, 255, "Green (higher z_index) should be on top");
    assert_eq!(bg.r, 0, "Red should not show through");
}

#[test]
fn test_lower_z_index_renders_underneath() {
    // Same as above but check a position only covered by the bottom element
    let root = Element::box_()
        .id("root")
        .width(Size::Fixed(20))
        .height(Size::Fixed(10))
        .child(
            Element::box_()
                .id("bottom")
                .width(Size::Fixed(10))
                .height(Size::Fixed(5))
                .position(Position::Absolute)
                .left(0)
                .top(0)
                .z_index(0)
                .style(Style::new().background(Color::rgb(255, 0, 0))), // Red
        )
        .child(
            Element::box_()
                .id("top")
                .width(Size::Fixed(10))
                .height(Size::Fixed(5))
                .position(Position::Absolute)
                .left(5)
                .top(2)
                .z_index(1)
                .style(Style::new().background(Color::rgb(0, 255, 0))), // Green
        );

    let buf = render_to_buffer(&root, 20, 10);

    // At position (2, 1) - only bottom element - should be red
    let cell = buf.get(2, 1).unwrap();
    let bg = to_rgb(cell.bg);
    assert_eq!(bg.r, 255, "Red (lower z_index) should be visible where not overlapped");
}

#[test]
fn test_equal_z_index_preserves_tree_order() {
    // Two overlapping boxes with same z_index - later in tree wins
    let root = Element::box_()
        .id("root")
        .width(Size::Fixed(20))
        .height(Size::Fixed(10))
        .child(
            Element::box_()
                .id("first")
                .width(Size::Fixed(10))
                .height(Size::Fixed(5))
                .position(Position::Absolute)
                .left(0)
                .top(0)
                .z_index(0)
                .style(Style::new().background(Color::rgb(255, 0, 0))), // Red - first in tree
        )
        .child(
            Element::box_()
                .id("second")
                .width(Size::Fixed(10))
                .height(Size::Fixed(5))
                .position(Position::Absolute)
                .left(5)
                .top(2)
                .z_index(0) // Same z_index
                .style(Style::new().background(Color::rgb(0, 255, 0))), // Green - second in tree
        );

    let buf = render_to_buffer(&root, 20, 10);

    // At overlap position - second (green) should be on top due to tree order
    let cell = buf.get(7, 3).unwrap();
    let bg = to_rgb(cell.bg);
    assert_eq!(bg.g, 255, "Later in tree should render on top when z_index equal");
}

#[test]
fn test_z_index_works_across_siblings() {
    // Child of first sibling has higher z_index than second sibling
    let root = Element::box_()
        .id("root")
        .width(Size::Fixed(20))
        .height(Size::Fixed(10))
        .child(
            Element::box_()
                .id("first_parent")
                .width(Size::Fixed(15))
                .height(Size::Fixed(8))
                .position(Position::Absolute)
                .left(0)
                .top(0)
                .child(
                    Element::box_()
                        .id("high_z_child")
                        .width(Size::Fixed(8))
                        .height(Size::Fixed(4))
                        .position(Position::Absolute)
                        .left(2)
                        .top(2)
                        .z_index(10) // High z_index
                        .style(Style::new().background(Color::rgb(0, 0, 255))), // Blue
                ),
        )
        .child(
            Element::box_()
                .id("second_sibling")
                .width(Size::Fixed(10))
                .height(Size::Fixed(6))
                .position(Position::Absolute)
                .left(5)
                .top(1)
                .z_index(5) // Lower z_index than the nested child
                .style(Style::new().background(Color::rgb(255, 255, 0))), // Yellow
        );

    let buf = render_to_buffer(&root, 20, 10);

    // At overlap position - blue (z=10) should be on top of yellow (z=5)
    let cell = buf.get(6, 3).unwrap();
    let bg = to_rgb(cell.bg);
    assert_eq!(bg.b, 255, "Higher z_index child should render on top of lower z_index sibling");
    assert_eq!(bg.r, 0, "Yellow should not show through");
}

#[test]
fn test_negative_z_index() {
    // Negative z_index should render behind default (0)
    let root = Element::box_()
        .id("root")
        .width(Size::Fixed(20))
        .height(Size::Fixed(10))
        .child(
            Element::box_()
                .id("behind")
                .width(Size::Fixed(10))
                .height(Size::Fixed(5))
                .position(Position::Absolute)
                .left(0)
                .top(0)
                .z_index(-1) // Negative - should render first
                .style(Style::new().background(Color::rgb(255, 0, 0))), // Red
        )
        .child(
            Element::box_()
                .id("front")
                .width(Size::Fixed(10))
                .height(Size::Fixed(5))
                .position(Position::Absolute)
                .left(5)
                .top(2)
                .z_index(0) // Default
                .style(Style::new().background(Color::rgb(0, 255, 0))), // Green
        );

    let buf = render_to_buffer(&root, 20, 10);

    // At overlap - green (z=0) should be on top of red (z=-1)
    let cell = buf.get(7, 3).unwrap();
    let bg = to_rgb(cell.bg);
    assert_eq!(bg.g, 255, "z_index 0 should be on top of z_index -1");
}

// ============================================================================
// Overflow Tests
// ============================================================================

#[test]
fn test_overflow_hidden_clips_children() {
    // Parent with overflow: hidden clips child that extends beyond
    let root = Element::box_()
        .id("container")
        .width(Size::Fixed(10))
        .height(Size::Fixed(5))
        .overflow(Overflow::Hidden)
        .style(Style::new().background(Color::rgb(50, 50, 50))) // Dark gray
        .child(
            Element::box_()
                .id("child")
                .width(Size::Fixed(20)) // Child is wider than parent
                .height(Size::Fixed(3))
                .style(Style::new().background(Color::rgb(255, 0, 0))), // Red
        );

    let buf = render_to_buffer(&root, 20, 10);

    // Inside container (x=5, y=1) - should show red child
    let inside = buf.get(5, 1).unwrap();
    let inside_bg = to_rgb(inside.bg);
    assert_eq!(inside_bg.r, 255, "Child should be visible inside container");

    // Outside container (x=12, y=1) - should NOT show red
    let outside = buf.get(12, 1).unwrap();
    let outside_bg = to_rgb(outside.bg);
    assert_ne!(
        outside_bg.r, 255,
        "Child should be clipped outside container"
    );
}

#[test]
fn test_overflow_visible_does_not_clip() {
    // Parent with overflow: visible (default) does not clip
    // Use absolute positioning so child can extend beyond parent
    let root = Element::box_()
        .id("container")
        .width(Size::Fixed(10))
        .height(Size::Fixed(5))
        .overflow(Overflow::Visible)
        .style(Style::new().background(Color::rgb(50, 50, 50)))
        .child(
            Element::box_()
                .id("child")
                .position(Position::Absolute)
                .left(0)
                .top(0)
                .width(Size::Fixed(20)) // Child extends beyond parent
                .height(Size::Fixed(3))
                .style(Style::new().background(Color::rgb(255, 0, 0))),
        );

    let buf = render_to_buffer(&root, 20, 10);

    // Outside container - child should still be visible (not clipped)
    let outside = buf.get(12, 1).unwrap();
    let outside_bg = to_rgb(outside.bg);
    assert_eq!(
        outside_bg.r, 255,
        "Child should extend beyond container with overflow: visible"
    );
}

#[test]
fn test_overflow_scroll_clips_children() {
    // Scroll should also clip like Hidden
    let root = Element::box_()
        .id("container")
        .width(Size::Fixed(10))
        .height(Size::Fixed(5))
        .overflow(Overflow::Scroll)
        .style(Style::new().background(Color::rgb(50, 50, 50)))
        .child(
            Element::box_()
                .id("child")
                .width(Size::Fixed(20))
                .height(Size::Fixed(3))
                .style(Style::new().background(Color::rgb(255, 0, 0))),
        );

    let buf = render_to_buffer(&root, 20, 10);

    // Outside container - should be clipped
    let outside = buf.get(12, 1).unwrap();
    let outside_bg = to_rgb(outside.bg);
    assert_ne!(
        outside_bg.r, 255,
        "Scroll overflow should clip like Hidden"
    );
}

#[test]
fn test_scroll_offset_moves_children() {
    // scroll_offset should move children up/left
    let root = Element::box_()
        .id("container")
        .width(Size::Fixed(10))
        .height(Size::Fixed(5))
        .overflow(Overflow::Scroll)
        .scroll_offset(0, 2) // Scroll down by 2
        .style(Style::new().background(Color::rgb(50, 50, 50)))
        .child(
            Element::text("Line 1").id("line1"),
        )
        .child(
            Element::text("Line 2").id("line2"),
        )
        .child(
            Element::text("Line 3").id("line3"),
        )
        .child(
            Element::text("Line 4").id("line4"),
        )
        .child(
            Element::text("Line 5").id("line5"),
        );

    let layout = tuidom::layout::layout(&root, Rect::new(0, 0, 20, 10));

    // Line 1 should be at y = 0 - 2 = -2 (effectively not visible, clamped to 0 but scrolled out)
    // Line 3 should be at y = 2 - 2 = 0
    let line3_rect = layout.get("line3").unwrap();
    assert_eq!(
        line3_rect.y, 0,
        "Line 3 should be scrolled to top (y=0) with scroll_offset 2"
    );
}

#[test]
fn test_scroll_renders_scrollbar() {
    // Scroll overflow should render a scrollbar
    let root = Element::box_()
        .id("container")
        .width(Size::Fixed(10))
        .height(Size::Fixed(5))
        .overflow(Overflow::Scroll)
        .style(Style::new().background(Color::rgb(50, 50, 50)));

    let buf = render_to_buffer(&root, 20, 10);

    // Right edge should have scrollbar characters
    let scrollbar_cell = buf.get(9, 2).unwrap();
    assert!(
        scrollbar_cell.char == '█' || scrollbar_cell.char == '░',
        "Scrollbar should be rendered on right edge"
    );
}

#[test]
fn test_nested_overflow_hidden() {
    // Nested overflow containers - each clips independently
    let root = Element::box_()
        .id("outer")
        .width(Size::Fixed(20))
        .height(Size::Fixed(10))
        .overflow(Overflow::Hidden)
        .style(Style::new().background(Color::rgb(30, 30, 30)))
        .child(
            Element::box_()
                .id("inner")
                .width(Size::Fixed(15))
                .height(Size::Fixed(8))
                .overflow(Overflow::Hidden)
                .style(Style::new().background(Color::rgb(60, 60, 60)))
                .child(
                    Element::box_()
                        .id("content")
                        .width(Size::Fixed(30)) // Much wider than both containers
                        .height(Size::Fixed(3))
                        .style(Style::new().background(Color::rgb(255, 0, 0))),
                ),
        );

    let buf = render_to_buffer(&root, 30, 15);

    // Inside inner container - should show red
    let inside_inner = buf.get(10, 1).unwrap();
    let inside_inner_bg = to_rgb(inside_inner.bg);
    assert_eq!(inside_inner_bg.r, 255, "Content visible inside inner container");

    // Outside inner but inside outer - should NOT show red (clipped by inner)
    let outside_inner = buf.get(17, 1).unwrap();
    let outside_inner_bg = to_rgb(outside_inner.bg);
    assert_ne!(
        outside_inner_bg.r, 255,
        "Content clipped by inner container"
    );

    // Outside outer - should also not show red
    let outside_outer = buf.get(22, 1).unwrap();
    let outside_outer_bg = to_rgb(outside_outer.bg);
    assert_ne!(
        outside_outer_bg.r, 255,
        "Content clipped by outer container"
    );
}
