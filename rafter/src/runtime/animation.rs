//! Animation system for smooth property transitions.
//!
//! Animations are unified under a single type with a `repeat` field:
//! - `repeat: Some(1)` = transition (play once, jump to end on blur)
//! - `repeat: Some(n)` = play n times then stop
//! - `repeat: None` = loop forever (pause on blur, resume on foreground)

use std::time::{Duration, Instant};

/// Easing function for animations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Easing {
    /// Linear interpolation (constant speed)
    #[default]
    Linear,
    /// Ease in (slow start, fast end)
    EaseIn,
    /// Ease out (fast start, slow end)
    EaseOut,
    /// Ease in-out (slow start and end)
    EaseInOut,
}

impl Easing {
    /// Apply easing function to a normalized time value (0.0 to 1.0).
    ///
    /// Returns the eased value (also 0.0 to 1.0).
    pub fn apply(&self, t: f32) -> f32 {
        match self {
            Easing::Linear => t,
            Easing::EaseIn => t * t,
            Easing::EaseOut => t * (2.0 - t),
            Easing::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
        }
    }
}

/// Property being animated.
#[derive(Debug, Clone, PartialEq)]
pub enum AnimatedProperty {
    /// Opacity (0.0 to 1.0)
    Opacity { from: f32, to: f32 },
    /// Background color lightness (OKLCH L component, 0.0 to 1.0)
    BackgroundLightness { from: f32, to: f32 },
    /// Foreground color lightness (OKLCH L component, 0.0 to 1.0)
    ForegroundLightness { from: f32, to: f32 },
}

impl AnimatedProperty {
    /// Interpolate the property at the given progress (0.0 to 1.0).
    ///
    /// Returns a value that can be applied to the style.
    pub fn interpolate(&self, progress: f32) -> AnimatedValue {
        match self {
            AnimatedProperty::Opacity { from, to } => {
                let value = from + (to - from) * progress;
                AnimatedValue::Opacity(value)
            }
            AnimatedProperty::BackgroundLightness { from, to } => {
                let value = from + (to - from) * progress;
                AnimatedValue::BackgroundLightness(value)
            }
            AnimatedProperty::ForegroundLightness { from, to } => {
                let value = from + (to - from) * progress;
                AnimatedValue::ForegroundLightness(value)
            }
        }
    }

    /// Get the final value of this property (at progress = 1.0).
    pub fn final_value(&self) -> AnimatedValue {
        self.interpolate(1.0)
    }
}

/// Interpolated animation value ready to apply to a style.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimatedValue {
    Opacity(f32),
    BackgroundLightness(f32),
    ForegroundLightness(f32),
}

/// A single animation targeting one property of one widget.
///
/// Animations are unified with a `repeat` field:
/// - `repeat: Some(1)` = transition (play once)
/// - `repeat: Some(n)` = play n times then stop
/// - `repeat: None` = loop forever (e.g., spinner)
///
/// Blur behavior is derived from repeat:
/// - Finite repeat: remove animation (property jumps to final value)
/// - Infinite repeat: pause animation (resume on foreground)
#[derive(Debug, Clone)]
pub struct Animation {
    /// Widget ID being animated
    pub widget_id: String,
    /// Property being animated
    pub property: AnimatedProperty,
    /// Duration of one animation cycle
    pub duration: Duration,
    /// Easing function
    pub easing: Easing,
    /// When the animation started (adjusted on resume)
    pub start_time: Instant,
    /// Number of times to repeat (None = infinite loop)
    pub repeat: Option<u32>,
    /// Elapsed time when paused (for pause/resume)
    paused_elapsed: Option<Duration>,
}

impl Animation {
    /// Create a new animation that plays once (transition).
    pub fn transition(
        widget_id: impl Into<String>,
        property: AnimatedProperty,
        duration: Duration,
        easing: Easing,
    ) -> Self {
        Self {
            widget_id: widget_id.into(),
            property,
            duration,
            easing,
            start_time: Instant::now(),
            repeat: Some(1),
            paused_elapsed: None,
        }
    }

    /// Create a new animation that loops forever.
    pub fn looping(
        widget_id: impl Into<String>,
        property: AnimatedProperty,
        duration: Duration,
        easing: Easing,
    ) -> Self {
        Self {
            widget_id: widget_id.into(),
            property,
            duration,
            easing,
            start_time: Instant::now(),
            repeat: None,
            paused_elapsed: None,
        }
    }

    /// Set the number of times to repeat.
    pub fn with_repeat(mut self, count: Option<u32>) -> Self {
        self.repeat = count;
        self
    }

    /// Check if this is a looping (infinite) animation.
    pub fn is_looping(&self) -> bool {
        self.repeat.is_none()
    }

    /// Check if the animation is paused.
    pub fn is_paused(&self) -> bool {
        self.paused_elapsed.is_some()
    }

    /// Pause the animation (stores current elapsed time).
    pub fn pause(&mut self) {
        if self.paused_elapsed.is_none() {
            self.paused_elapsed = Some(self.start_time.elapsed());
        }
    }

    /// Resume the animation (restores timing from pause point).
    pub fn resume(&mut self) {
        if let Some(elapsed) = self.paused_elapsed.take() {
            self.start_time = Instant::now() - elapsed;
        }
    }

    /// Get the elapsed time (accounting for pause state).
    fn elapsed(&self) -> Duration {
        self.paused_elapsed
            .unwrap_or_else(|| self.start_time.elapsed())
    }

    /// Check if the animation is complete.
    ///
    /// Looping animations never complete.
    pub fn is_complete(&self) -> bool {
        match self.repeat {
            None => false, // Looping animations never complete
            Some(n) => {
                let cycles = self.elapsed().as_secs_f32() / self.duration.as_secs_f32();
                cycles >= n as f32
            }
        }
    }

