//! Scroll state and scrollbar widget for virtualized containers.
//!
//! This module provides:
//! - `ScrollState`: Widget-level scroll state with offset, viewport, and request queue
//! - `ScrollRequest`: Actions that can be requested on scroll state
//! - `Scrollbar`: Standalone scrollbar widget
//! - `ScrollableWidgetState`: Trait for widget states that support scrollbar interaction
//! - `register_scrollbar_handlers`: Helper to register scrollbar click/drag handlers

use std::sync::Arc;

use tuidom::{Color, Element, Size, Style};

use crate::state::State;
use crate::HandlerRegistry;

// =============================================================================
// ScrollRequest
// =============================================================================

/// Actions that can be requested on scroll state.
///
/// These are consumed by widgets on the next build cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollRequest {
    /// Scroll by relative amount (positive = down, negative = up).
    Delta(i16),
    /// Scroll to absolute offset.
    ToOffset(u16),
    /// Scroll to make item at index visible.
    IntoView(usize),
    /// Scroll up by one page.
    PageUp,
    /// Scroll down by one page.
    PageDown,
    /// Scroll to top.
    Home,
    /// Scroll to bottom.
    End,
}

// =============================================================================
// ScrollState
// =============================================================================

/// Scroll state for virtualized containers.
///
/// Managed by the widget, can be manipulated by app via scroll requests.
/// The widget consumes pending requests on each build cycle.
///
/// # Example
///
/// ```ignore
/// // In app state:
/// scroll: ScrollState,
///
/// // In handler:
/// self.scroll.get_mut().scroll_by(3);  // Scroll down 3 rows
/// self.scroll.get_mut().page_down();   // Page down
/// ```
#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    /// Current scroll offset (pixels/rows from top).
    pub offset: u16,

    /// Viewport height (set by widget after layout feedback).
    pub viewport: u16,

    /// Total content height (computed from items).
    pub content_height: u16,

    /// Pending scroll request (consumed by widget on next build).
    request: Option<ScrollRequest>,
}

impl ScrollState {
    /// Create a new scroll state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a scroll state with known content height.
    pub fn with_content_height(content_height: u16) -> Self {
        Self {
            content_height,
            ..Default::default()
        }
    }

    /// Maximum scroll offset (content_height - viewport).
    pub fn max_offset(&self) -> u16 {
        self.content_height.saturating_sub(self.viewport)
    }

    /// Current scroll progress (0.0 = top, 1.0 = bottom).
    pub fn progress(&self) -> f32 {
        let max = self.max_offset();
        if max == 0 {
            0.0
        } else {
            self.offset as f32 / max as f32
        }
    }

    /// Whether content exceeds viewport (scrolling needed).
    pub fn can_scroll(&self) -> bool {
        self.content_height > self.viewport
    }

    /// Request scroll by relative amount.
    pub fn scroll_by(&mut self, delta: i16) {
        self.request = Some(ScrollRequest::Delta(delta));
    }

    /// Request scroll to absolute offset.
    pub fn scroll_to(&mut self, offset: u16) {
        self.request = Some(ScrollRequest::ToOffset(offset));
    }

    /// Request scroll to make item at index visible.
    pub fn scroll_into_view(&mut self, index: usize) {
        self.request = Some(ScrollRequest::IntoView(index));
    }

    /// Request page up.
    pub fn page_up(&mut self) {
        self.request = Some(ScrollRequest::PageUp);
    }

    /// Request page down.
    pub fn page_down(&mut self) {
        self.request = Some(ScrollRequest::PageDown);
    }

    /// Request scroll to top.
    pub fn home(&mut self) {
        self.request = Some(ScrollRequest::Home);
    }

    /// Request scroll to bottom.
    pub fn end(&mut self) {
        self.request = Some(ScrollRequest::End);
    }

    /// Take and clear pending request.
    ///
    /// Called by widgets to process pending scroll actions.
    pub fn take_request(&mut self) -> Option<ScrollRequest> {
        self.request.take()
    }

    /// Check if there's a pending request.
    pub fn has_request(&self) -> bool {
        self.request.is_some()
    }

    /// Set viewport size.
    ///
    /// Called by widgets after receiving layout dimensions.
    pub fn set_viewport(&mut self, height: u16) {
        self.viewport = height;
        // Clamp offset if viewport grew larger than content
        self.offset = self.offset.min(self.max_offset());
    }

