//! List component rendering.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style as RatatuiStyle;

use crate::components::list::AnyList;
use crate::components::scrollbar::render_vertical_scrollbar;
use crate::node::Layout;
use crate::runtime::hit_test::HitTestMap;
use crate::runtime::render::RenderNodeFn;
use crate::runtime::render::layout::{apply_border, apply_padding};
use crate::theme::Theme;

/// Render a list component.
#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    id: &str,
    style: RatatuiStyle,
    layout: &Layout,
    component: &dyn AnyList,
    area: Rect,
    hit_map: &mut HitTestMap,
    theme: &dyn Theme,
    focused_id: Option<&str>,
    render_node: RenderNodeFn,
) {
    use ratatui::widgets::Block;

    // Apply border and get inner area
    let (inner_area, block) = apply_border(area, &layout.border, style);
    if let Some(block) = block {
        frame.render_widget(block, area);
    } else if style.bg.is_some() {
        // Fill background if no border but has background
        let bg_block = Block::default().style(style);
        frame.render_widget(bg_block, area);
    }

    // Apply padding
    let padded_area = apply_padding(inner_area, layout.padding);

    if padded_area.width == 0 || padded_area.height == 0 {
        return;
    }

    // Determine if we need a scrollbar
    let needs_scrollbar = component.needs_vertical_scrollbar();
    // Scrollbar takes 1 cell, plus 1 cell padding between content and scrollbar
    let scrollbar_reserved = if needs_scrollbar { 2u16 } else { 0u16 };

    // Content area excludes scrollbar and padding
    let content_area = Rect {
        x: padded_area.x,
        y: padded_area.y,
        width: padded_area.width.saturating_sub(scrollbar_reserved),
        height: padded_area.height,
    };

    if content_area.width == 0 || content_area.height == 0 {
        return;
    }

    // Update component's viewport height
    component.set_viewport_height(content_area.height);

    // Get visible range
    let visible_range = component.visible_range();
    let item_height = component.item_height();
    let scroll_offset = component.scroll_offset();

    // Calculate offset for first visible item
    let first_item_y = (visible_range.start as u16 * item_height).saturating_sub(scroll_offset);

    // Render visible items
    for (i, index) in visible_range.enumerate() {
        let item_y = content_area.y + first_item_y + (i as u16 * item_height);

        // Skip if outside viewport
        if item_y >= content_area.y + content_area.height {
            break;
        }

        let item_area = Rect {
            x: content_area.x,
            y: item_y,
            width: content_area.width,
            height: item_height.min(content_area.y + content_area.height - item_y),
        };

        // Render the item
        if let Some(item_node) = component.render_item(index) {
            render_node(frame, &item_node, item_area, hit_map, theme, focused_id);
        }
    }

    // Render vertical scrollbar if needed
    if needs_scrollbar {
        let scrollbar_area = Rect {
            x: padded_area.x + padded_area.width - 1,
            y: padded_area.y,
            width: 1,
            height: padded_area.height,
        };

        let config = component.scrollbar_config();
        let v_geom = render_vertical_scrollbar(
            frame.buffer_mut(),
            scrollbar_area,
            scroll_offset,
            component.total_height(),
            content_area.height,
            &config,
            theme,
        );
        component.set_vertical_scrollbar(v_geom);
    } else {
        component.set_vertical_scrollbar(None);
    }

    // Register hit box for the content area (where items are clickable)
    // We use padded_area so click coordinates are relative to item positions
    if !id.is_empty() {
        hit_map.register(id.to_string(), padded_area, true);
    }
}
