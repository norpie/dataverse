use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::element::{Content, Element};
use crate::layout::LayoutResult;
use crate::transitions::{Easing, TransitionConfig};
use crate::types::{Color, Size};

/// Which property is being transitioned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransitionProperty {
    Width,
    Height,
    Background,
    Foreground,
}

/// A property value that can be interpolated.
#[derive(Debug, Clone)]
pub enum PropertyValue {
    U16(u16),
    Color(Color),
}

/// Snapshot of an element's transitionable properties (excluding position).
#[derive(Debug, Clone, Default)]
struct ElementSnapshot {
    width: Option<u16>,
    height: Option<u16>,
    background: Option<Color>,
    foreground: Option<Color>,
}

/// A single active transition.
#[derive(Debug, Clone)]
struct ActiveTransition {
    from: PropertyValue,
    to: PropertyValue,
    start: Instant,
    duration: Duration,
    easing: Easing,
}

/// Frame animation state for a single element.
#[derive(Debug, Clone)]
struct FrameState {
    /// Current frame index
    index: usize,
    /// When the current frame started
    started: Instant,
    /// Frame interval
    interval: Duration,
    /// Total number of frames
    count: usize,
}

/// Active position transition for an element (x and/or y).
#[derive(Debug, Clone)]
struct PositionTransition {
    // X transition (if active)
    from_x: Option<u16>,
    to_x: Option<u16>,
    x_start: Option<Instant>,
    x_duration: Duration,
    x_easing: Easing,
    // Y transition (if active)
    from_y: Option<u16>,
    to_y: Option<u16>,
    y_start: Option<Instant>,
    y_duration: Duration,
    y_easing: Easing,
}

impl PositionTransition {
    fn has_active_x(&self, now: Instant) -> bool {
        if let (Some(start), Some(_)) = (self.x_start, self.from_x) {
            now.duration_since(start) < self.x_duration
        } else {
            false
        }
    }

    fn has_active_y(&self, now: Instant) -> bool {
        if let (Some(start), Some(_)) = (self.y_start, self.from_y) {
            now.duration_since(start) < self.y_duration
        } else {
            false
        }
    }

    fn has_any_active(&self, now: Instant) -> bool {
        self.has_active_x(now) || self.has_active_y(now)
    }
}

/// Manages animation state across frames.
#[derive(Debug, Default)]
pub struct AnimationState {
    /// Previous frame's property values per element.
    snapshots: HashMap<String, ElementSnapshot>,
    /// Currently active property transitions: (element_id, property) -> transition.
    active: HashMap<(String, TransitionProperty), ActiveTransition>,
    /// Frame animation state per element.
    frame_states: HashMap<String, FrameState>,
    /// Reduced motion flag - when true, transitions complete instantly.
    reduced_motion: bool,
    /// Previous frame's computed positions per element.
    positions: HashMap<String, (u16, u16)>,
    /// Active position transitions per element.
    position_transitions: HashMap<String, PositionTransition>,
}

