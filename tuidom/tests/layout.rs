use tuidom::{Align, Edges, Element, Rect, Size};

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
