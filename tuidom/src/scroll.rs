use std::collections::HashMap;

use crate::element::{Content, Element};
use crate::event::{Event, Key};
use crate::layout::{LayoutResult, Rect};
use crate::types::Overflow;

/// Scroll offset for a scrollable element.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScrollOffset {
    pub x: u16,
    pub y: u16,
}

impl ScrollOffset {
    pub fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }
}

/// Tracks scroll offsets for scrollable elements.
/// Similar to FocusState, this is user-managed state that persists across frames.
#[derive(Debug, Default)]
pub struct ScrollState {
    offsets: HashMap<String, ScrollOffset>,
    /// Content sizes computed during layout (element_id -> (content_width, content_height))
    content_sizes: HashMap<String, (u16, u16)>,
    /// Last known mouse position for keyboard scroll fallback
    last_mouse_pos: Option<(u16, u16)>,
}

impl ScrollState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the scroll offset for an element.
    pub fn get(&self, id: &str) -> ScrollOffset {
        self.offsets.get(id).copied().unwrap_or_default()
    }

    /// Set the scroll offset for an element, clamping to valid range.
    pub fn set(&mut self, id: &str, x: u16, y: u16) {
        self.offsets.insert(id.to_string(), ScrollOffset::new(x, y));
    }

    /// Scroll an element by a delta amount.
    /// Returns true if the scroll offset changed.
    pub fn scroll_by(&mut self, id: &str, dx: i16, dy: i16) -> bool {
        let current = self.get(id);
        let new_x = (current.x as i32 + dx as i32).max(0) as u16;
        let new_y = (current.y as i32 + dy as i32).max(0) as u16;

        if new_x != current.x || new_y != current.y {
            self.offsets
                .insert(id.to_string(), ScrollOffset::new(new_x, new_y));
            true
        } else {
            false
        }
    }

    /// Scroll up by one viewport height.
    /// Returns true if scroll position changed.
    pub fn page_up(&mut self, id: &str, layout: &LayoutResult) -> bool {
        let Some((_, viewport_height)) = layout.viewport_size(id) else {
            return false;
        };
        let Some((_, content_height)) = layout.content_size(id) else {
            return false;
        };

        let current = self.get(id);
        let max_y = content_height.saturating_sub(viewport_height);
        let new_y = current.y.saturating_sub(viewport_height).min(max_y);

        if new_y != current.y {
            self.offsets
                .insert(id.to_string(), ScrollOffset::new(current.x, new_y));
            true
        } else {
            false
        }
    }

    /// Scroll down by one viewport height.
    /// Returns true if scroll position changed.
    pub fn page_down(&mut self, id: &str, layout: &LayoutResult) -> bool {
        let Some((_, viewport_height)) = layout.viewport_size(id) else {
            return false;
        };
        let Some((_, content_height)) = layout.content_size(id) else {
            return false;
        };

        let current = self.get(id);
        let max_y = content_height.saturating_sub(viewport_height);
        let new_y = current.y.saturating_add(viewport_height).min(max_y);

        if new_y != current.y {
            self.offsets
                .insert(id.to_string(), ScrollOffset::new(current.x, new_y));
            true
        } else {
            false
        }
    }

    /// Scroll to the top.
    /// Returns true if scroll position changed.
    pub fn scroll_home(&mut self, id: &str) -> bool {
        let current = self.get(id);
        if current.y != 0 {
            self.offsets
                .insert(id.to_string(), ScrollOffset::new(current.x, 0));
            true
        } else {
            false
        }
    }

    /// Scroll to the bottom.
    /// Returns true if scroll position changed.
    pub fn scroll_end(&mut self, id: &str, layout: &LayoutResult) -> bool {
        let Some((_, viewport_height)) = layout.viewport_size(id) else {
            return false;
        };
        let Some((_, content_height)) = layout.content_size(id) else {
            return false;
        };

        let current = self.get(id);
        let max_y = content_height.saturating_sub(viewport_height);

        if current.y != max_y {
            self.offsets
                .insert(id.to_string(), ScrollOffset::new(current.x, max_y));
            true
        } else {
            false
        }
    }

    /// Clamp scroll offsets to valid ranges based on content and container sizes.
    /// Call this after layout to ensure offsets are within bounds.
    pub fn clamp(&mut self, id: &str, container: Rect, content_width: u16, content_height: u16) {
        let max_x = content_width.saturating_sub(container.width);
        let max_y = content_height.saturating_sub(container.height);

        if let Some(offset) = self.offsets.get_mut(id) {
            offset.x = offset.x.min(max_x);
            offset.y = offset.y.min(max_y);
        }

        self.content_sizes
            .insert(id.to_string(), (content_width, content_height));
    }

    /// Get the content size for a scrollable element (set during layout).
    pub fn content_size(&self, id: &str) -> Option<(u16, u16)> {
        self.content_sizes.get(id).copied()
    }

    /// Process events and update scroll offsets.
    /// Returns events that were consumed (scroll events on scrollable elements).
    pub fn process_events(
        &mut self,
        events: &[Event],
        root: &Element,
        layout: &LayoutResult,
    ) -> Vec<Event> {
        let mut consumed = Vec::new();

        for event in events {
            match event {
                // Track mouse position for keyboard scroll fallback
                Event::MouseMove { x, y } => {
                    self.last_mouse_pos = Some((*x, *y));
                }

                Event::Scroll {
                    target: _,
                    delta_x,
                    delta_y,
                    x,
                    y,
                } => {
                    // Update last mouse position from scroll events too
                    self.last_mouse_pos = Some((*x, *y));

                    // Find the scrollable element at this position
                    if let Some(scrollable_id) = find_scrollable_at(root, layout, *x, *y) {
                        // Get content and viewport sizes from layout (computed during layout pass)
                        let Some((content_width, content_height)) =
                            layout.content_size(&scrollable_id)
                        else {
                            continue;
                        };
                        let Some((inner_width, inner_height)) =
                            layout.viewport_size(&scrollable_id)
                        else {
                            continue;
                        };

                        // Check if content actually overflows
                        let can_scroll_vertical = content_height > inner_height;
                        let can_scroll_horizontal = content_width > inner_width;

                        let current = self.get(&scrollable_id);
                        let mut new_x = current.x;
                        let mut new_y = current.y;

                        // Handle vertical scrolling
                        if *delta_y != 0 && can_scroll_vertical {
                            let max_scroll_y = content_height.saturating_sub(inner_height);
                            new_y = (current.y as i32 + *delta_y as i32)
                                .clamp(0, max_scroll_y as i32)
                                as u16;
                        }

                        // Handle horizontal scrolling
                        if *delta_x != 0 && can_scroll_horizontal {
                            let max_scroll_x = content_width.saturating_sub(inner_width);
                            new_x = (current.x as i32 + *delta_x as i32)
                                .clamp(0, max_scroll_x as i32)
                                as u16;
                        }

                        if new_x != current.x || new_y != current.y {
                            self.offsets
                                .insert(scrollable_id.clone(), ScrollOffset::new(new_x, new_y));
                            consumed.push(event.clone());
                        }

                        // Store content size for reference
                        self.content_sizes
                            .insert(scrollable_id, (content_width, content_height));
                    }
                }

                Event::Key { target, key, .. } => {
                    // Only handle scroll keys
                    if !matches!(key, Key::PageUp | Key::PageDown | Key::Home | Key::End) {
                        continue;
                    }

                    // Find scrollable element: from focused element's ancestor, or under mouse
                    let scrollable_id = match target {
                        Some(target_id) => find_scrollable_ancestor(root, target_id),
                        None => self
                            .last_mouse_pos
                            .and_then(|(x, y)| find_scrollable_at(root, layout, x, y)),
                    };

                    let Some(scrollable_id) = scrollable_id else {
                        continue;
                    };

                    let scrolled = match key {
                        Key::PageUp => self.page_up(&scrollable_id, layout),
                        Key::PageDown => self.page_down(&scrollable_id, layout),
                        Key::Home => self.scroll_home(&scrollable_id),
                        Key::End => self.scroll_end(&scrollable_id, layout),
                        _ => false,
                    };

                    if scrolled {
                        consumed.push(event.clone());
                    }
                }

                _ => {}
            }
        }

        consumed
    }
}

