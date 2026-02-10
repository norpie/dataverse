//! Virtual scroller - common virtualization logic for scrollable widgets.
//!
//! This module provides shared infrastructure for virtualized scrolling,
//! used by List, Table, Tree, Select, and Autocomplete widgets.

use std::ops::Range;
use std::sync::Arc;

use tuidom::ScrollAction;

use super::scroll::{ScrollRequest, ScrollState, ScrollableWidgetState, Scrollbar};
use crate::HandlerRegistry;
use crate::state::State;

// =============================================================================
// VirtualScroller
// =============================================================================

/// Common virtualization logic for vertical scrolling.
///
/// Manages cumulative height caching for O(1) position lookups and O(log n)
/// offset-to-index queries. Used internally by List, Table, Tree, and dropdown
/// widgets.
///
/// # Example
///
/// ```ignore
/// let mut scroller = VirtualScroller::new();
/// scroller.rebuild(items.iter().map(|item| item.height()));
///
/// let range = scroller.visible_range(&scroll_state);
/// for i in range {
///     // Render item at index i
/// }
/// ```
#[derive(Clone, Debug, Default)]
pub struct VirtualScroller {
    /// Cached cumulative heights for O(1) position lookups.
    /// `cumulative[i]` = total height of items `0..i`
    /// `cumulative[0]` = 0, `cumulative[n]` = total content height
    /// Length = `item_count + 1`
    cumulative_heights: Vec<u16>,
}

impl VirtualScroller {
    /// Create a new empty virtual scroller.
    pub fn new() -> Self {
        Self {
            cumulative_heights: vec![0],
        }
    }

    /// Rebuild cumulative heights from item heights.
    ///
    /// Call this whenever items change. Returns the total content height.
    pub fn rebuild(&mut self, heights: impl Iterator<Item = u16>) -> u16 {
        self.cumulative_heights.clear();
        self.cumulative_heights.push(0);

        let mut total: u16 = 0;
        for height in heights {
            total = total.saturating_add(height);
            self.cumulative_heights.push(total);
        }

        total
    }