    /// Set content height.
    ///
    /// Called by widgets when items change.
    pub fn set_content_height(&mut self, height: u16) {
        self.content_height = height;
        // Clamp offset if content shrunk
        self.offset = self.offset.min(self.max_offset());
    }

    /// Apply a scroll request directly to the offset.
    ///
    /// This is a convenience method for widgets to process requests.
    /// For `IntoView`, the widget should handle it separately since
    /// it requires knowledge of item positions.
    ///
    /// Returns the processed request if it was handled, or the original
    /// request if it needs widget-specific handling (like `IntoView`).
    pub fn apply_request(&mut self, request: ScrollRequest) -> Option<ScrollRequest> {
        let max = self.max_offset();
        match request {
            ScrollRequest::Delta(d) => {
                let new_offset = (self.offset as i32 + d as i32).clamp(0, max as i32) as u16;
                self.offset = new_offset;
                None
            }
            ScrollRequest::ToOffset(o) => {
                self.offset = o.min(max);
                None
            }
            ScrollRequest::PageUp => {
                self.offset = self.offset.saturating_sub(self.viewport);
                None
            }
            ScrollRequest::PageDown => {
                self.offset = (self.offset + self.viewport).min(max);
                None
            }
            ScrollRequest::Home => {
                self.offset = 0;
                None
            }
            ScrollRequest::End => {
                self.offset = max;
                None
            }
            ScrollRequest::IntoView(_) => {
                // Needs widget-specific handling
                Some(request)
            }
        }
    }

    /// Process and apply any pending request.
    ///
    /// Returns the request if it needs widget-specific handling.
    pub fn process_request(&mut self) -> Option<ScrollRequest> {
        self.take_request().and_then(|r| self.apply_request(r))
    }
}

// =============================================================================
// Scrollbar
// =============================================================================

/// Scrollbar orientation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Orientation {
    #[default]
    Vertical,
    Horizontal,
}

/// Style configuration for scrollbar rendering.
#[derive(Debug, Clone)]
pub struct ScrollbarStyle {
    /// Character for the track (background).
    pub track_char: char,
    /// Character for the thumb (draggable part).
    pub thumb_char: char,
    /// Style for the track.
    pub track_style: Style,
    /// Style for the thumb.
    pub thumb_style: Style,
}

impl Default for ScrollbarStyle {
    fn default() -> Self {
        Self {
            track_char: '░',
            thumb_char: '█',
            // Match tuidom's scrollbar colors: dark gray track, lighter gray thumb
            track_style: Style::new().foreground(Color::Rgb { r: 60, g: 60, b: 60 }),
            thumb_style: Style::new().foreground(Color::Rgb { r: 150, g: 150, b: 150 }),
        }
    }
}

impl ScrollbarStyle {
    /// Create a new scrollbar style.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set track character.
    pub fn track_char(mut self, c: char) -> Self {
        self.track_char = c;
        self
    }

    /// Set thumb character.
    pub fn thumb_char(mut self, c: char) -> Self {
        self.thumb_char = c;
        self
    }

    /// Set track style.
    pub fn track_style(mut self, style: Style) -> Self {
        self.track_style = style;
        self
    }

    /// Set thumb style.
    pub fn thumb_style(mut self, style: Style) -> Self {
        self.thumb_style = style;
        self
    }
}

/// Typestate marker: scrollbar needs a state reference.
pub struct NeedsScrollState;

/// Typestate marker: scrollbar has a state reference.
pub struct HasScrollState<'a, S: ScrollableWidgetState>(pub(crate) &'a State<S>);

/// A standalone scrollbar widget.
///
/// Renders a track with a thumb positioned according to scroll state.
/// Can be vertical or horizontal. Uses typestate pattern to enforce
/// `state()` is called before `build()`.
///
/// # Example
///
/// ```ignore
/// // In a widget's build method:
/// let scrollbar = Scrollbar::vertical()
///     .id(&scrollbar_id)
///     .state(table_state)
///     .build(registry, handlers);
/// ```
#[derive(Debug, Clone)]
pub struct Scrollbar<S = NeedsScrollState> {
    state_marker: S,
    id: Option<String>,
    orientation: Orientation,
    /// Scrollbar style.
    style: ScrollbarStyle,
}

