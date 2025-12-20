//! Scrollable component state.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use crate::color::StyleColor;

/// Unique identifier for a Scrollable component instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScrollableId(usize);

impl ScrollableId {
    fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl std::fmt::Display for ScrollableId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "__scrollable_{}", self.0)
    }
}

/// Scroll direction configuration.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDirection {
    /// Vertical scrolling only.
    #[default]
    Vertical,
    /// Horizontal scrolling only.
    Horizontal,
    /// Both vertical and horizontal scrolling.
    Both,
}

/// Scrollbar visibility configuration.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarVisibility {
    /// Show scrollbar only when content overflows.
    #[default]
    Auto,
    /// Always show scrollbar.
    Always,
    /// Never show scrollbar.
    Never,
}

/// Scrollbar appearance configuration.
#[derive(Debug, Default, Clone)]
pub struct ScrollbarConfig {
    /// Horizontal scrollbar visibility.
    pub horizontal: ScrollbarVisibility,
    /// Vertical scrollbar visibility.
    pub vertical: ScrollbarVisibility,
    /// Track (background) color.
    pub track_color: Option<StyleColor>,
    /// Handle (draggable part) color.
    pub handle_color: Option<StyleColor>,
}

/// Scrollbar geometry for hit testing.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScrollbarGeometry {
    /// X position of the scrollbar track.
    pub x: u16,
    /// Y position of the scrollbar track.
    pub y: u16,
    /// Width of the scrollbar (1 for vertical, track length for horizontal).
    pub width: u16,
    /// Height of the scrollbar (track length for vertical, 1 for horizontal).
    pub height: u16,
    /// Position of the handle within the track (0-based).
    pub handle_pos: u16,
    /// Size of the handle.
    pub handle_size: u16,
}

impl ScrollbarGeometry {
    /// Check if a point is within the scrollbar track.
    pub fn contains(&self, x: u16, y: u16) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }

    /// Check if a point is on the handle.
    pub fn handle_contains(&self, x: u16, y: u16, vertical: bool) -> bool {
        if !self.contains(x, y) {
            return false;
        }
        if vertical {
            let rel_y = y - self.y;
            rel_y >= self.handle_pos && rel_y < self.handle_pos + self.handle_size
        } else {
            let rel_x = x - self.x;
            rel_x >= self.handle_pos && rel_x < self.handle_pos + self.handle_size
        }
    }

    /// Convert a position on the track to a scroll ratio (0.0 - 1.0).
    /// Centers the handle on the click position.
    pub fn position_to_ratio(&self, x: u16, y: u16, vertical: bool) -> f32 {
        self.position_to_ratio_with_offset(x, y, vertical, self.handle_size / 2)
    }

    /// Convert a position on the track to a scroll ratio, with a custom offset within the handle.
    /// `grab_offset` is where within the handle the user grabbed (0 = top/left edge).
    pub fn position_to_ratio_with_offset(
        &self,
        x: u16,
        y: u16,
        vertical: bool,
        grab_offset: u16,
    ) -> f32 {
        if vertical {
            let track_size = self.height.saturating_sub(self.handle_size);
            if track_size == 0 {
                return 0.0;
            }
            let rel_y = y.saturating_sub(self.y).saturating_sub(grab_offset);
            (rel_y as f32 / track_size as f32).clamp(0.0, 1.0)
        } else {
            let track_size = self.width.saturating_sub(self.handle_size);
            if track_size == 0 {
                return 0.0;
            }
            let rel_x = x.saturating_sub(self.x).saturating_sub(grab_offset);
            (rel_x as f32 / track_size as f32).clamp(0.0, 1.0)
        }
    }
}

use super::events::ScrollbarDrag;

/// Internal state for the Scrollable component.
#[derive(Debug)]
struct ScrollableInner {
    /// Scroll direction configuration.
    direction: ScrollDirection,
    /// Scrollbar configuration.
    scrollbar: ScrollbarConfig,

    /// Horizontal scroll offset.
    offset_x: u16,
    /// Vertical scroll offset.
    offset_y: u16,

