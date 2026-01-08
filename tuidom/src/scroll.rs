use std::collections::HashMap;

use crossterm::event::{Event as CrosstermEvent, MouseButton, MouseEventKind};

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

/// Scrollbar geometry and calculations.
/// Used for hit testing, rendering, and scroll position conversion.
#[derive(Debug, Clone, Copy)]
pub struct ScrollbarGeometry {
    /// Start position of the track (screen coordinate)
    pub track_start: u16,
    /// Length of the track in pixels
    pub track_length: u16,
    /// Size of the thumb in pixels
    pub thumb_size: u16,
    /// Maximum scroll value (content_size - viewport_size)
    pub max_scroll: u16,
}

impl ScrollbarGeometry {
    /// Create geometry for a scrollbar.
    /// Returns None if no scrollbar is needed (content fits in viewport).
    pub fn new(
        track_start: u16,
        track_length: u16,
        viewport_size: u16,
        content_size: u16,
    ) -> Option<Self> {
        if content_size <= viewport_size || track_length == 0 {
            return None;
        }

        let thumb_size = ((viewport_size as u32 * track_length as u32) / content_size as u32)
            .max(1)
            .min(track_length as u32) as u16;
        let max_scroll = content_size.saturating_sub(viewport_size);

        Some(Self {
            track_start,
            track_length,
            thumb_size,
            max_scroll,
        })
    }

    /// The range the thumb can move within the track.
    pub fn scroll_range(&self) -> u16 {
        self.track_length.saturating_sub(self.thumb_size)
    }

    /// Convert scroll offset to thumb position (with rounding).
    pub fn thumb_pos(&self, scroll_offset: u16) -> u16 {
        let scroll_range = self.scroll_range();
        if self.max_scroll == 0 || scroll_range == 0 {
            return 0;
        }
        ((scroll_offset as u32 * scroll_range as u32 + self.max_scroll as u32 / 2)
            / self.max_scroll as u32)
            .min(scroll_range as u32) as u16
    }

    /// Convert thumb position in track to scroll offset (with rounding).
    pub fn scroll_pos(&self, thumb_pos_in_track: u16) -> u16 {
        let scroll_range = self.scroll_range();
        if scroll_range == 0 {
            return 0;
        }
        ((thumb_pos_in_track as u32 * self.max_scroll as u32 + scroll_range as u32 / 2)
            / scroll_range as u32)
            .min(self.max_scroll as u32) as u16
    }

    /// Screen coordinate where thumb starts.
    pub fn thumb_screen_start(&self, scroll_offset: u16) -> u16 {
        self.track_start + self.thumb_pos(scroll_offset)
    }

    /// Check if a screen coordinate is on the thumb.
    /// Returns Some(offset_within_thumb) if hit, None otherwise.
    pub fn hit_test_thumb(&self, screen_pos: u16, scroll_offset: u16) -> Option<u16> {
        let thumb_start = self.thumb_screen_start(scroll_offset);
        let thumb_end = thumb_start + self.thumb_size;
        if screen_pos >= thumb_start && screen_pos < thumb_end {
            Some(screen_pos - thumb_start)
        } else {
            None
        }
    }

    /// Check if a screen coordinate is on the track.
    pub fn hit_test_track(&self, screen_pos: u16) -> bool {
        screen_pos >= self.track_start && screen_pos < self.track_start + self.track_length
    }

    /// Calculate grab offset for a track click (proportional to click position).
    pub fn track_click_offset(&self, click_screen_pos: u16) -> u16 {
        let relative_pos = click_screen_pos.saturating_sub(self.track_start);
        let track_ratio = relative_pos as f32 / self.track_length.max(1) as f32;
        let offset = (track_ratio * self.thumb_size as f32).round() as u16;
        offset.min(self.thumb_size.saturating_sub(1))
    }

    /// Calculate scroll position from mouse position during drag.
    /// `screen_pos` is the current mouse position, `grab_offset` is where in the thumb we grabbed.
    pub fn scroll_from_drag(&self, screen_pos: u16, grab_offset: u16) -> u16 {
        let thumb_start = screen_pos.saturating_sub(grab_offset);
        let scroll_range = self.scroll_range();
        let clamped = thumb_start.clamp(self.track_start, self.track_start + scroll_range);
        let thumb_pos_in_track = clamped.saturating_sub(self.track_start);
        self.scroll_pos(thumb_pos_in_track)
    }
}

/// Result of a scrollbar hit test.
#[derive(Debug, Clone, Copy)]
pub struct ScrollbarHit {
    /// The scrollbar geometry
    pub geom: ScrollbarGeometry,
    /// True if vertical scrollbar, false if horizontal
    pub is_vertical: bool,
    /// If clicked on thumb, the offset within the thumb. None if clicked on track.
    pub thumb_offset: Option<u16>,
}