/// Find the innermost scrollable element at the given coordinates.
fn find_scrollable_at(root: &Element, layout: &LayoutResult, x: u16, y: u16) -> Option<String> {
    find_scrollable_recursive(root, layout, x, y)
}

/// Find the nearest scrollable ancestor of a target element (or the element itself if scrollable).
/// Returns None if no scrollable ancestor exists.
fn find_scrollable_ancestor(root: &Element, target_id: &str) -> Option<String> {
    find_scrollable_ancestor_recursive(root, target_id, None)
}

fn find_scrollable_ancestor_recursive(
    element: &Element,
    target_id: &str,
    current_scrollable: Option<&str>,
) -> Option<String> {
    // Update current scrollable ancestor if this element is scrollable
    let scrollable = if element.overflow == Overflow::Scroll || element.overflow == Overflow::Auto {
        Some(element.id.as_str())
    } else {
        current_scrollable
    };

    // If this is the target, return the current scrollable ancestor
    if element.id == target_id {
        return scrollable.map(|s| s.to_string());
    }

    // Search children
    if let Content::Children(children) = &element.content {
        for child in children {
            if let Some(result) = find_scrollable_ancestor_recursive(child, target_id, scrollable) {
                return Some(result);
            }
        }
    }

    None
}

fn find_scrollable_recursive(
    element: &Element,
    layout: &LayoutResult,
    x: u16,
    y: u16,
) -> Option<String> {
    let rect = layout.get(&element.id)?;

    // Check if point is within bounds
    if x < rect.x || x >= rect.right() || y < rect.y || y >= rect.bottom() {
        return None;
    }

    // Check children first (innermost takes priority)
    if let Content::Children(children) = &element.content {
        for child in children.iter().rev() {
            if let Some(id) = find_scrollable_recursive(child, layout, x, y) {
                return Some(id);
            }
        }
    }

    // Check if this element is scrollable
    if element.overflow == Overflow::Scroll || element.overflow == Overflow::Auto {
        return Some(element.id.clone());
    }

    None
}

/// Collect all scrollable element IDs.
pub fn collect_scrollable(element: &Element) -> Vec<String> {
    let mut result = Vec::new();
    collect_scrollable_recursive(element, &mut result);
    result
}

fn collect_scrollable_recursive(element: &Element, result: &mut Vec<String>) {
    if element.overflow == Overflow::Scroll || element.overflow == Overflow::Auto {
        result.push(element.id.clone());
    }
    if let Content::Children(children) = &element.content {
        for child in children {
            collect_scrollable_recursive(child, result);
        }
    }
}