    /// Get the number of items.
    pub fn len(&self) -> usize {
        self.cumulative_heights.len().saturating_sub(1)
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get Y offset for item at index. O(1).
    pub fn item_y_offset(&self, index: usize) -> u16 {
        self.cumulative_heights.get(index).copied().unwrap_or(0)
    }

    /// Get total content height. O(1).
    pub fn total_height(&self) -> u16 {
        self.cumulative_heights.last().copied().unwrap_or(0)
    }

    /// Find item index at given Y offset. O(log n) binary search.
    pub fn item_at_offset(&self, y: u16) -> usize {
        self.cumulative_heights
            .partition_point(|&h| h <= y)
            .saturating_sub(1)
    }

    /// Get height of item at index. O(1).
    pub fn item_height(&self, index: usize) -> u16 {
        if index + 1 < self.cumulative_heights.len() {
            self.cumulative_heights[index + 1] - self.cumulative_heights[index]
        } else {
            1 // Default
        }
    }

    /// Calculate the range of visible items given current scroll state.
    ///
    /// Returns a range of indices into the item list.
    pub fn visible_range(&self, scroll: &ScrollState) -> Range<usize> {
        if self.is_empty() {
            return 0..0;
        }

        let scroll_y = scroll.offset;
        let viewport = scroll.viewport;

        // If viewport is 0 (first frame before layout), use a safe maximum
        let effective_viewport = if viewport == 0 { 200 } else { viewport };

        // O(log n) binary search to find first visible item
        let first_visible = self.item_at_offset(scroll_y);

        // Calculate exactly how many items fit in viewport
        let mut end_idx = first_visible;
        let mut total_height: u16 = 0;
        while end_idx < self.len() && total_height < effective_viewport {
            total_height += self.item_height(end_idx);
            end_idx += 1;
        }

        first_visible..end_idx
    }

    /// Process a scroll-into-view request for a specific item index.
    ///
    /// Returns the new scroll offset if scrolling is needed, or None if
    /// the item is already visible.
    pub fn scroll_into_view(&self, index: usize, scroll: &ScrollState) -> Option<u16> {
        let y = self.item_y_offset(index);
        let item_h = self.item_height(index);
        let viewport = scroll.viewport;
        let offset = scroll.offset;

        if y < offset {
            // Item above viewport - scroll up to show it
            Some(y)
        } else if y + item_h > offset + viewport {
            // Item below viewport - scroll down to show it
            Some((y + item_h).saturating_sub(viewport))
        } else {
            // Already visible
            None
        }
    }

    /// Get the index of the first visible item based on current scroll offset.
    pub fn first_visible_index(&self, scroll: &ScrollState) -> usize {
        self.item_at_offset(scroll.offset)
    }
}

// =============================================================================
// Scroll Handler Helpers
// =============================================================================

/// Configuration for registering scroll handlers.
pub struct ScrollHandlerConfig<F> {
    /// Element ID to register handlers on.
    pub element_id: String,
    /// Widget ID prefix for generating item IDs.
    pub widget_id: String,
    /// Function to get the item ID suffix from an index.
    pub get_item_id_suffix: F,
}

/// Register standard vertical scroll handlers on an element.
///
/// This registers:
/// - `on_scroll` handler for mouse wheel and Page/Home/End navigation
/// - `on_layout` handler for viewport discovery
///
/// The `get_item_id` function should return the full element ID for focusing
/// after scroll actions like PageUp/PageDown/Home/End.
pub fn register_scroll_handlers<S, F>(
    config: ScrollHandlerConfig<F>,
    state: &State<S>,
    scroller_getter: impl Fn(&S) -> &VirtualScroller + Send + Sync + Clone + 'static,
    registry: &HandlerRegistry,
    user_on_scroll: Option<Arc<dyn Fn(&crate::HandlerContext) + Send + Sync>>,
) where
    S: ScrollableWidgetState,
    F: Fn(&S, usize) -> String + Send + Sync + Clone + 'static,
{
    let element_id = config.element_id;
    let widget_id = config.widget_id;
    let get_item_id_suffix = config.get_item_id_suffix;

    // on_scroll: handle mouse wheel and keyboard scroll actions
    {
        let state_clone = state.clone();
        let widget_id_clone = widget_id.clone();
        let scroller_getter = scroller_getter.clone();
        let get_item_id_suffix = get_item_id_suffix.clone();
        let user_on_scroll = user_on_scroll.clone();

        registry.register(
            &element_id,
            "on_scroll",
            Arc::new(move |hx| {
                let mut scrolled = false;

                // Mouse wheel: delta
                if let Some((_, delta_y)) = hx.event().scroll_delta() {
                    log::debug!(
                        "[VirtualScroller::on_scroll] scroll_delta delta_y={}",
                        delta_y
                    );
                    state_clone.update(|s| {
                        s.scroll_mut().scroll_by(delta_y);
                    });
                    scrolled = true;
                }

                // Page Up/Down/Home/End: scroll action from keyboard
                if let Some(action) = hx.event().scroll_action() {
                    log::debug!("[VirtualScroller::on_scroll] scroll_action {:?}", action);

                    // Apply the scroll action immediately
                    state_clone.update(|s| {
                        let scroll_request = match action {
                            ScrollAction::PageUp => ScrollRequest::PageUp,
                            ScrollAction::PageDown => ScrollRequest::PageDown,
                            ScrollAction::Home => ScrollRequest::Home,
                            ScrollAction::End => ScrollRequest::End,
                        };
                        s.scroll_mut().apply_request(scroll_request);
                    });
                    scrolled = true;

                    // Calculate target based on NEW scroll position and focus it
                    let current = state_clone.get();
                    let scroller = scroller_getter(&current);
                    if scroller.is_empty() {
                        return;
                    }

                    let target_index = match action {
                        ScrollAction::Home => 0,
                        ScrollAction::End => scroller.len() - 1,
                        ScrollAction::PageUp => {
                            // Focus first visible item after scroll
                            scroller.first_visible_index(current.scroll())
                        }
                        ScrollAction::PageDown => {
                            // Focus last visible item after scroll
                            let first = scroller.first_visible_index(current.scroll());
                            let viewport = current.scroll().viewport as usize;
                            (first + viewport.saturating_sub(1)).min(scroller.len() - 1)
                        }
                    };

                    let item_id_suffix = get_item_id_suffix(&current, target_index);
                    let item_id = format!("{}-{}", widget_id_clone, item_id_suffix);
                    log::debug!(
                        "[VirtualScroller::on_scroll] Focusing item: {} (index {})",
                        item_id,
                        target_index
                    );
                    hx.cx().focus(&item_id);
                }

                // Call user's on_scroll handler with scroll metrics
                if scrolled && let Some(ref handler) = user_on_scroll {
                    let current = state_clone.get();
                    let scroll = current.scroll();
                    let scroll_event = crate::handler_context::EventData::Scroll {
                        offset_x: 0,
                        offset_y: scroll.offset,
                        content_width: 0,
                        content_height: scroll.content_height,
                        viewport_width: 0,
                        viewport_height: scroll.viewport,
                    };
                    let scroll_hx = hx.with_event(scroll_event);
                    handler(&scroll_hx);
                }
            }),
        );
    }
}

/// Register a layout handler for viewport discovery.
///
/// The `subtract_for_horizontal_scrollbar` parameter indicates whether to
/// subtract 1 from the height for a horizontal scrollbar.
pub fn register_layout_handler<S: ScrollableWidgetState>(
    element_id: &str,
    state: &State<S>,
    registry: &HandlerRegistry,
    subtract_for_horizontal_scrollbar: bool,
) {
    let state_clone = state.clone();
    registry.register(
        element_id,
        "on_layout",
        Arc::new(move |hx| {
            if let Some((_, _, _, height)) = hx.event().layout() {
                let viewport_height = if subtract_for_horizontal_scrollbar {
                    height.saturating_sub(1)
                } else {
                    height
                };
                state_clone.update(|s| {
                    s.scroll_mut().set_viewport(viewport_height);
                });
            }
        }),
    );
}

/// Build and return a vertical scrollbar element for the given state.
pub fn build_scrollbar<S: ScrollableWidgetState>(
    scrollbar_id: &str,
    state: &State<S>,
    registry: &HandlerRegistry,
    handlers: &crate::WidgetHandlers,
) -> tuidom::Element {
    Scrollbar::vertical()
        .id(scrollbar_id)
        .state(state)
        .build(registry, handlers)
}