/// Tracks scrollbar drag state.
#[derive(Debug, Clone)]
struct ScrollbarDrag {
    /// Element ID of the scrollable container being dragged
    element_id: String,
    /// True if dragging vertical scrollbar, false for horizontal
    is_vertical: bool,
    /// Offset within thumb where drag started (for smooth dragging)
    grab_offset: u16,
    /// Scrollbar geometry at drag start
    geom: ScrollbarGeometry,
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
    /// Current scrollbar drag state
    drag: Option<ScrollbarDrag>,
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
                    log::debug!("[scroll] Event::Scroll at ({}, {}) delta=({}, {})", x, y, delta_x, delta_y);
                    // Update last mouse position from scroll events too
                    self.last_mouse_pos = Some((*x, *y));

                    // Find the scrollable element at this position
                    let scrollable = find_scrollable_at(root, layout, *x, *y);
                    log::debug!("[scroll] find_scrollable_at returned: {:?}", scrollable);
                    if let Some(scrollable_id) = scrollable {
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

                        log::debug!("[scroll] can_v={} can_h={} current=({},{}) new=({},{})",
                            can_scroll_vertical, can_scroll_horizontal, current.x, current.y, new_x, new_y);
                        if new_x != current.x || new_y != current.y {
                            log::debug!("[scroll] Updating scroll for {} to ({}, {})", scrollable_id, new_x, new_y);
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

    /// Process raw crossterm events for scrollbar dragging.
    /// This should be called BEFORE FocusState::process_events so that
    /// scrollbar interactions don't propagate as click events.
    /// Returns events that were NOT consumed by scrollbar interaction.
    pub fn process_raw_events(
        &mut self,
        events: &[CrosstermEvent],
        root: &Element,
        layout: &LayoutResult,
    ) -> Vec<CrosstermEvent> {
        let mut unconsumed = Vec::new();

        for event in events {
            match event {
                CrosstermEvent::Mouse(mouse) => {
                    let x = mouse.column;
                    let y = mouse.row;

                    match mouse.kind {
                        MouseEventKind::Down(MouseButton::Left) => {
                            // Check if click is on any scrollbar
                            if let Some((element_id, hit)) = self.check_any_scrollbar_hit(root, layout, x, y) {
                                log::debug!("[scroll-drag] MouseDown at ({}, {}), hit={:?}", x, y, hit);

                                // Calculate grab offset
                                let click_pos = if hit.is_vertical { y } else { x };
                                let grab_offset = hit.thumb_offset.unwrap_or_else(|| {
                                    // Clicked on track - calculate proportional offset
                                    hit.geom.track_click_offset(click_pos)
                                });

                                self.drag = Some(ScrollbarDrag {
                                    element_id: element_id.clone(),
                                    is_vertical: hit.is_vertical,
                                    grab_offset,
                                    geom: hit.geom,
                                });

                                // If clicked on track (not thumb), immediately scroll to that position
                                if hit.thumb_offset.is_none() {
                                    let scroll_pos = hit.geom.scroll_from_drag(click_pos, grab_offset);
                                    let current = self.get(&element_id);
                                    if hit.is_vertical {
                                        self.set(&element_id, current.x, scroll_pos);
                                    } else {
                                        self.set(&element_id, scroll_pos, current.y);
                                    }
                                }
                                // Consume this event - don't let it become a click
                                continue;
                            }
                        }

                        MouseEventKind::Up(MouseButton::Left) => {
                            if self.drag.is_some() {
                                self.drag = None;
                                // Consume this event
                                continue;
                            }
                        }

                        MouseEventKind::Drag(MouseButton::Left) => {
                            if let Some(drag) = self.drag.clone() {
                                let mouse_pos = if drag.is_vertical { y } else { x };
                                let scroll_pos = drag.geom.scroll_from_drag(mouse_pos, drag.grab_offset);
                                let current = self.get(&drag.element_id);
                                if drag.is_vertical {
                                    self.set(&drag.element_id, current.x, scroll_pos);
                                } else {
                                    self.set(&drag.element_id, scroll_pos, current.y);
                                }
                                // Consume this event
                                continue;
                            }
                        }

                        _ => {}
                    }
                }
                _ => {}
            }

            unconsumed.push(event.clone());
        }

        unconsumed
    }

    /// Check if a point hits any scrollbar in the element tree.
    fn check_any_scrollbar_hit(
        &self,
        root: &Element,
        layout: &LayoutResult,
        x: u16,
        y: u16,
    ) -> Option<(String, ScrollbarHit)> {
        let scrollables = collect_scrollable_with_elements(root);
        log::debug!("[scroll-drag] scrollables={:?}", scrollables.iter().map(|(id, _)| id).collect::<Vec<_>>());
        for (id, element) in scrollables {
            let rect = layout.get(&id);
            let content = layout.content_size(&id);
            let viewport = layout.viewport_size(&id);
            log::debug!("[scroll-drag] checking {} rect={:?} content={:?} viewport={:?}", id, rect, content, viewport);
            if let Some(hit) = check_scrollbar_hit(&id, x, y, layout, self, element) {
                return Some((id, hit));
            }
        }
        None
    }
}

/// Collect all scrollable elements with their Element references.
fn collect_scrollable_with_elements(element: &Element) -> Vec<(String, &Element)> {
    let mut result = Vec::new();
    collect_scrollable_with_elements_recursive(element, &mut result);
    result
}

fn collect_scrollable_with_elements_recursive<'a>(element: &'a Element, result: &mut Vec<(String, &'a Element)>) {
    if element.overflow == Overflow::Scroll || element.overflow == Overflow::Auto {
        result.push((element.id.clone(), element));
    }
    if let Content::Children(children) = &element.content {
        for child in children {
            collect_scrollable_with_elements_recursive(child, result);
        }
    }
}

/// Check if a click is on a scrollbar.
fn check_scrollbar_hit(
    id: &str,
    x: u16,
    y: u16,
    layout: &LayoutResult,
    scroll: &ScrollState,
    element: &Element,
) -> Option<ScrollbarHit> {
    let rect = layout.get(id)?;
    let (content_width, content_height) = layout.content_size(id)?;
    let (inner_width, inner_height) = layout.viewport_size(id)?;
    let current = scroll.get(id);

    let border_size = if element.style.border == crate::types::Border::None {
        0
    } else {
        1
    };

    // Check vertical scrollbar (right edge)
    let scrollbar_x = rect.right().saturating_sub(1).saturating_sub(border_size);
    let v_track_start = rect.y + border_size;
    let v_track_end = rect.bottom().saturating_sub(border_size);
    let v_track_length = v_track_end.saturating_sub(v_track_start);

    if x == scrollbar_x && y >= v_track_start && y < v_track_end {
        if let Some(geom) = ScrollbarGeometry::new(v_track_start, v_track_length, inner_height, content_height) {
            let thumb_offset = geom.hit_test_thumb(y, current.y);
            return Some(ScrollbarHit {
                geom,
                is_vertical: true,
                thumb_offset,
            });
        }
    }

    // Check horizontal scrollbar (bottom edge)
    let scrollbar_y = rect.bottom().saturating_sub(1).saturating_sub(border_size);
    let h_track_start = rect.x + border_size;
    let mut h_track_end = rect.right().saturating_sub(border_size);
    // Reduce for vertical scrollbar if present
    if content_height > inner_height {
        h_track_end = h_track_end.saturating_sub(1);
    }
    let h_track_length = h_track_end.saturating_sub(h_track_start);

    if y == scrollbar_y && x >= h_track_start && x < h_track_end {
        if let Some(geom) = ScrollbarGeometry::new(h_track_start, h_track_length, inner_width, content_width) {
            let thumb_offset = geom.hit_test_thumb(x, current.x);
            return Some(ScrollbarHit {
                geom,
                is_vertical: false,
                thumb_offset,
            });
        }
    }

    None
}

/// Find the innermost scrollable element at the given coordinates.
/// This traverses the entire tree to find all scrollables, then checks which ones
/// contain the mouse position. Returns the deepest/innermost one.
fn find_scrollable_at(root: &Element, layout: &LayoutResult, x: u16, y: u16) -> Option<String> {
    // Collect all scrollable elements (this traverses the entire tree including absolute children)
    let scrollables = collect_scrollable(root);
    log::debug!("[scroll] find_scrollable_at ({}, {}) scrollables={:?}", x, y, scrollables);

    // Find all scrollables that contain the mouse position
    // Later elements in the list are deeper in the tree, so we iterate in reverse
    // to find the deepest/innermost one first
    for id in scrollables.iter().rev() {
        if let Some(rect) = layout.get(id) {
            let contains = x >= rect.x && x < rect.right() && y >= rect.y && y < rect.bottom();
            log::debug!("[scroll] checking {} rect={:?} contains={}", id, rect, contains);
            if contains {
                return Some(id.clone());
            }
        }
    }

    None
}

/// Find the nearest scrollable ancestor of a target element (or the element itself if scrollable).
/// Returns None if no scrollable ancestor exists.
pub fn find_scrollable_ancestor(root: &Element, target_id: &str) -> Option<String> {
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