impl Default for Scrollbar<NeedsScrollState> {
    fn default() -> Self {
        Self::new()
    }
}

impl Scrollbar<NeedsScrollState> {
    /// Create a new scrollbar builder.
    pub fn new() -> Self {
        Self {
            state_marker: NeedsScrollState,
            id: None,
            orientation: Orientation::Vertical,
            style: ScrollbarStyle::default(),
        }
    }

    /// Create a vertical scrollbar.
    pub fn vertical() -> Self {
        Self::new()
    }

    /// Create a horizontal scrollbar.
    pub fn horizontal() -> Self {
        Self {
            orientation: Orientation::Horizontal,
            ..Self::new()
        }
    }

    /// Set the state reference. Required before calling `build()`.
    pub fn state<T: ScrollableWidgetState>(self, s: &State<T>) -> Scrollbar<HasScrollState<'_, T>> {
        Scrollbar {
            state_marker: HasScrollState(s),
            id: self.id,
            orientation: self.orientation,
            style: self.style,
        }
    }
}

impl<S> Scrollbar<S> {
    /// Set the scrollbar id.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set scrollbar style.
    pub fn style(mut self, style: ScrollbarStyle) -> Self {
        self.style = style;
        self
    }
}

impl<'a, S: ScrollableWidgetState> Scrollbar<HasScrollState<'a, S>> {
    /// Build the scrollbar element and register handlers.
    ///
    /// Registers handlers for click, drag, and layout events.
    /// Calls `on_scroll` handler when scrolling occurs.
    pub fn build(
        self,
        registry: &HandlerRegistry,
        handlers: &crate::WidgetHandlers,
    ) -> Element {
        let state = self.state_marker.0;
        let current = state.get();
        let scroll = current.scroll();

        let id = self.id.clone().unwrap_or_else(|| "scrollbar".into());

        log::debug!(
            "[Scrollbar::build] viewport={} content_size={} offset={}",
            scroll.viewport, scroll.content_height, scroll.offset
        );

        // Calculate thumb size and position
        let track_size = scroll.viewport;
        let max_offset = scroll.content_height.saturating_sub(scroll.viewport);

        // If viewport is 0, return empty element (happens on first frame before layout)
        if track_size == 0 {
            return Element::col().id(&id).width(Size::Fixed(1)).height(Size::Fill);
        }

        // Thumb size proportional to viewport/content ratio
        let thumb_size = if scroll.content_height == 0 {
            track_size
        } else {
            let ratio = scroll.viewport as f32 / scroll.content_height as f32;
            ((ratio * track_size as f32).round() as u16).clamp(1, track_size)
        };

        // Thumb position proportional to scroll progress
        let thumb_pos = if max_offset == 0 {
            0
        } else {
            let progress = scroll.offset as f32 / max_offset as f32;
            let available_space = track_size.saturating_sub(thumb_size);
            (progress * available_space as f32).round() as u16
        };

        // Register handlers
        self.register_handlers(&id, registry, handlers, state);

        // Build the element
        match self.orientation {
            Orientation::Vertical => {
                Self::build_vertical_element(&id, track_size, thumb_size, thumb_pos, &self.style)
            }
            Orientation::Horizontal => {
                Self::build_horizontal_element(&id, track_size, thumb_size, thumb_pos, &self.style)
            }
        }
    }

