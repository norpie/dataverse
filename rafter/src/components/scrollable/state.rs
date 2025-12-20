//! Scrollable component state.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use super::super::scrollbar::{ScrollbarConfig, ScrollbarDrag, ScrollbarGeometry, ScrollbarState};

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

// =============================================================================
// ScrollbarState trait implementation
// =============================================================================

impl ScrollbarState for Scrollable {
    fn scrollbar_config(&self) -> ScrollbarConfig {
        self.inner
            .read()
            .map(|guard| guard.scrollbar.clone())
            .unwrap_or_default()
    }

    fn set_scrollbar_config(&self, config: ScrollbarConfig) {
        if let Ok(mut guard) = self.inner.write() {
            guard.scrollbar = config;
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    fn scroll_offset_y(&self) -> u16 {
        self.offset_y()
    }

    fn scroll_offset_x(&self) -> u16 {
        self.offset_x()
    }

    fn scroll_to_y(&self, y: u16) {
        if let Ok(mut guard) = self.inner.write() {
            let max_y = guard.content_size.1.saturating_sub(guard.viewport_size.1);
            guard.offset_y = y.min(max_y);
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    fn scroll_to_x(&self, x: u16) {
        if let Ok(mut guard) = self.inner.write() {
            let max_x = guard.content_size.0.saturating_sub(guard.viewport_size.0);
            guard.offset_x = x.min(max_x);
            self.dirty.store(true, Ordering::SeqCst);
        }
    }

    fn scroll_by(&self, dx: i16, dy: i16) {
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

    fn scroll_to_top(&self) {
        if let Ok(mut guard) = self.inner.write() {
            if guard.offset_y != 0 {
                guard.offset_y = 0;
                self.dirty.store(true, Ordering::SeqCst);
            }
        }
    }

    fn scroll_to_bottom(&self) {
        if let Ok(mut guard) = self.inner.write() {
            let max_y = guard.content_size.1.saturating_sub(guard.viewport_size.1);
            if guard.offset_y != max_y {
                guard.offset_y = max_y;
                self.dirty.store(true, Ordering::SeqCst);
            }
        }
    }

    fn content_height(&self) -> u16 {
        self.inner.read().map(|g| g.content_size.1).unwrap_or(0)
    }

    fn content_width(&self) -> u16 {
        self.inner.read().map(|g| g.content_size.0).unwrap_or(0)
    }

    fn viewport_height(&self) -> u16 {
        self.inner.read().map(|g| g.viewport_size.1).unwrap_or(0)
    }

    fn viewport_width(&self) -> u16 {
        self.inner.read().map(|g| g.viewport_size.0).unwrap_or(0)
    }

    fn vertical_scrollbar(&self) -> Option<ScrollbarGeometry> {
        self.inner
            .read()
            .ok()
            .and_then(|guard| guard.vertical_scrollbar)
    }

    fn set_vertical_scrollbar(&self, geometry: Option<ScrollbarGeometry>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.vertical_scrollbar = geometry;
        }
    }

    fn horizontal_scrollbar(&self) -> Option<ScrollbarGeometry> {
        self.inner
            .read()
            .ok()
            .and_then(|guard| guard.horizontal_scrollbar)
    }

    fn set_horizontal_scrollbar(&self, geometry: Option<ScrollbarGeometry>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.horizontal_scrollbar = geometry;
        }
    }

    fn drag(&self) -> Option<ScrollbarDrag> {
        self.inner.read().map(|guard| guard.drag).unwrap_or(None)
    }

    fn set_drag(&self, drag: Option<ScrollbarDrag>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.drag = drag;
        }
    }
}
