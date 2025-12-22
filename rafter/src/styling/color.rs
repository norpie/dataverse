//! Color system with OKLCH-first public API.
//!
//! Colors are stored internally as RGB for fast rendering, but the public API
//! encourages OKLCH usage for perceptually uniform color definitions.
//!
//! # Example
//!
//! ```rust,ignore
//! use rafter::color::Color;
//!
//! // OKLCH constructor (recommended for defining colors)
//! let primary = Color::oklch(0.7, 0.15, 250.0);
//!
//! // Other constructors available for convenience
//! let red = Color::rgb(255, 0, 0);
//! let blue = Color::hex(0x0000FF);
//! let css = Color::parse("hsl(200, 80%, 50%)").unwrap();
//! ```

use color::{AlphaColor, Hsl, Oklch, Srgb};
use ratatui::style::Color as RatatuiColor;

/// A color value stored as RGB internally.
///
/// Use the OKLCH constructor for perceptually uniform color definitions.
/// RGB is used internally for fast conversion to terminal colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
    r: u8,
    g: u8,
    b: u8,
}

// Constructors
impl Color {
    /// Create a color from OKLCH values (recommended).
    ///
    /// OKLCH provides perceptually uniform color specification:
    /// - `l`: Lightness (0.0-1.0)
    /// - `c`: Chroma (0.0-0.4+, varies by hue)
    /// - `h`: Hue (0-360 degrees)
    pub fn oklch(l: f32, c: f32, h: f32) -> Self {
        let oklch = AlphaColor::<Oklch>::new([l, c, h, 1.0]);
        let srgb: AlphaColor<Srgb> = oklch.convert();
        let [r, g, b, _] = srgb.components;
        Self {
            r: (r.clamp(0.0, 1.0) * 255.0) as u8,
            g: (g.clamp(0.0, 1.0) * 255.0) as u8,
            b: (b.clamp(0.0, 1.0) * 255.0) as u8,
        }
    }

    /// Create a color from RGB values.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Create a color from a hex value (0xRRGGBB).
    pub const fn hex(value: u32) -> Self {
        Self {
            r: ((value >> 16) & 0xFF) as u8,
            g: ((value >> 8) & 0xFF) as u8,
            b: (value & 0xFF) as u8,
        }
    }

    /// Create a color from HSL values.
    pub fn hsl(h: f32, s: f32, l: f32) -> Self {
        let hsl = AlphaColor::<Hsl>::new([h, s, l, 1.0]);
        let srgb: AlphaColor<Srgb> = hsl.convert();
        let [r, g, b, _] = srgb.components;
        Self {
            r: (r.clamp(0.0, 1.0) * 255.0) as u8,
            g: (g.clamp(0.0, 1.0) * 255.0) as u8,
            b: (b.clamp(0.0, 1.0) * 255.0) as u8,
        }
    }

    /// Parse a CSS color string.
    pub fn parse(s: &str) -> Option<Self> {
        let parsed = color::parse_color(s).ok()?;
        let srgb: AlphaColor<Srgb> = parsed.to_alpha_color();
        let [r, g, b, _] = srgb.components;
        Some(Self {
            r: (r.clamp(0.0, 1.0) * 255.0) as u8,
            g: (g.clamp(0.0, 1.0) * 255.0) as u8,
            b: (b.clamp(0.0, 1.0) * 255.0) as u8,
        })
    }
}

// Accessors
impl Color {
    /// Get the red widget.
    pub const fn r(&self) -> u8 {
        self.r
    }

    /// Get the green widget.
    pub const fn g(&self) -> u8 {
        self.g
    }

    /// Get the blue widget.
    pub const fn b(&self) -> u8 {
        self.b
    }

    /// Convert to RGB tuple.
    pub const fn to_rgb(&self) -> (u8, u8, u8) {
        (self.r, self.g, self.b)
    }

    /// Convert to ratatui Color.
    pub const fn to_ratatui(&self) -> RatatuiColor {
        RatatuiColor::Rgb(self.r, self.g, self.b)
    }
}