    /// Content size (width, height) - updated by renderer.
    content_size: (u16, u16),
    /// Viewport size (width, height) - updated by renderer.
    viewport_size: (u16, u16),

    /// Vertical scrollbar geometry - updated by renderer.
    vertical_scrollbar: Option<ScrollbarGeometry>,
    /// Horizontal scrollbar geometry - updated by renderer.
    horizontal_scrollbar: Option<ScrollbarGeometry>,

    /// Current drag state.
    drag: Option<ScrollbarDrag>,
}

impl Default for ScrollableInner {
    fn default() -> Self {
        Self {
            direction: ScrollDirection::default(),
            scrollbar: ScrollbarConfig::default(),
            offset_x: 0,
            offset_y: 0,
            content_size: (0, 0),
            viewport_size: (0, 0),
            vertical_scrollbar: None,
            horizontal_scrollbar: None,
            drag: None,
        }
    }
}

/// A scrollable container component with reactive state.
///
/// `Scrollable` manages scroll position and provides imperative methods
/// for programmatic scrolling. It supports both vertical and horizontal
/// scrolling with configurable scrollbar visibility.
///
/// # Example
///
/// ```ignore
/// #[app]
/// struct MyApp {
///     content_scroll: Scrollable,
/// }
///
/// #[app_impl]
/// impl MyApp {
///     fn view(&self) -> Node {
///         view! {
///             scrollable(bind: self.content_scroll, direction: vertical) {
///                 column {
///                     for item in &self.items {
///                         text { item.name }
///                     }
///                 }
///             }
///         }
///     }
///
///     #[handler]
///     async fn go_to_top(&self, _cx: &AppContext) {
///         self.content_scroll.scroll_to_top();
///     }
/// }
/// ```
#[derive(Debug)]
pub struct Scrollable {
    /// Unique identifier for this scrollable instance.
    id: ScrollableId,
    /// Internal state.
    inner: Arc<RwLock<ScrollableInner>>,
    /// Dirty flag for re-render.
    dirty: Arc<AtomicBool>,
}

