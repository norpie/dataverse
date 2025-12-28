use tuidom::{Align, Edges, Element, Position, Rect, Size};

fn layout_root(root: &Element, width: u16, height: u16) -> std::collections::HashMap<String, Rect> {
    tuidom::layout::layout(root, Rect::new(0, 0, width, height))
}

// ============================================================================
// Margin Tests
// ============================================================================

#[test]
fn test_margin_top_left() {
    let root = Element::box_()
        .id("root")
        .width(Size::Fixed(50))
        .height(Size::Fixed(50))
        .margin(Edges::new(5, 0, 0, 10));

    let layout = layout_root(&root, 100, 100);
    let rect = layout.get("root").unwrap();

    assert_eq!(rect.x, 10, "margin left");
    assert_eq!(rect.y, 5, "margin top");
    assert_eq!(rect.width, 50);
    assert_eq!(rect.height, 50);
}

#[test]
fn test_margin_shrinks_available_space() {
    let root = Element::box_()
        .id("root")
        .width(Size::Fill)
        .height(Size::Fill)
        .margin(Edges::all(10));

    let layout = layout_root(&root, 100, 100);
    let rect = layout.get("root").unwrap();

    assert_eq!(rect.x, 10);
    assert_eq!(rect.y, 10);
    assert_eq!(rect.width, 80); // 100 - 10 - 10
    assert_eq!(rect.height, 80);
}

#[test]
fn test_child_margin_in_column() {
    let root = Element::col()
        .id("root")
        .width(Size::Fixed(100))
        .height(Size::Fixed(100))
        .child(
            Element::box_()
                .id("child1")
                .height(Size::Fixed(20))
                .margin(Edges::new(5, 0, 5, 0)),
        )
        .child(Element::box_().id("child2").height(Size::Fixed(20)));

    let layout = layout_root(&root, 100, 100);

    let child1 = layout.get("child1").unwrap();
    assert_eq!(child1.y, 5, "child1 has margin top");
    assert_eq!(child1.height, 20);

    let child2 = layout.get("child2").unwrap();
    assert_eq!(child2.y, 30, "child2 starts after child1 + margins (5 + 20 + 5)");
}

// ============================================================================
// Min/Max Constraint Tests
// ============================================================================

#[test]
fn test_min_width() {
    let root = Element::box_()
        .id("root")
        .width(Size::Fixed(20))
        .min_width(50);

    let layout = layout_root(&root, 100, 100);
    let rect = layout.get("root").unwrap();

    assert_eq!(rect.width, 50, "min_width enforced");
}

#[test]
fn test_max_width() {
    let root = Element::box_()
        .id("root")
        .width(Size::Fill)
        .max_width(50);

    let layout = layout_root(&root, 100, 100);
    let rect = layout.get("root").unwrap();

    assert_eq!(rect.width, 50, "max_width enforced");
}

#[test]
fn test_min_max_height() {
    let root = Element::box_()
        .id("root")
        .height(Size::Fixed(10))
        .min_height(30)
        .max_height(80);

    let layout = layout_root(&root, 100, 100);
    let rect = layout.get("root").unwrap();

    assert_eq!(rect.height, 30, "min_height enforced");
}

#[test]
fn test_max_constrains_fill() {
    let root = Element::col()
        .id("root")
        .width(Size::Fixed(100))
        .height(Size::Fixed(100))
        .child(
            Element::box_()
                .id("child")
                .width(Size::Fill)
                .max_width(40),
        );

    let layout = layout_root(&root, 100, 100);
    let child = layout.get("child").unwrap();

    assert_eq!(child.width, 40, "max_width constrains Fill");
}

// ============================================================================
// Cross-Axis Alignment Tests
// ============================================================================

#[test]
fn test_align_start() {
    let root = Element::row()
        .id("root")
        .width(Size::Fixed(100))
        .height(Size::Fixed(100))
        .align(Align::Start)
        .child(
            Element::box_()
                .id("child")
                .width(Size::Fixed(20))
                .height(Size::Fixed(30)),
        );

    let layout = layout_root(&root, 100, 100);
    let child = layout.get("child").unwrap();

    assert_eq!(child.y, 0, "align start = top");
}

#[test]
fn test_align_center() {
    let root = Element::row()
        .id("root")
        .width(Size::Fixed(100))
        .height(Size::Fixed(100))
        .align(Align::Center)
        .child(
            Element::box_()
                .id("child")
                .width(Size::Fixed(20))
                .height(Size::Fixed(30)),
        );

    let layout = layout_root(&root, 100, 100);
    let child = layout.get("child").unwrap();

    assert_eq!(child.y, 35, "align center = (100-30)/2");
}