impl AnimationState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable or disable reduced motion (accessibility).
    /// When enabled, all transitions complete instantly.
    pub fn set_reduced_motion(&mut self, enabled: bool) {
        self.reduced_motion = enabled;
    }

    /// Returns true if any animation (transition or frame) is currently active.
    pub fn has_active_animations(&self) -> bool {
        let now = Instant::now();
        !self.active.is_empty()
            || !self.frame_states.is_empty()
            || self.position_transitions.values().any(|t| t.has_any_active(now))
    }

    /// Capture current layout positions for next frame comparison.
    /// Call this BEFORE running new layout.
    pub fn capture_layout(&mut self, layout: &LayoutResult) {
        self.positions.clear();
        for (id, rect) in layout.iter_rects() {
            self.positions.insert(id.clone(), (rect.x, rect.y));
        }
        log::trace!(
            "[anim] capture_layout: captured {} positions",
            self.positions.len()
        );
    }

    /// Compare new layout with captured positions, start transitions for changes.
    /// Call this AFTER running new layout.
    pub fn update_layout(&mut self, layout: &LayoutResult, root: &Element) {
        let now = Instant::now();

        // Prune completed position transitions
        self.position_transitions.retain(|_, t| t.has_any_active(now));

        // Check each element for position changes
        self.check_position_changes(root, layout, now);
    }

    fn check_position_changes(&mut self, element: &Element, layout: &LayoutResult, now: Instant) {
        let x_config = element.transitions.x;
        let y_config = element.transitions.y;

        if x_config.is_some() || y_config.is_some() {
            if let Some(new_rect) = layout.get(&element.id) {
                let new_pos = (new_rect.x, new_rect.y);
                let old_pos = self.positions.get(&element.id).copied();

                if let Some(old_pos) = old_pos {
                    let x_changed = old_pos.0 != new_pos.0;
                    let y_changed = old_pos.1 != new_pos.1;

                    if (x_changed || y_changed) && !self.reduced_motion {
                        // Get current interpolated positions BEFORE mutating (for interrupt handling)
                        let current_interp_x = self.get_interpolated_x(&element.id, now);
                        let current_interp_y = self.get_interpolated_y(&element.id, now);

                        // Get or create position transition for this element
                        let transition = self
                            .position_transitions
                            .entry(element.id.clone())
                            .or_insert(PositionTransition {
                                from_x: None,
                                to_x: None,
                                x_start: None,
                                x_duration: Duration::ZERO,
                                x_easing: Easing::Linear,
                                from_y: None,
                                to_y: None,
                                y_start: None,
                                y_duration: Duration::ZERO,
                                y_easing: Easing::Linear,
                            });

                        // Handle X change
                        if x_changed {
                            if let Some(config) = x_config {
                                let from_x = if transition.has_active_x(now) {
                                    // Interrupt: start from current interpolated position
                                    current_interp_x.unwrap_or(old_pos.0)
                                } else {
                                    old_pos.0
                                };

                                log::debug!(
                                    "[anim] {} x: starting transition {} -> {}",
                                    element.id,
                                    from_x,
                                    new_pos.0
                                );

                                transition.from_x = Some(from_x);
                                transition.to_x = Some(new_pos.0);
                                transition.x_start = Some(now);
                                transition.x_duration = config.duration;
                                transition.x_easing = config.easing;
                            }
                        }

                        // Handle Y change
                        if y_changed {
                            if let Some(config) = y_config {
                                let from_y = if transition.has_active_y(now) {
                                    // Interrupt: start from current interpolated position
                                    current_interp_y.unwrap_or(old_pos.1)
                                } else {
                                    old_pos.1
                                };

                                log::debug!(
                                    "[anim] {} y: starting transition {} -> {}",
                                    element.id,
                                    from_y,
                                    new_pos.1
                                );

                                transition.from_y = Some(from_y);
                                transition.to_y = Some(new_pos.1);
                                transition.y_start = Some(now);
                                transition.y_duration = config.duration;
                                transition.y_easing = config.easing;
                            }
                        }
                    }
                }
            }
        }

        // Recurse into children
        // Skip children of virtualized containers (item_height set)
        match &element.content {
            Content::Children(children) if element.item_height.is_none() => {
                for child in children {
                    self.check_position_changes(child, layout, now);
                }
            }
            Content::Frames { children, .. } => {
                for child in children {
                    self.check_position_changes(child, layout, now);
                }
            }
            _ => {}
        }
    }

    /// Get interpolated X position for an element.
    fn get_interpolated_x(&self, id: &str, now: Instant) -> Option<u16> {
        let transition = self.position_transitions.get(id)?;
        let start = transition.x_start?;
        let from = transition.from_x?;
        let to = transition.to_x?;

        let elapsed = now.duration_since(start);
        if elapsed >= transition.x_duration {
            return None; // Transition complete
        }

        let progress = elapsed.as_secs_f32() / transition.x_duration.as_secs_f32();
        let eased = transition.x_easing.apply(progress);
        Some(lerp_u16(from, to, eased))
    }

    /// Get interpolated Y position for an element.
    fn get_interpolated_y(&self, id: &str, now: Instant) -> Option<u16> {
        let transition = self.position_transitions.get(id)?;
        let start = transition.y_start?;
        let from = transition.from_y?;
        let to = transition.to_y?;

        let elapsed = now.duration_since(start);
        if elapsed >= transition.y_duration {
            return None; // Transition complete
        }

        let progress = elapsed.as_secs_f32() / transition.y_duration.as_secs_f32();
        let eased = transition.y_easing.apply(progress);
        Some(lerp_u16(from, to, eased))
    }

    /// Get interpolated position for an element.
    /// Returns (interpolated_x, interpolated_y) where each is Some if active, None if not.
    pub fn get_interpolated_position(&self, id: &str, now: Instant) -> (Option<u16>, Option<u16>) {
        (self.get_interpolated_x(id, now), self.get_interpolated_y(id, now))
    }

    /// Returns when the next animation tick is due.
    pub fn next_tick_due(&self) -> Option<Duration> {
        let now = Instant::now();
        let mut min_due: Option<Duration> = None;

        // Check property transitions and position transitions
        if !self.active.is_empty()
            || self
                .position_transitions
                .values()
                .any(|t| t.has_any_active(now))
        {
            min_due = Some(Duration::from_millis(16));
        }

        // Check frame animations
        for state in self.frame_states.values() {
            let elapsed = now.duration_since(state.started);
            let remaining = state.interval.saturating_sub(elapsed);
            min_due = Some(min_due.map_or(remaining, |m| m.min(remaining)));
        }

        min_due
    }

    /// Get the current frame index for an element with frame animation.
    pub fn current_frame(&self, element_id: &str) -> usize {
        self.frame_states
            .get(element_id)
            .map(|s| s.index)
            .unwrap_or(0)
    }

    /// Update animation state based on current element tree.
    /// Detects property changes, starts new transitions, and prunes completed ones.
    pub fn update(&mut self, root: &Element) {
        let now = Instant::now();

        // Prune completed property transitions
        self.active
            .retain(|_, transition| now.duration_since(transition.start) < transition.duration);

        // Walk tree and check for property changes
        self.update_element(root, now);
    }

    fn update_element(&mut self, element: &Element, now: Instant) {
        let id = &element.id;
        let current = Self::snapshot_element(element);
        let transitions = &element.transitions;

        // Compare with previous snapshot and start transitions for non-position properties
        if let Some(prev) = self.snapshots.get(id).cloned() {
            self.check_and_start_u16_transition(
                id,
                TransitionProperty::Width,
                prev.width,
                current.width,
                transitions.width,
                now,
            );
            self.check_and_start_u16_transition(
                id,
                TransitionProperty::Height,
                prev.height,
                current.height,
                transitions.height,
                now,
            );
            self.check_and_start_color_transition(
                id,
                TransitionProperty::Background,
                prev.background.as_ref(),
                current.background.as_ref(),
                transitions.background,
                now,
            );
            self.check_and_start_color_transition(
                id,
                TransitionProperty::Foreground,
                prev.foreground.as_ref(),
                current.foreground.as_ref(),
                transitions.foreground,
                now,
            );
        }

        // Update snapshot
        self.snapshots.insert(id.clone(), current);

        // Recurse into children
        // Skip children of virtualized containers (item_height set) - they typically
        // don't have transitions and would cause O(n) traversal of all list items
        match &element.content {
            Content::Children(children) if element.item_height.is_none() => {
                for child in children {
                    self.update_element(child, now);
                }
            }
            Content::Frames { children, interval } => {
                self.update_frame_state(id, children.len(), *interval, now);
                let frame_idx = self.current_frame(id);
                if let Some(child) = children.get(frame_idx) {
                    self.update_element(child, now);
                }
            }
            _ => {}
        }
    }

    fn update_frame_state(&mut self, id: &str, count: usize, interval: Duration, now: Instant) {
        if count == 0 {
            self.frame_states.remove(id);
            return;
        }

        let state = self
            .frame_states
            .entry(id.to_string())
            .or_insert(FrameState {
                index: 0,
                started: now,
                interval,
                count,
            });

        state.interval = interval;
        state.count = count;

        let elapsed = now.duration_since(state.started);
        if elapsed >= interval {
            let frames_to_advance = (elapsed.as_millis() / interval.as_millis()) as usize;
            state.index = (state.index + frames_to_advance) % count;
            state.started = now;
        }
    }

    fn check_and_start_u16_transition(
        &mut self,
        id: &str,
        property: TransitionProperty,
        prev: Option<u16>,
        current: Option<u16>,
        config: Option<TransitionConfig>,
        now: Instant,
    ) {
        let Some(config) = config else { return };
        let Some(prev_val) = prev else { return };
        let Some(curr_val) = current else { return };

        if prev_val == curr_val || self.reduced_motion {
            return;
        }

        let key = (id.to_string(), property);
        let from = if let Some(existing) = self.active.get(&key) {
            self.interpolate_value(
                &existing.from,
                &existing.to,
                existing.start,
                existing.duration,
                existing.easing,
                now,
            )
        } else {
            PropertyValue::U16(prev_val)
        };

        self.active.insert(
            key,
            ActiveTransition {
                from,
                to: PropertyValue::U16(curr_val),
                start: now,
                duration: config.duration,
                easing: config.easing,
            },
        );
    }

    fn check_and_start_color_transition(
        &mut self,
        id: &str,
        property: TransitionProperty,
        prev: Option<&Color>,
        current: Option<&Color>,
        config: Option<TransitionConfig>,
        now: Instant,
    ) {
        let Some(config) = config else { return };
        let Some(prev_color) = prev else { return };
        let Some(curr_color) = current else { return };

        if prev_color == curr_color || self.reduced_motion {
            return;
        }

        let key = (id.to_string(), property);
        let from = if let Some(existing) = self.active.get(&key) {
            self.interpolate_value(
                &existing.from,
                &existing.to,
                existing.start,
                existing.duration,
                existing.easing,
                now,
            )
        } else {
            PropertyValue::Color(prev_color.clone())
        };

        self.active.insert(
            key,
            ActiveTransition {
                from,
                to: PropertyValue::Color(curr_color.clone()),
                start: now,
                duration: config.duration,
                easing: config.easing,
            },
        );
    }

    fn snapshot_element(element: &Element) -> ElementSnapshot {
        ElementSnapshot {
            width: match element.width {
                Size::Fixed(w) => Some(w),
                _ => None,
            },
            height: match element.height {
                Size::Fixed(h) => Some(h),
                _ => None,
            },
            background: element.style.background.clone(),
            foreground: element.style.foreground.clone(),
        }
    }

    /// Get interpolated value for a property.
    pub fn get_interpolated(
        &self,
        element_id: &str,
        property: TransitionProperty,
    ) -> Option<PropertyValue> {
        let key = (element_id.to_string(), property);
        let transition = self.active.get(&key)?;
        let now = Instant::now();

        Some(self.interpolate_value(
            &transition.from,
            &transition.to,
            transition.start,
            transition.duration,
            transition.easing,
            now,
        ))
    }

    fn interpolate_value(
        &self,
        from: &PropertyValue,
        to: &PropertyValue,
        start: Instant,
        duration: Duration,
        easing: Easing,
        now: Instant,
    ) -> PropertyValue {
        let elapsed = now.duration_since(start);
        let progress = if duration.is_zero() {
            1.0
        } else {
            (elapsed.as_secs_f32() / duration.as_secs_f32()).min(1.0)
        };
        let eased = easing.apply(progress);

        match (from, to) {
            (PropertyValue::U16(from_val), PropertyValue::U16(to_val)) => {
                PropertyValue::U16(lerp_u16(*from_val, *to_val, eased))
            }
            (PropertyValue::Color(from_color), PropertyValue::Color(to_color)) => {
                PropertyValue::Color(lerp_color(from_color, to_color, eased))
            }
            _ => to.clone(),
        }
    }

    /// Remove transitions, snapshots, and frame states for elements no longer in tree.
    pub fn cleanup(&mut self, current_ids: &HashSet<String>) {
        self.snapshots.retain(|id, _| current_ids.contains(id));
        self.active.retain(|(id, _), _| current_ids.contains(id));
        self.frame_states.retain(|id, _| current_ids.contains(id));
        self.positions.retain(|id, _| current_ids.contains(id));
        self.position_transitions
            .retain(|id, _| current_ids.contains(id));
    }
}

