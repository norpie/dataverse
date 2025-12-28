use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::element::{Content, Element};
use crate::transitions::{Easing, TransitionConfig};
use crate::types::{Color, Size};

/// Which property is being transitioned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransitionProperty {
    Left,
    Top,
    Right,
    Bottom,
    Width,
    Height,
    Background,
    Foreground,
}

/// A property value that can be interpolated.
#[derive(Debug, Clone)]
pub enum PropertyValue {
    I16(i16),
    U16(u16),
    Color(Color),
}

/// Snapshot of an element's transitionable properties.
#[derive(Debug, Clone, Default)]
struct ElementSnapshot {
    left: Option<i16>,
    top: Option<i16>,
    right: Option<i16>,
    bottom: Option<i16>,
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

/// Manages animation state across frames.
#[derive(Debug, Default)]
pub struct AnimationState {
    /// Previous frame's property values per element.
    snapshots: HashMap<String, ElementSnapshot>,
    /// Currently active transitions: (element_id, property) -> transition.
    active: HashMap<(String, TransitionProperty), ActiveTransition>,
    /// Reduced motion flag - when true, transitions complete instantly.
    reduced_motion: bool,
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

    /// Returns true if any transition is currently active.
    pub fn has_active_transitions(&self) -> bool {
        !self.active.is_empty()
    }

    /// Update animation state based on current element tree.
    /// Detects property changes, starts new transitions, and prunes completed ones.
    pub fn update(&mut self, root: &Element) {
        let now = Instant::now();

        // Prune completed transitions
        self.active.retain(|_, transition| {
            now.duration_since(transition.start) < transition.duration
        });

        // Walk tree and check for property changes
        self.update_element(root, now);
    }

    fn update_element(&mut self, element: &Element, now: Instant) {
        let id = &element.id;
        let current = Self::snapshot_element(element);
        let transitions = &element.transitions;

        // Compare with previous snapshot and start transitions
        if let Some(prev) = self.snapshots.get(id).cloned() {
            self.check_and_start_i16_transition(
                id,
                TransitionProperty::Left,
                prev.left,
                current.left,
                transitions.left,
                now,
            );
            self.check_and_start_i16_transition(
                id,
                TransitionProperty::Top,
                prev.top,
                current.top,
                transitions.top,
                now,
            );
            self.check_and_start_i16_transition(
                id,
                TransitionProperty::Right,
                prev.right,
                current.right,
                transitions.right,
                now,
            );
            self.check_and_start_i16_transition(
                id,
                TransitionProperty::Bottom,
                prev.bottom,
                current.bottom,
                transitions.bottom,
                now,
            );
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
        if let Content::Children(children) = &element.content {
            for child in children {
                self.update_element(child, now);
            }
        }
    }

    fn check_and_start_i16_transition(
        &mut self,
        id: &str,
        property: TransitionProperty,
        prev: Option<i16>,
        current: Option<i16>,
        config: Option<TransitionConfig>,
        now: Instant,
    ) {
        let Some(config) = config else { return };
        let Some(prev_val) = prev else { return };
        let Some(curr_val) = current else { return };

        if prev_val == curr_val {
            return;
        }

        // Skip if reduced motion is enabled
        if self.reduced_motion {
            return;
        }

        let key = (id.to_string(), property);

        // Check if there's already an active transition for this property
        let from = if let Some(existing) = self.active.get(&key) {
            // Transition from current interpolated value
            self.interpolate_value(&existing.from, &existing.to, existing.start, existing.duration, existing.easing, now)
        } else {
            PropertyValue::I16(prev_val)
        };

        self.active.insert(
            key,
            ActiveTransition {
                from,
                to: PropertyValue::I16(curr_val),
                start: now,
                duration: config.duration,
                easing: config.easing,
            },
        );
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

        if prev_val == curr_val {
            return;
        }

        // Skip if reduced motion is enabled
        if self.reduced_motion {
            return;
        }

        let key = (id.to_string(), property);

        // Check if there's already an active transition for this property
        let from = if let Some(existing) = self.active.get(&key) {
            // Transition from current interpolated value
            self.interpolate_value(&existing.from, &existing.to, existing.start, existing.duration, existing.easing, now)
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

        if prev_color == curr_color {
            return;
        }

        // Skip if reduced motion is enabled
        if self.reduced_motion {
            return;
        }

        let key = (id.to_string(), property);

        // Check if there's already an active transition for this property
        let from = if let Some(existing) = self.active.get(&key) {
            // Transition from current interpolated value
            self.interpolate_value(&existing.from, &existing.to, existing.start, existing.duration, existing.easing, now)
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
            left: element.left,
            top: element.top,
            right: element.right,
            bottom: element.bottom,
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
    /// Returns None if no active transition for this property.
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
            (PropertyValue::I16(from_val), PropertyValue::I16(to_val)) => {
                PropertyValue::I16(lerp_i16(*from_val, *to_val, eased))
            }
            (PropertyValue::U16(from_val), PropertyValue::U16(to_val)) => {
                PropertyValue::U16(lerp_u16(*from_val, *to_val, eased))
            }
            (PropertyValue::Color(from_color), PropertyValue::Color(to_color)) => {
                PropertyValue::Color(lerp_color(from_color, to_color, eased))
            }
            _ => to.clone(), // Mismatched types, just use target
        }
    }

    /// Remove transitions and snapshots for elements no longer in tree.
    pub fn cleanup(&mut self, current_ids: &HashSet<String>) {
        self.snapshots.retain(|id, _| current_ids.contains(id));
        self.active.retain(|(id, _), _| current_ids.contains(id));
    }
}

/// Linear interpolation for i16 values.
fn lerp_i16(from: i16, to: i16, t: f32) -> i16 {
    let from = from as f32;
    let to = to as f32;
    (from + (to - from) * t).round() as i16
}

/// Linear interpolation for u16 values.
fn lerp_u16(from: u16, to: u16, t: f32) -> u16 {
    let from = from as f32;
    let to = to as f32;
    (from + (to - from) * t).round() as u16
}

/// Interpolate colors in OKLCH space.
fn lerp_color(from: &Color, to: &Color, t: f32) -> Color {
    // Extract OKLCH values, converting if necessary
    let (from_l, from_c, from_h) = color_to_oklch(from);
    let (to_l, to_c, to_h) = color_to_oklch(to);

    // Interpolate L and C linearly
    let l = from_l + (to_l - from_l) * t;
    let c = from_c + (to_c - from_c) * t;

    // Hue interpolation (shortest path around the circle)
    let mut dh = to_h - from_h;
    if dh > 180.0 {
        dh -= 360.0;
    } else if dh < -180.0 {
        dh += 360.0;
    }
    let h = (from_h + dh * t).rem_euclid(360.0);

    Color::oklch(l, c, h)
}

/// Extract OKLCH values from a color.
fn color_to_oklch(color: &Color) -> (f32, f32, f32) {
    match color {
        Color::Oklch { l, c, h, .. } => (*l, *c, *h),
        Color::Rgb { r, g, b } => {
            // Convert RGB to OKLCH using palette crate
            use palette::{IntoColor, Oklch, Srgb};
            let srgb = Srgb::new(*r as f32 / 255.0, *g as f32 / 255.0, *b as f32 / 255.0);
            let oklch: Oklch = srgb.into_color();
            (
                oklch.l,
                oklch.chroma,
                oklch.hue.into_positive_degrees(),
            )
        }
        // For Var and Derived, we can't interpolate without resolving them first
        // Return neutral values that won't cause issues
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