    fn register_handlers(
        &self,
        scrollbar_id: &str,
        registry: &HandlerRegistry,
        handlers: &crate::WidgetHandlers,
        state: &State<S>,
    ) {
        // on_layout: store scrollbar rect for position calculations
        {
            let state_clone = state.clone();
            registry.register(
                scrollbar_id,
                "on_layout",
                Arc::new(move |hx| {
                    if let Some((x, y, width, height)) = hx.event().layout() {
                        state_clone.update(|s| {
                            s.set_scrollbar_rect(Some((x, y, width, height)));
                        });
                    }
                }),
            );
        }

        // on_activate: click on scrollbar - detect thumb vs track
        {
            let state_clone = state.clone();
            let on_scroll = handlers.get("on_scroll").cloned();
            registry.register(
                scrollbar_id,
                "on_activate",
                Arc::new(move |hx| {
                    if let Some((_, click_y)) = hx.event().click_position() {
                        // Extract only needed values without cloning entire state
                        let scroll_data = state_clone.with_ref(|s| {
                            s.scrollbar_rect().map(|(_, track_y, _, track_height)| {
                                let scroll = s.scroll();
                                (
                                    track_y,
                                    track_height,
                                    scroll.content_height,
                                    scroll.viewport,
                                    scroll.max_offset(),
                                    scroll.offset,
                                )
                            })
                        });

                        if let Some((track_y, track_height, content_size, viewport, max_offset, current_offset)) = scroll_data {
                            if track_height > 0 {
                                // Calculate thumb size and position
                                let thumb_size = if content_size == 0 {
                                    track_height
                                } else {
                                    let ratio = viewport as f32 / content_size as f32;
                                    ((ratio * track_height as f32).round() as u16)
                                        .clamp(1, track_height)
                                };

                                let thumb_pos = if max_offset == 0 {
                                    0
                                } else {
                                    let progress = current_offset as f32 / max_offset as f32;
                                    let available_space = track_height.saturating_sub(thumb_size);
                                    (progress * available_space as f32).round() as u16
                                };

                                let thumb_screen_start = track_y + thumb_pos;
                                let thumb_screen_end = thumb_screen_start + thumb_size;

                                if click_y >= thumb_screen_start && click_y < thumb_screen_end {
                                    // Clicked on thumb - store grab offset
                                    let grab_offset = click_y - thumb_screen_start;
                                    state_clone.update(|s| {
                                        s.set_drag_grab_offset(Some(grab_offset));
                                    });
                                } else {
                                    // Clicked on track - jump to position
                                    let relative_y = click_y.saturating_sub(track_y);
                                    let track_ratio =
                                        relative_y as f32 / track_height.max(1) as f32;
                                    let grab_offset = ((track_ratio * thumb_size as f32).round()
                                        as u16)
                                        .min(thumb_size.saturating_sub(1));

                                    let scroll_range = track_height.saturating_sub(thumb_size);
                                    let thumb_start = click_y.saturating_sub(grab_offset);
                                    let clamped_thumb_start =
                                        thumb_start.clamp(track_y, track_y + scroll_range);
                                    let thumb_pos_in_track =
                                        clamped_thumb_start.saturating_sub(track_y);

                                    let new_offset = if scroll_range == 0 {
                                        0
                                    } else {
                                        ((thumb_pos_in_track as u32 * max_offset as u32
                                            + scroll_range as u32 / 2)
                                            / scroll_range as u32)
                                            .min(max_offset as u32)
                                            as u16
                                    };

                                    state_clone.update(|s| {
                                        s.scroll_mut().offset = new_offset;
                                        s.set_drag_grab_offset(Some(grab_offset));
                                    });

                                    // Call on_scroll handler with proper scroll event
                                    if let Some(ref handler) = on_scroll {
                                        let scroll_event = crate::handler_context::EventData::Scroll {
                                            offset_x: 0,
                                            offset_y: new_offset,
                                            content_width: 0,
                                            content_height: content_size,
                                            viewport_width: 0,
                                            viewport_height: viewport,
                                        };
                                        let scroll_hx = hx.with_event(scroll_event);
                                        handler(&scroll_hx);
                                    }
                                }
                            }
                        }
                    }
                }),
            );
        }

        // on_drag: drag to scroll
        {
            let state_clone = state.clone();
            let on_scroll = handlers.get("on_scroll").cloned();
            registry.register(
                scrollbar_id,
                "on_drag",
                Arc::new(move |hx| {
                    if let Some((_, drag_y)) = hx.event().drag_position() {
                        // Extract only needed values without cloning entire state
                        let scroll_data = state_clone.with_ref(|s| {
                            s.scrollbar_rect().map(|(_, track_y, _, track_height)| {
                                let scroll = s.scroll();
                                (
                                    track_y,
                                    track_height,
                                    scroll.content_height,
                                    scroll.viewport,
                                    scroll.max_offset(),
                                    s.drag_grab_offset().unwrap_or(0),
                                )
                            })
                        });

                        if let Some((track_y, track_height, content_size, viewport, max_offset, grab_offset)) = scroll_data {
                            if track_height > 0 {
                                let thumb_size = if content_size == 0 {
                                    track_height
                                } else {
                                    let ratio = viewport as f32 / content_size as f32;
                                    ((ratio * track_height as f32).round() as u16)
                                        .clamp(1, track_height)
                                };

                                let scroll_range = track_height.saturating_sub(thumb_size);
                                let thumb_start = drag_y.saturating_sub(grab_offset);
                                let clamped_thumb_start =
                                    thumb_start.clamp(track_y, track_y + scroll_range);
                                let thumb_pos_in_track =
                                    clamped_thumb_start.saturating_sub(track_y);

                                let new_offset = if scroll_range == 0 {
                                    0
                                } else {
                                    ((thumb_pos_in_track as u32 * max_offset as u32
                                        + scroll_range as u32 / 2)
                                        / scroll_range as u32)
                                        .min(max_offset as u32) as u16
                                };

                                state_clone.update(|s| {
                                    s.scroll_mut().offset = new_offset;
                                });

                                // Call on_scroll handler with proper scroll event
                                if let Some(ref handler) = on_scroll {
                                    let scroll_event = crate::handler_context::EventData::Scroll {
                                        offset_x: 0,
                                        offset_y: new_offset,
                                        content_width: 0,
                                        content_height: content_size,
                                        viewport_width: 0,
                                        viewport_height: viewport,
                                    };
                                    let scroll_hx = hx.with_event(scroll_event);
                                    handler(&scroll_hx);
                                }
                            }
                        }
                    }
                }),
            );
        }

        // on_release: clear grab offset
        {
            let state_clone = state.clone();
            registry.register(
                scrollbar_id,
                "on_release",
                Arc::new(move |_hx| {
                    state_clone.update(|s| {
                        s.set_drag_grab_offset(None);
                    });
                }),
            );
        }
    }