#[test]
fn test_align_end() {
    let root = Element::row()
        .id("root")
        .width(Size::Fixed(100))
        .height(Size::Fixed(100))
        .align(Align::End)
        .child(
            Element::box_()
                .id("child")
                .width(Size::Fixed(20))
                .height(Size::Fixed(30)),
        );

    let layout = layout_root(&root, 100, 100);
    let child = layout.get("child").unwrap();

    assert_eq!(child.y, 70, "align end = 100-30");
}

#[test]
fn test_align_stretch() {
    let root = Element::row()
        .id("root")
        .width(Size::Fixed(100))
        .height(Size::Fixed(100))
        .align(Align::Stretch)
        .child(Element::box_().id("child").width(Size::Fixed(20)));

    let layout = layout_root(&root, 100, 100);
    let child = layout.get("child").unwrap();

    assert_eq!(child.height, 100, "align stretch fills cross axis");
}

#[test]
fn test_align_column_direction() {
    let root = Element::col()
        .id("root")
        .width(Size::Fixed(100))
        .height(Size::Fixed(100))
        .align(Align::Center)
        .child(
            Element::box_()
                .id("child")
                .width(Size::Fixed(30))
                .height(Size::Fixed(20)),
        );

    let layout = layout_root(&root, 100, 100);
    let child = layout.get("child").unwrap();

    assert_eq!(child.x, 35, "column align center = horizontal center");
}

// ============================================================================
// align_self Tests
// ============================================================================

#[test]
fn test_align_self_overrides_parent() {
    let root = Element::row()
        .id("root")
        .width(Size::Fixed(100))
        .height(Size::Fixed(100))
        .align(Align::Start)
        .child(
            Element::box_()
                .id("child1")
                .width(Size::Fixed(20))
                .height(Size::Fixed(30)),
        )
        .child(
            Element::box_()
                .id("child2")
                .width(Size::Fixed(20))
                .height(Size::Fixed(30))
                .align_self(Align::End),
        );

    let layout = layout_root(&root, 100, 100);

    let child1 = layout.get("child1").unwrap();
    assert_eq!(child1.y, 0, "child1 uses parent align (start)");

    let child2 = layout.get("child2").unwrap();
    assert_eq!(child2.y, 70, "child2 uses align_self (end)");
}

#[test]
fn test_align_self_center() {
    let root = Element::row()
        .id("root")
        .width(Size::Fixed(100))
        .height(Size::Fixed(100))
        .align(Align::Start)
        .child(
            Element::box_()
                .id("child")
                .width(Size::Fixed(20))
                .height(Size::Fixed(40))
                .align_self(Align::Center),
        );

    let layout = layout_root(&root, 100, 100);
    let child = layout.get("child").unwrap();

    assert_eq!(child.y, 30, "align_self center = (100-40)/2");
}

// ============================================================================
// Position::Relative Tests
// ============================================================================

#[test]
fn test_relative_offset_left_top() {
    let root = Element::col()
        .id("root")
        .width(Size::Fixed(100))
        .height(Size::Fixed(100))
        .child(
            Element::box_()
                .id("child")
                .width(Size::Fixed(20))
                .height(Size::Fixed(20))
                .position(Position::Relative)
                .left(5)
                .top(10),
        );

    let layout = layout_root(&root, 100, 100);
    let child = layout.get("child").unwrap();

    assert_eq!(child.x, 5, "relative left offset");
    assert_eq!(child.y, 10, "relative top offset");
}

#[test]
fn test_relative_offset_right_bottom() {
    let root = Element::col()
        .id("root")
        .width(Size::Fixed(100))
        .height(Size::Fixed(100))
        .child(
            Element::box_()
                .id("child")
                .width(Size::Fixed(20))
                .height(Size::Fixed(20))
                .position(Position::Relative)
                .right(5)
                .bottom(10),
        );

    let layout = layout_root(&root, 100, 100);
    let child = layout.get("child").unwrap();

    // right moves element left (negative x), bottom moves element up (negative y)
    // Since normal position is (0, 0), right=5 means x = 0 - 5 = -5, clamped to 0
    // Same for bottom
    assert_eq!(child.x, 0, "relative right offset clamped to 0");
    assert_eq!(child.y, 0, "relative bottom offset clamped to 0");
}