// Color manipulation (converts to OKLCH, manipulates, converts back)
impl Color {
    /// Get this color's OKLCH widgets.
    pub fn to_oklch(&self) -> (f32, f32, f32) {
        let srgb = AlphaColor::<Srgb>::new([
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            1.0,
        ]);
        let oklch: AlphaColor<Oklch> = srgb.convert();
        let [l, c, h, _] = oklch.components;
        (l, c, h)
    }

    /// Create a darker version of this color.
    pub fn darken(&self, amount: f32) -> Self {
        let (l, c, h) = self.to_oklch();
        Self::oklch(l * (1.0 - amount), c, h)
    }

    /// Create a lighter version of this color.
    pub fn lighten(&self, amount: f32) -> Self {
        let (l, c, h) = self.to_oklch();
        Self::oklch(l + (1.0 - l) * amount, c, h)
    }

    /// Create a less saturated version of this color.
    pub fn desaturate(&self, amount: f32) -> Self {
        let (l, c, h) = self.to_oklch();
        Self::oklch(l, c * (1.0 - amount), h)
    }

    /// Create a more saturated version of this color.
    pub fn saturate(&self, amount: f32) -> Self {
        let (l, c, h) = self.to_oklch();
        Self::oklch(l, c * (1.0 + amount), h)
    }

    /// Rotate the hue by the given degrees.
    pub fn rotate_hue(&self, degrees: f32) -> Self {
        let (l, c, h) = self.to_oklch();
        Self::oklch(l, c, (h + degrees) % 360.0)
    }

    /// Create a new color with the given lightness.
    pub fn with_lightness(&self, l: f32) -> Self {
        let (_, c, h) = self.to_oklch();
        Self::oklch(l.clamp(0.0, 1.0), c, h)
    }

    /// Create a new color with the given chroma.
    pub fn with_chroma(&self, c: f32) -> Self {
        let (l, _, h) = self.to_oklch();
        Self::oklch(l, c.max(0.0), h)
    }

    /// Create a new color with the given hue.
    pub fn with_hue(&self, h: f32) -> Self {
        let (l, c, _) = self.to_oklch();
        Self::oklch(l, c, h % 360.0)
    }
}

// Common color constants (pre-computed RGB values)
impl Color {
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);
    pub const RED: Self = Self::rgb(239, 68, 68);
    pub const GREEN: Self = Self::rgb(34, 197, 94);
    pub const BLUE: Self = Self::rgb(59, 130, 246);
    pub const YELLOW: Self = Self::rgb(250, 204, 21);
    pub const CYAN: Self = Self::rgb(6, 182, 212);
    pub const MAGENTA: Self = Self::rgb(217, 70, 239);
    pub const GRAY: Self = Self::rgb(156, 163, 175);
    pub const DARK_GRAY: Self = Self::rgb(75, 85, 99);
    pub const LIGHT_GRAY: Self = Self::rgb(209, 213, 219);
}

/// A color that references a theme value by name.
///
/// This is resolved to a concrete `Color` at render time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedColor(pub String);

impl NamedColor {
    /// Create a new named color.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

/// A style color that can be either concrete or theme-referenced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StyleColor {
    /// A concrete color value.
    Concrete(Color),
    /// A named theme color.
    Named(String),
}

impl StyleColor {
    /// Create a concrete color.
    pub fn color(c: Color) -> Self {
        Self::Concrete(c)
    }

    /// Create a named color reference.
    pub fn named(name: impl Into<String>) -> Self {
        Self::Named(name.into())
    }
}

impl From<Color> for StyleColor {
    fn from(c: Color) -> Self {
        Self::Concrete(c)
    }
}

impl From<&str> for StyleColor {
    fn from(s: &str) -> Self {
        Self::Named(s.to_string())
    }
}

impl From<String> for StyleColor {
    fn from(s: String) -> Self {
        Self::Named(s)
    }
}
