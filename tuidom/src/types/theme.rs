use crate::types::{Color, ColorOp, Oklch, Rgb};

/// A theme provides named color variables.
pub trait Theme: Send + Sync {
    /// Resolve a color variable name to a concrete color.
    /// Returns None if the variable is not defined.
    fn resolve(&self, name: &str) -> Option<&Color>;
}

/// Default empty theme that resolves nothing.
pub struct EmptyTheme;

impl Theme for EmptyTheme {
    fn resolve(&self, _name: &str) -> Option<&Color> {
        None
    }
}

/// Minimal default theme providing basic readable colors.
/// Like bare HTML defaults - ensures content is visible.
pub struct DefaultTheme {
    pub background: Color,
    pub foreground: Color,
    pub surface: Color,
    pub border: Color,
    pub primary: Color,
}

impl DefaultTheme {
    pub const fn new() -> Self {
        Self {
            background: Color::Oklch { l: 0.0, c: 0.0, h: 0.0, a: 1.0 },  // black
            foreground: Color::Oklch { l: 1.0, c: 0.0, h: 0.0, a: 1.0 }, // white
            surface: Color::Oklch { l: 0.15, c: 0.0, h: 0.0, a: 1.0 },   // dark gray
            border: Color::Oklch { l: 0.4, c: 0.0, h: 0.0, a: 1.0 },     // gray
            primary: Color::Oklch { l: 0.9, c: 0.0, h: 0.0, a: 1.0 },    // light gray
        }
    }
}

impl Default for DefaultTheme {
    fn default() -> Self {
        Self::new()
    }
}

impl Theme for DefaultTheme {
    fn resolve(&self, name: &str) -> Option<&Color> {
        match name {
            "background" => Some(&self.background),
            "foreground" => Some(&self.foreground),
            "surface" => Some(&self.surface),
            "border" => Some(&self.border),
            "primary" => Some(&self.primary),
            _ => None,
        }
    }
}

/// Context for resolving color variables and derived colors.
pub struct ColorContext<'a> {
    theme: &'a dyn Theme,
}

impl<'a> ColorContext<'a> {
    pub fn new(theme: &'a dyn Theme) -> Self {
        Self { theme }
    }

    /// Resolve a color to a concrete Color (no Var or Derived variants).
    /// Variables are looked up in the theme.
    /// Derived colors have their operations applied.
    pub fn resolve(&self, color: &Color) -> Color {
        match color {
            Color::Var(name) => {
                if let Some(resolved) = self.theme.resolve(name) {
                    // Recursively resolve in case theme returns another Var or Derived
                    self.resolve(resolved)
                } else {
                    // Unresolved variable: return transparent/default
                    Color::Rgb { r: 0, g: 0, b: 0 }
                }
            }
            Color::Derived { base, ops } => {
                // First resolve the base color
                let resolved_base = self.resolve(base);
                // Convert to Oklch for operations
                let mut oklch = self.color_to_oklch(&resolved_base);
                let mut alpha = self.color_alpha(&resolved_base);

                // Apply each operation
                for op in ops {
                    match op {
                        ColorOp::Lighten(amount) => {
                            oklch.l = (oklch.l + amount).clamp(0.0, 1.0);
                        }
                        ColorOp::Darken(amount) => {
                            oklch.l = (oklch.l - amount).clamp(0.0, 1.0);
                        }
                        ColorOp::Saturate(amount) => {
                            oklch.c = (oklch.c + amount).clamp(0.0, 0.4);
                        }
                        ColorOp::Desaturate(amount) => {
                            oklch.c = (oklch.c - amount).clamp(0.0, 0.4);
                        }
                        ColorOp::HueShift(degrees) => {
                            oklch.h = (oklch.h + degrees) % 360.0;
                            if oklch.h < 0.0 {
                                oklch.h += 360.0;
                            }
                        }
                        ColorOp::Alpha(a) => {
                            alpha = *a;
                        }
                        ColorOp::Mix(other, amount) => {
                            let other_resolved = self.resolve(other);
                            let other_oklch = self.color_to_oklch(&other_resolved);
                            oklch.l = oklch.l * (1.0 - amount) + other_oklch.l * amount;
                            oklch.c = oklch.c * (1.0 - amount) + other_oklch.c * amount;
                            // Hue interpolation needs special handling for wrap-around
                            let h_diff = other_oklch.h - oklch.h;
                            let h_diff = if h_diff > 180.0 {
                                h_diff - 360.0
                            } else if h_diff < -180.0 {
                                h_diff + 360.0
                            } else {
                                h_diff
                            };
                            oklch.h = (oklch.h + h_diff * amount) % 360.0;
                            if oklch.h < 0.0 {
                                oklch.h += 360.0;
                            }
                        }
                    }
                }

                // Return as Oklch color
                Color::Oklch {
                    l: oklch.l,
                    c: oklch.c,
                    h: oklch.h,
                    a: alpha,
                }
            }
            // Already concrete, return as-is
            Color::Oklch { .. } | Color::Rgb { .. } => color.clone(),
        }
    }

    /// Convert a concrete color to Oklch.
    fn color_to_oklch(&self, color: &Color) -> Oklch {
        match color {
            Color::Oklch { l, c, h, .. } => Oklch::new(*l, *c, *h),
            Color::Rgb { r, g, b } => Oklch::from_rgb(Rgb::new(*r, *g, *b)),
            // Should not happen after resolve(), but handle gracefully
            Color::Var(_) | Color::Derived { .. } => Oklch::default(),
        }
    }

    /// Get alpha value from a color.
    fn color_alpha(&self, color: &Color) -> f32 {
        match color {
            Color::Oklch { a, .. } => *a,
            Color::Rgb { .. } => 1.0,
            Color::Var(_) | Color::Derived { .. } => 1.0,
        }
    }
}
