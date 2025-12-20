use rafter::utils::geometry::{intersect_rects, rects_overlap};
use ratatui::layout::Rect;

#[test]
fn test_rects_overlap_intersecting() {
    let a = Rect::new(0, 0, 10, 10);
    let b = Rect::new(5, 5, 10, 10);
    assert!(rects_overlap(a, b));
}

#[test]
fn test_rects_overlap_non_intersecting() {
    let a = Rect::new(0, 0, 10, 10);
    let b = Rect::new(20, 20, 10, 10);
    assert!(!rects_overlap(a, b));
}

#[test]
fn test_rects_overlap_adjacent() {
    let a = Rect::new(0, 0, 10, 10);
    let b = Rect::new(10, 0, 10, 10);
    assert!(!rects_overlap(a, b));
}

#[test]
fn test_intersect_rects_overlapping() {
    let a = Rect::new(0, 0, 10, 10);
    let b = Rect::new(5, 5, 10, 10);
    let intersection = intersect_rects(a, b);
    assert_eq!(intersection, Rect::new(5, 5, 5, 5));
}

#[test]
fn test_intersect_rects_non_overlapping() {
    let a = Rect::new(0, 0, 10, 10);
    let b = Rect::new(20, 20, 10, 10);
    let intersection = intersect_rects(a, b);
    assert_eq!(intersection.width, 0);
    assert_eq!(intersection.height, 0);
}

#[test]
fn test_intersect_rects_contained() {
    let outer = Rect::new(0, 0, 20, 20);
    let inner = Rect::new(5, 5, 10, 10);
    let intersection = intersect_rects(outer, inner);
    assert_eq!(intersection, inner);
}