    /// Get the current progress within the current cycle (0.0 to 1.0).
    pub fn progress(&self) -> f32 {
        let elapsed = self.elapsed().as_secs_f32();
        let cycle = elapsed / self.duration.as_secs_f32();

        match self.repeat {
            None => cycle % 1.0, // Loop forever
            Some(n) => {
                if cycle >= n as f32 {
                    1.0 // Completed all repeats
                } else {
                    cycle % 1.0
                }
            }
        }
    }

    /// Get the current interpolated value.
    pub fn current_value(&self) -> AnimatedValue {
        let raw_progress = self.progress();
        let eased_progress = self.easing.apply(raw_progress);
        self.property.interpolate(eased_progress)
    }

    /// Get the completion time of this animation.
    ///
    /// Returns None for looping animations (they never complete).
    pub fn completion_time(&self) -> Option<Instant> {
        self.repeat.map(|n| self.start_time + self.duration * n)
    }
}

/// Manager for all active animations.
#[derive(Debug, Default)]
pub struct AnimationManager {
    /// Active animations
    animations: Vec<Animation>,
    /// Whether reduce_motion is enabled (instant completion)
    reduce_motion: bool,
}

impl AnimationManager {
    /// Create a new animation manager.
    pub fn new(reduce_motion: bool) -> Self {
        Self {
            animations: Vec::new(),
            reduce_motion,
        }
    }

    /// Start a new animation (or replace existing animation for same widget+property).
    pub fn start(&mut self, animation: Animation) {
        // If reduce_motion is enabled, don't add the animation
        // (properties will jump to final value)
        if self.reduce_motion {
            return;
        }

        // Remove any existing animation for the same widget and property type
        self.animations.retain(|a| {
            !(a.widget_id == animation.widget_id
                && std::mem::discriminant(&a.property) == std::mem::discriminant(&animation.property))
        });

        // Add the new animation
        self.animations.push(animation);
    }

    /// Remove completed animations (finite repeat only).
    ///
    /// Returns the number of animations removed.
    pub fn cleanup_completed(&mut self) -> usize {
        let before = self.animations.len();
        self.animations.retain(|a| !a.is_complete());
        before - self.animations.len()
    }

    /// Check if there are any active (non-paused) animations.
    pub fn has_active(&self) -> bool {
        self.animations.iter().any(|a| !a.is_paused())
    }

    /// Get the next animation completion time (earliest).
    ///
    /// Only considers finite-repeat animations (looping ones never complete).
    pub fn next_completion_time(&self) -> Option<Instant> {
        self.animations
            .iter()
            .filter_map(|a| a.completion_time())
            .min()
    }

    /// Handle app blur: pause looping animations, remove finite ones.
    ///
    /// Finite animations are removed so their properties jump to final value.
    /// Looping animations are paused so they can resume on foreground.
    pub fn on_blur(&mut self) {
        // Pause looping animations
        for anim in &mut self.animations {
            if anim.is_looping() {
                anim.pause();
            }
        }

        // Remove finite animations (they'll render at final value)
        self.animations.retain(|a| a.is_looping());
    }

    /// Handle app foreground: resume paused animations.
    pub fn on_foreground(&mut self) {
        for anim in &mut self.animations {
            anim.resume();
        }
    }

    /// Stop all animations for a widget.
    pub fn stop(&mut self, widget_id: &str) {
        self.animations.retain(|a| a.widget_id != widget_id);
    }

    /// Stop a specific animation by widget ID and property type.
    pub fn stop_property(&mut self, widget_id: &str, property: &AnimatedProperty) {
        self.animations.retain(|a| {
            !(a.widget_id == widget_id
                && std::mem::discriminant(&a.property) == std::mem::discriminant(property))
        });
    }

    /// Check if a widget has an active animation for a property.
    pub fn has_animation(&self, widget_id: &str, property: &AnimatedProperty) -> bool {
        self.animations.iter().any(|a| {
            a.widget_id == widget_id
                && std::mem::discriminant(&a.property) == std::mem::discriminant(property)
        })
    }

    /// Get current animated values for a widget.
    ///
    /// Returns a list of current interpolated values.
    pub fn get_values(&self, widget_id: &str) -> Vec<AnimatedValue> {
        self.animations
            .iter()
            .filter(|a| a.widget_id == widget_id)
            .map(|a| a.current_value())
            .collect()
    }

    /// Get all animations (for debugging).
    pub fn all(&self) -> &[Animation] {
        &self.animations
    }

    /// Remove animations for widgets that are no longer rendered.
    ///
    /// Called after each render with the set of widget IDs that were rendered.
    /// Animations for removed widgets are stopped immediately.
    pub fn cleanup_removed_widgets<'a>(&mut self, rendered_ids: impl Iterator<Item = &'a str>) {
        let rendered_set: std::collections::HashSet<&str> = rendered_ids.collect();
        self.animations
            .retain(|a| rendered_set.contains(a.widget_id.as_str()));
    }

    /// Convenience: Fade in a widget (opacity 0 -> 1).
    pub fn fade_in(&mut self, widget_id: impl Into<String>, duration: Duration) {
        self.start(Animation::transition(
            widget_id,
            AnimatedProperty::Opacity { from: 0.0, to: 1.0 },
            duration,
            Easing::EaseOut,
        ));
    }

    /// Convenience: Fade out a widget (opacity 1 -> 0).
    pub fn fade_out(&mut self, widget_id: impl Into<String>, duration: Duration) {
        self.start(Animation::transition(
            widget_id,
            AnimatedProperty::Opacity { from: 1.0, to: 0.0 },
            duration,
            Easing::EaseIn,
        ));
    }
}
