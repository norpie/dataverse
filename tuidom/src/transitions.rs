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
    pub left: Option<TransitionConfig>,
    pub top: Option<TransitionConfig>,
    pub right: Option<TransitionConfig>,
    pub bottom: Option<TransitionConfig>,
    pub width: Option<TransitionConfig>,
    pub height: Option<TransitionConfig>,
    pub background: Option<TransitionConfig>,
    pub foreground: Option<TransitionConfig>,
}

impl Transitions {
    pub fn new() -> Self {
        Self::default()
    }

    // Individual property setters

    pub fn left(mut self, duration: Duration, easing: Easing) -> Self {
        self.left = Some(TransitionConfig::new(duration, easing));
        self
    }

    pub fn top(mut self, duration: Duration, easing: Easing) -> Self {
        self.top = Some(TransitionConfig::new(duration, easing));
        self
    }

    pub fn right(mut self, duration: Duration, easing: Easing) -> Self {
        self.right = Some(TransitionConfig::new(duration, easing));
        self
    }

    pub fn bottom(mut self, duration: Duration, easing: Easing) -> Self {
        self.bottom = Some(TransitionConfig::new(duration, easing));
        self
    }

    pub fn width(mut self, duration: Duration, easing: Easing) -> Self {
        self.width = Some(TransitionConfig::new(duration, easing));
        self
    }

    pub fn height(mut self, duration: Duration, easing: Easing) -> Self {
        self.height = Some(TransitionConfig::new(duration, easing));
        self
    }

    pub fn background(mut self, duration: Duration, easing: Easing) -> Self {
        self.background = Some(TransitionConfig::new(duration, easing));
        self
    }

    pub fn foreground(mut self, duration: Duration, easing: Easing) -> Self {
        self.foreground = Some(TransitionConfig::new(duration, easing));
        self
    }

    // Convenience group setters

    /// Set transition for all position offsets (left, top, right, bottom).
    pub fn position(self, duration: Duration, easing: Easing) -> Self {
        self.left(duration, easing)
            .top(duration, easing)
            .right(duration, easing)
            .bottom(duration, easing)
    }

    /// Set transition for size (width, height).
    pub fn size(self, duration: Duration, easing: Easing) -> Self {
        self.width(duration, easing).height(duration, easing)
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
        self.left.is_some()
            || self.top.is_some()
            || self.right.is_some()
            || self.bottom.is_some()
            || self.width.is_some()
            || self.height.is_some()
            || self.background.is_some()
            || self.foreground.is_some()
    }
}
