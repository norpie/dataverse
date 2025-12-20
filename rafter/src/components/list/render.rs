//! List component rendering.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style as RatatuiStyle;

use crate::node::Node;
use crate::runtime::hit_test::HitTestMap;
use crate::theme::Theme;

use super::state::{List, ListItem};

/// Render a list with virtualization.
pub fn render_list<T: ListItem>(
    frame: &mut Frame,
    list: &List<T>,
    area: Rect,
    style: RatatuiStyle,
    hit_map: &mut HitTestMap,
    theme: &dyn Theme,
    focused_id: Option<&str>,
    render_node_fn: fn(
        &mut Frame,
        &Node,
        Rect,
        &mut HitTestMap,
        &dyn Theme,
        Option<&str>,
    ),
) {
    // Fill background if specified
    if style.bg.is_some() {
        let block = ratatui::widgets::Block::default().style(style);
        frame.render_widget(block, area);
    }

    // Update viewport height
    list.set_viewport_height(area.height);

    // Get visible range
    let visible_range = list.visible_range();
    if visible_range.is_empty() {
        return;
    }

    let items = list.items();
    let cursor = list.cursor();
    let item_height = T::HEIGHT;
    let scroll_offset = list.scroll_offset();

    // Render each visible item
    for index in visible_range {
        if let Some(item) = items.get(index) {
            let is_focused = cursor == Some(index);
            let is_selected = list.is_selected(index);

            // Calculate item position
            let item_y = (index as u16 * item_height).saturating_sub(scroll_offset);
            
            // Check if item is within viewport
            if item_y >= area.height {
                continue;
            }

            let item_area = Rect {
                x: area.x,
                y: area.y + item_y,
                width: area.width,
                height: item_height.min(area.height - item_y),
            };

            // Render the item
            let item_node = item.render(is_focused, is_selected);
            render_node_fn(frame, &item_node, item_area, hit_map, theme, focused_id);
        }
    }
}
