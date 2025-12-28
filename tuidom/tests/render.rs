use tuidom::{Buffer, Color, Element, Position, Rect, Size, Style};

fn render_to_buffer(root: &Element, width: u16, height: u16) -> Buffer {
    let layout = tuidom::layout::layout(root, Rect::new(0, 0, width, height));
    let mut buf = Buffer::new(width, height);
    tuidom::render::render_to_buffer(root, &layout, &mut buf);
    buf
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
    assert_eq!(cell.bg.g, 255, "Green (higher z_index) should be on top");
    assert_eq!(cell.bg.r, 0, "Red should not show through");
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
    assert_eq!(cell.bg.r, 255, "Red (lower z_index) should be visible where not overlapped");
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
    assert_eq!(cell.bg.g, 255, "Later in tree should render on top when z_index equal");
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
    assert_eq!(cell.bg.b, 255, "Higher z_index child should render on top of lower z_index sibling");
    assert_eq!(cell.bg.r, 0, "Yellow should not show through");
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
    assert_eq!(cell.bg.g, 255, "z_index 0 should be on top of z_index -1");
}