#[test]
fn test_relative_still_takes_space_in_flow() {
    let root = Element::col()
        .id("root")
        .width(Size::Fixed(100))
        .height(Size::Fixed(100))
        .child(
            Element::box_()
                .id("child1")
                .width(Size::Fixed(20))
                .height(Size::Fixed(20))
                .position(Position::Relative)
                .left(50), // offset visually but still takes space at original position
        )
        .child(
            Element::box_()
                .id("child2")
                .width(Size::Fixed(20))
                .height(Size::Fixed(20)),
        );

    let layout = layout_root(&root, 100, 100);
    let child1 = layout.get("child1").unwrap();
    let child2 = layout.get("child2").unwrap();

    assert_eq!(child1.x, 50, "child1 offset visually");
    assert_eq!(child2.y, 20, "child2 starts after child1's original space");
}

// ============================================================================
// Position::Absolute Tests
// ============================================================================

#[test]
fn test_absolute_left_top() {
    let root = Element::box_()
        .id("root")
        .width(Size::Fixed(100))
        .height(Size::Fixed(100))
        .child(
            Element::box_()
                .id("child")
                .width(Size::Fixed(20))
                .height(Size::Fixed(20))
                .position(Position::Absolute)
                .left(10)
                .top(15),
        );

    let layout = layout_root(&root, 100, 100);
    let child = layout.get("child").unwrap();

    assert_eq!(child.x, 10, "absolute left");
    assert_eq!(child.y, 15, "absolute top");
}

#[test]
fn test_absolute_right_anchor() {
    let root = Element::box_()
        .id("root")
        .width(Size::Fixed(100))
        .height(Size::Fixed(100))
        .child(
            Element::box_()
                .id("child")
                .width(Size::Fixed(20))
                .height(Size::Fixed(20))
                .position(Position::Absolute)
                .right(10),
        );

    let layout = layout_root(&root, 100, 100);
    let child = layout.get("child").unwrap();

    // right=10 means: x = container.right - width - right = 100 - 20 - 10 = 70
    assert_eq!(child.x, 70, "absolute right anchor");
}

#[test]
fn test_absolute_bottom_anchor() {
    let root = Element::box_()
        .id("root")
        .width(Size::Fixed(100))
        .height(Size::Fixed(100))
        .child(
            Element::box_()
                .id("child")
                .width(Size::Fixed(20))
                .height(Size::Fixed(20))
                .position(Position::Absolute)
                .bottom(10),
        );

    let layout = layout_root(&root, 100, 100);
    let child = layout.get("child").unwrap();

    // bottom=10 means: y = container.bottom - height - bottom = 100 - 20 - 10 = 70
    assert_eq!(child.y, 70, "absolute bottom anchor");
}

#[test]
fn test_absolute_left_right_stretches_width() {
    let root = Element::box_()
        .id("root")
        .width(Size::Fixed(100))
        .height(Size::Fixed(100))
        .child(
            Element::box_()
                .id("child")
                .height(Size::Fixed(20))
                .position(Position::Absolute)
                .left(10)
                .right(20),
        );

    let layout = layout_root(&root, 100, 100);
    let child = layout.get("child").unwrap();

    assert_eq!(child.x, 10, "left anchor");
    assert_eq!(child.width, 70, "width = 100 - 10 - 20");
}

#[test]
fn test_absolute_top_bottom_stretches_height() {
    let root = Element::box_()
        .id("root")
        .width(Size::Fixed(100))
        .height(Size::Fixed(100))
        .child(
            Element::box_()
                .id("child")
                .width(Size::Fixed(20))
                .position(Position::Absolute)
                .top(5)
                .bottom(15),
        );

    let layout = layout_root(&root, 100, 100);
    let child = layout.get("child").unwrap();

    assert_eq!(child.y, 5, "top anchor");
    assert_eq!(child.height, 80, "height = 100 - 5 - 15");
}

#[test]
fn test_absolute_all_anchors_stretches_both() {
    let root = Element::box_()
        .id("root")
        .width(Size::Fixed(100))
        .height(Size::Fixed(100))
        .child(
            Element::box_()
                .id("child")
                .position(Position::Absolute)
                .left(10)
                .right(10)
                .top(10)
                .bottom(10),
        );

    let layout = layout_root(&root, 100, 100);
    let child = layout.get("child").unwrap();

    assert_eq!(child.x, 10);
    assert_eq!(child.y, 10);
    assert_eq!(child.width, 80, "width = 100 - 10 - 10");
    assert_eq!(child.height, 80, "height = 100 - 10 - 10");
}