impl Scrollable {
    /// Create a new scrollable with default settings.
    pub fn new() -> Self {
        Self {
            id: ScrollableId::new(),
            inner: Arc::new(RwLock::new(ScrollableInner::default())),
            dirty: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a scrollable with a specific direction.
    pub fn with_direction(direction: ScrollDirection) -> Self {
        Self {
            id: ScrollableId::new(),
            inner: Arc::new(RwLock::new(ScrollableInner {
                direction,
                ..Default::default()
            })),
            dirty: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get the unique ID for this scrollable.
    pub fn id(&self) -> ScrollableId {
        self.id
    }

    /// Get the ID as a string (for node binding).
    pub fn id_string(&self) -> String {
        self.id.to_string()
    }

    // -------------------------------------------------------------------------
    // Configuration
    // -------------------------------------------------------------------------

    /// Get the scroll direction.
    pub fn direction(&self) -> ScrollDirection {
        self.inner
            .read()
            .map(|guard| guard.direction)
            .unwrap_or_default()
    }

    /// Set the scroll direction.
    pub fn set_direction(&self, direction: ScrollDirection) {
        if let Ok(mut guard) = self.inner.write() {
            guard.direction = direction;
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Get the scrollbar configuration.
    pub fn scrollbar_config(&self) -> ScrollbarConfig {
        self.inner
            .read()
            .map(|guard| guard.scrollbar.clone())
            .unwrap_or_default()
    }

    /// Set the scrollbar configuration.
    pub fn set_scrollbar_config(&self, config: ScrollbarConfig) {
        if let Ok(mut guard) = self.inner.write() {
            guard.scrollbar = config;
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    // -------------------------------------------------------------------------
    // Read scroll position
    // -------------------------------------------------------------------------

    /// Get the current scroll offset (x, y).
    pub fn offset(&self) -> (u16, u16) {
        self.inner
            .read()
            .map(|guard| (guard.offset_x, guard.offset_y))
            .unwrap_or((0, 0))
    }

    /// Get the horizontal scroll offset.
    pub fn offset_x(&self) -> u16 {
        self.inner
            .read()
            .map(|guard| guard.offset_x)
            .unwrap_or(0)
    }

    /// Get the vertical scroll offset.
    pub fn offset_y(&self) -> u16 {
        self.inner
            .read()
            .map(|guard| guard.offset_y)
            .unwrap_or(0)
    }

    // -------------------------------------------------------------------------
    // Write scroll position
    // -------------------------------------------------------------------------

    /// Scroll to an absolute position.
    pub fn scroll_to(&self, x: u16, y: u16) {
        if let Ok(mut guard) = self.inner.write() {
            let max_x = guard.content_size.0.saturating_sub(guard.viewport_size.0);
            let max_y = guard.content_size.1.saturating_sub(guard.viewport_size.1);
            guard.offset_x = x.min(max_x);
            guard.offset_y = y.min(max_y);
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    /// Scroll by a relative amount.
    pub fn scroll_by(&self, dx: i16, dy: i16) {
        if let Ok(mut guard) = self.inner.write() {
            let max_x = guard.content_size.0.saturating_sub(guard.viewport_size.0);
            let max_y = guard.content_size.1.saturating_sub(guard.viewport_size.1);

            let new_x = (guard.offset_x as i32 + dx as i32).clamp(0, max_x as i32) as u16;
            let new_y = (guard.offset_y as i32 + dy as i32).clamp(0, max_y as i32) as u16;

            if new_x != guard.offset_x || new_y != guard.offset_y {
                guard.offset_x = new_x;
                guard.offset_y = new_y;
                self.dirty.store(true, Ordering::SeqCst);
            }
        }
    }

    /// Scroll to the top.
    pub fn scroll_to_top(&self) {
        if let Ok(mut guard) = self.inner.write() {
            if guard.offset_y != 0 {
                guard.offset_y = 0;
                self.dirty.store(true, Ordering::SeqCst);
            }
        }
    }

    /// Scroll to the bottom.
    pub fn scroll_to_bottom(&self) {
        if let Ok(mut guard) = self.inner.write() {
            let max_y = guard.content_size.1.saturating_sub(guard.viewport_size.1);
            if guard.offset_y != max_y {
                guard.offset_y = max_y;
                self.dirty.store(true, Ordering::SeqCst);
            }
        }
    }

    /// Scroll to the left edge.
    pub fn scroll_to_left(&self) {
        if let Ok(mut guard) = self.inner.write() {
            if guard.offset_x != 0 {
                guard.offset_x = 0;
                self.dirty.store(true, Ordering::SeqCst);
            }
        }
    }

    /// Scroll to the right edge.
    pub fn scroll_to_right(&self) {
        if let Ok(mut guard) = self.inner.write() {
            let max_x = guard.content_size.0.saturating_sub(guard.viewport_size.0);
            if guard.offset_x != max_x {
                guard.offset_x = max_x;
                self.dirty.store(true, Ordering::SeqCst);
            }
        }
    }

    // -------------------------------------------------------------------------
    // Scroll queries
    // -------------------------------------------------------------------------

    /// Check if scrolling up is possible.
    pub fn can_scroll_up(&self) -> bool {
        self.inner
            .read()
            .map(|guard| guard.offset_y > 0)
            .unwrap_or(false)
    }

    /// Check if scrolling down is possible.
    pub fn can_scroll_down(&self) -> bool {
        self.inner
            .read()
            .map(|guard| {
                let max_y = guard.content_size.1.saturating_sub(guard.viewport_size.1);
                guard.offset_y < max_y
            })
            .unwrap_or(false)
    }

    /// Check if scrolling left is possible.
    pub fn can_scroll_left(&self) -> bool {
        self.inner
            .read()
            .map(|guard| guard.offset_x > 0)
            .unwrap_or(false)
    }

    /// Check if scrolling right is possible.
    pub fn can_scroll_right(&self) -> bool {
        self.inner
            .read()
            .map(|guard| {
                let max_x = guard.content_size.0.saturating_sub(guard.viewport_size.0);
                guard.offset_x < max_x
            })
            .unwrap_or(false)
    }

    /// Get the content size (width, height).
    pub fn content_size(&self) -> (u16, u16) {
        self.inner
            .read()
            .map(|guard| guard.content_size)
            .unwrap_or((0, 0))
    }

    /// Get the viewport size (width, height).
    pub fn viewport_size(&self) -> (u16, u16) {
        self.inner
            .read()
            .map(|guard| guard.viewport_size)
            .unwrap_or((0, 0))
    }

    // -------------------------------------------------------------------------
    // Runtime updates (called by renderer)
    // -------------------------------------------------------------------------

    /// Update the content and viewport sizes (called by renderer).
    pub fn set_sizes(&self, content: (u16, u16), viewport: (u16, u16)) {
        if let Ok(mut guard) = self.inner.write() {
            guard.content_size = content;
            guard.viewport_size = viewport;

            // Clamp scroll position to valid range
            let max_x = content.0.saturating_sub(viewport.0);
            let max_y = content.1.saturating_sub(viewport.1);
            guard.offset_x = guard.offset_x.min(max_x);
            guard.offset_y = guard.offset_y.min(max_y);
        }
    }

    /// Update the vertical scrollbar geometry (called by renderer).
    pub fn set_vertical_scrollbar(&self, geometry: Option<ScrollbarGeometry>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.vertical_scrollbar = geometry;
        }
    }

    /// Update the horizontal scrollbar geometry (called by renderer).
    pub fn set_horizontal_scrollbar(&self, geometry: Option<ScrollbarGeometry>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.horizontal_scrollbar = geometry;
        }
    }

    /// Get the vertical scrollbar geometry.
    pub fn vertical_scrollbar(&self) -> Option<ScrollbarGeometry> {
        self.inner
            .read()
            .ok()
            .and_then(|guard| guard.vertical_scrollbar)
    }

    /// Get the horizontal scrollbar geometry.
    pub fn horizontal_scrollbar(&self) -> Option<ScrollbarGeometry> {
        self.inner
            .read()
            .ok()
            .and_then(|guard| guard.horizontal_scrollbar)
    }

    /// Scroll to a position based on a ratio (0.0 - 1.0).
    pub fn scroll_to_ratio(&self, x_ratio: Option<f32>, y_ratio: Option<f32>) {
        if let Ok(mut guard) = self.inner.write() {
            if let Some(ratio) = y_ratio {
                let max_y = guard.content_size.1.saturating_sub(guard.viewport_size.1);
                guard.offset_y = (ratio * max_y as f32).round() as u16;
                self.dirty.store(true, Ordering::SeqCst);
            }
            if let Some(ratio) = x_ratio {
                let max_x = guard.content_size.0.saturating_sub(guard.viewport_size.0);
                guard.offset_x = (ratio * max_x as f32).round() as u16;
                self.dirty.store(true, Ordering::SeqCst);
            }
        }
    }

    // -------------------------------------------------------------------------
    // Dirty tracking
    // -------------------------------------------------------------------------

    /// Check if the scrollable state has changed.
    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::SeqCst)
    }

    /// Clear the dirty flag.
    pub fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::SeqCst);
    }

    // -------------------------------------------------------------------------
    // Drag state
    // -------------------------------------------------------------------------

    /// Get current drag state.
    pub fn drag(&self) -> Option<ScrollbarDrag> {
        self.inner.read().map(|guard| guard.drag).unwrap_or(None)
    }

    /// Set current drag state.
    pub fn set_drag(&self, drag: Option<ScrollbarDrag>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.drag = drag;
        }
    }
}

impl Clone for Scrollable {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            inner: Arc::clone(&self.inner),
            dirty: Arc::clone(&self.dirty),
        }
    }
}

impl Default for Scrollable {
    fn default() -> Self {
        Self::new()
    }
}
