use std::time::Duration;

/// Configuration for a single property transition.
#[derive(Debug, Clone, Copy)]
pub struct TransitionConfig {
    pub duration: Duration,
    pub easing: Easing,
}

impl TransitionConfig {
    pub fn new(duration: Duration, easing: Easing) -> Self {
        Self { duration, easing }
    }
}

/// Easing function for transitions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Easing {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

impl Easing {
    /// Apply easing to progress (0.0 to 1.0).
    pub fn apply(self, t: f32) -> f32 {
        match self {
            Easing::Linear => t,
            Easing::EaseIn => t * t,
            Easing::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Easing::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
        }
    }
}

/// Transitions configuration for an element.
/// Similar to Style, this is a builder for configuring property transitions.
#[derive(Debug, Clone, Default)]
pub struct Transitions {
    /// Transition for X position changes (from left, right, or layout reflow).
    pub x: Option<TransitionConfig>,
    /// Transition for Y position changes (from top, bottom, or layout reflow).
    pub y: Option<TransitionConfig>,
    pub width: Option<TransitionConfig>,
    pub height: Option<TransitionConfig>,
    pub background: Option<TransitionConfig>,
    pub foreground: Option<TransitionConfig>,
}

impl Transitions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set transition for X position changes.
    /// This animates horizontal position changes from any source (left, right, or layout reflow).
    pub fn x(mut self, duration: Duration, easing: Easing) -> Self {
        self.x = Some(TransitionConfig::new(duration, easing));
        self
    }

    /// Set transition for Y position changes.
    /// This animates vertical position changes from any source (top, bottom, or layout reflow).
    pub fn y(mut self, duration: Duration, easing: Easing) -> Self {
        self.y = Some(TransitionConfig::new(duration, easing));
        self
    }

    /// Set transition for both X and Y position changes.
    pub fn position(self, duration: Duration, easing: Easing) -> Self {
        self.x(duration, easing).y(duration, easing)
    }

    pub fn width(mut self, duration: Duration, easing: Easing) -> Self {
        self.width = Some(TransitionConfig::new(duration, easing));
        self
    }

    pub fn height(mut self, duration: Duration, easing: Easing) -> Self {
        self.height = Some(TransitionConfig::new(duration, easing));
        self
    }

    /// Set transition for size (width, height).
    pub fn size(self, duration: Duration, easing: Easing) -> Self {
        self.width(duration, easing).height(duration, easing)
    }

    pub fn background(mut self, duration: Duration, easing: Easing) -> Self {
        self.background = Some(TransitionConfig::new(duration, easing));
        self
    }

    pub fn foreground(mut self, duration: Duration, easing: Easing) -> Self {
        self.foreground = Some(TransitionConfig::new(duration, easing));
        self
    }

    /// Set transition for colors (background, foreground).
    pub fn colors(self, duration: Duration, easing: Easing) -> Self {
        self.background(duration, easing)
            .foreground(duration, easing)
    }

    /// Set transition for all properties.
    pub fn all(self, duration: Duration, easing: Easing) -> Self {
        self.position(duration, easing)
            .size(duration, easing)
            .colors(duration, easing)
    }

    /// Returns true if any transition is configured.
    pub fn has_any(&self) -> bool {
        self.x.is_some()
            || self.y.is_some()
            || self.width.is_some()
            || self.height.is_some()
            || self.background.is_some()
            || self.foreground.is_some()
    }
}
