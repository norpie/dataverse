//! Shared types for component rendering.

use ratatui::Frame;
use ratatui::layout::Rect;

use crate::node::Node;
use crate::runtime::hit_test::HitTestMap;
use crate::theme::Theme;

/// Function pointer for recursive node rendering.
///
/// Components use this to render their child nodes (e.g., list items, table cells)
/// without depending on the full render module.
pub type RenderNodeFn = fn(&mut Frame, &Node, Rect, &mut HitTestMap, &dyn Theme, Option<&str>);