    fn build_vertical_element(
        id: &str,
        track_size: u16,
        thumb_size: u16,
        thumb_pos: u16,
        style: &ScrollbarStyle,
    ) -> Element {
        let mut children = Vec::with_capacity(track_size as usize);

        for i in 0..track_size {
            let is_thumb = i >= thumb_pos && i < thumb_pos + thumb_size;
            let (ch, s) = if is_thumb {
                (style.thumb_char, style.thumb_style.clone())
            } else {
                (style.track_char, style.track_style.clone())
            };
            children.push(Element::text(&ch.to_string()).style(s));
        }

        Element::col()
            .id(id)
            .width(Size::Fixed(1))
            .height(Size::Fill)
            .focusable(true)
            .clickable(true)
            .z_index(10)
            .children(children)
    }

    fn build_horizontal_element(
        id: &str,
        track_size: u16,
        thumb_size: u16,
        thumb_pos: u16,
        style: &ScrollbarStyle,
    ) -> Element {
        let mut text = String::with_capacity(track_size as usize);

        for i in 0..track_size {
            let is_thumb = i >= thumb_pos && i < thumb_pos + thumb_size;
            if is_thumb {
                text.push(style.thumb_char);
            } else {
                text.push(style.track_char);
            }
        }

        Element::text(&text)
            .id(id)
            .width(Size::Fill)
            .height(Size::Fixed(1))
            .style(style.thumb_style.clone())
    }
}

// =============================================================================
// ScrollableWidgetState Trait
// =============================================================================

/// Trait for widget states that support scrollbar interaction.
///
/// Implement this trait on your widget's state type to use the
/// `register_scrollbar_handlers` helper function.
pub trait ScrollableWidgetState: Clone + Send + Sync + 'static {
    /// Get a reference to the scroll state.
    fn scroll(&self) -> &ScrollState;

    /// Get a mutable reference to the scroll state.
    fn scroll_mut(&mut self) -> &mut ScrollState;

    /// Get the scrollbar's screen rect (x, y, width, height).
    fn scrollbar_rect(&self) -> Option<(u16, u16, u16, u16)>;

    /// Set the scrollbar's screen rect.
    fn set_scrollbar_rect(&mut self, rect: Option<(u16, u16, u16, u16)>);

    /// Get the drag grab offset within the thumb.
    fn drag_grab_offset(&self) -> Option<u16>;

    /// Set the drag grab offset.
    fn set_drag_grab_offset(&mut self, offset: Option<u16>);
}