/// Linear interpolation for u16 values.
fn lerp_u16(from: u16, to: u16, t: f32) -> u16 {
    let from = from as f32;
    let to = to as f32;
    (from + (to - from) * t).round() as u16
}

/// Interpolate colors in OKLCH space.
fn lerp_color(from: &Color, to: &Color, t: f32) -> Color {
    let (from_l, from_c, from_h) = color_to_oklch(from);
    let (to_l, to_c, to_h) = color_to_oklch(to);

    let l = from_l + (to_l - from_l) * t;
    let c = from_c + (to_c - from_c) * t;

    let mut dh = to_h - from_h;
    if dh > 180.0 {
        dh -= 360.0;
    } else if dh < -180.0 {
        dh += 360.0;
    }
    let h = (from_h + dh * t).rem_euclid(360.0);

    Color::oklch(l, c, h)
}

fn color_to_oklch(color: &Color) -> (f32, f32, f32) {
    match color {
        Color::Oklch { l, c, h, .. } => (*l, *c, *h),
        Color::Rgb { r, g, b } => {
            use palette::{IntoColor, Oklch, Srgb};
            let srgb = Srgb::new(*r as f32 / 255.0, *g as f32 / 255.0, *b as f32 / 255.0);
            let oklch: Oklch = srgb.into_color();
            (oklch.l, oklch.chroma, oklch.hue.into_positive_degrees())
        }
        Color::Var(_) | Color::Derived { .. } => (0.5, 0.0, 0.0),
    }
}

/// Collect all element IDs from the tree.
pub fn collect_element_ids(element: &Element) -> HashSet<String> {
    let mut ids = HashSet::new();
    collect_ids_recursive(element, &mut ids);
    ids
}

fn collect_ids_recursive(element: &Element, ids: &mut HashSet<String>) {
    ids.insert(element.id.clone());
    if let Content::Children(children) = &element.content {
        for child in children {
            collect_ids_recursive(child, ids);
        }
    }
}
