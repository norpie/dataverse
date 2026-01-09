use crate::element::{Content, Element};
use crate::layout::LayoutResult;

/// A potential hit with its effective z-index.
#[derive(Debug)]
struct Hit {
    id: String,
    z_index: i16,
    /// Depth in tree (for tie-breaking: deeper = on top for same z-index)
    depth: u32,
}

/// Find the deepest clickable element at the given coordinates.
/// Returns None if no clickable element contains the point.
/// Respects z-index ordering.
pub fn hit_test(layout: &LayoutResult, root: &Element, x: u16, y: u16) -> Option<String> {
    let mut hits = Vec::new();
    collect_hits(layout, root, x, y, 0, 0, &mut hits, |el| el.clickable);
    best_hit(hits)
}

/// Find any element (clickable or not) at the given coordinates.
/// Returns the deepest element containing the point.
/// Respects z-index ordering.
pub fn hit_test_any(layout: &LayoutResult, root: &Element, x: u16, y: u16) -> Option<String> {
    let mut hits = Vec::new();
    collect_hits(layout, root, x, y, 0, 0, &mut hits, |_| true);
    best_hit(hits)
}

/// Find the focusable element at the given coordinates.
/// Returns None if no focusable element contains the point.
/// Respects z-index ordering.
pub fn hit_test_focusable(layout: &LayoutResult, root: &Element, x: u16, y: u16) -> Option<String> {
    let mut hits = Vec::new();
    collect_hits(layout, root, x, y, 0, 0, &mut hits, |el| el.focusable);
    best_hit(hits)
}

/// Find the scrollable element at the given coordinates.
/// Returns None if no scrollable element contains the point.
/// Respects z-index ordering.
pub fn hit_test_scrollable(layout: &LayoutResult, root: &Element, x: u16, y: u16) -> Option<String> {
    let mut hits = Vec::new();
    collect_hits(layout, root, x, y, 0, 0, &mut hits, |el| el.scrollable);
    best_hit(hits)
}

/// Select the best hit: highest z-index, then deepest in tree.
fn best_hit(mut hits: Vec<Hit>) -> Option<String> {
    if hits.is_empty() {
        return None;
    }

    log::debug!("hit_test: collected {} hits:", hits.len());
    for hit in &hits {
        log::debug!("  - id={} z_index={} depth={}", hit.id, hit.z_index, hit.depth);
    }

    // Sort by z_index descending, then depth descending
    hits.sort_by(|a, b| {
        b.z_index
            .cmp(&a.z_index)
            .then_with(|| b.depth.cmp(&a.depth))
    });

    let best = hits.remove(0);
    log::debug!("hit_test: best hit = {} (z_index={}, depth={})", best.id, best.z_index, best.depth);
    Some(best.id)
}

/// Collect all elements matching the predicate at the given coordinates.
fn collect_hits<F>(
    layout: &LayoutResult,
    element: &Element,
    x: u16,
    y: u16,
    inherited_z: i16,
    depth: u32,
    hits: &mut Vec<Hit>,
    predicate: F,
) where
    F: Fn(&Element) -> bool + Copy,
{
    let Some(rect) = layout.get(&element.id) else {
        return;
    };

    // Effective z-index: use element's z_index if set, otherwise inherit
    let effective_z = if element.z_index != 0 {
        element.z_index
    } else {
        inherited_z
    };

    let contains_point = rect.contains(x, y);

    // ALWAYS recurse into ALL children to find absolute descendants
    // This is critical: a flow child that doesn't contain the point may have
    // absolute grandchildren that DO contain the point (e.g., dropdown in a height=1 container)
    if let Content::Children(children) = &element.content {
        for child in children {
            collect_hits(layout, child, x, y, effective_z, depth + 1, hits, predicate);
        }
    }

    // Only add this element if it contains the point and matches predicate
    if contains_point && predicate(element) {
        hits.push(Hit {
            id: element.id.clone(),
            z_index: effective_z,
            depth,
        });
    }
}
