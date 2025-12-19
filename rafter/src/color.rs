use color::{AlphaColor, Hsl, Oklch, Srgb};
use ratatui::style::Color as RatatuiColor;

/// A color value that can be defined in multiple color spaces.
/// Converts to ratatui colors at render time.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Color {
    /// OKLCH color space (lightness, chroma, hue)
    /// Lightness: 0.0-1.0, Chroma: 0.0-0.4+, Hue: 0-360
    Oklch {
        l: f32,
        c: f32,
        h: f32,
    },

    /// HSL color space (hue, saturation, lightness)
    /// Hue: 0-360, Saturation: 0.0-1.0, Lightness: 0.0-1.0
    Hsl {
        h: f32,
        s: f32,
        l: f32,
    },

    /// RGB color space
    Rgb {
        r: u8,
        g: u8,
        b: u8,
    },

    /// Hex color (stored as RGB)
    Hex(u32),

    /// ANSI 256 color
    Indexed(u8),

    /// Named theme color (resolved at render time)
    Named(String),

    /// Basic ANSI colors
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,

    /// Reset to default
    #[default]
    Reset,
}

impl Color {
    /// Create an OKLCH color
    pub const fn oklch(l: f32, c: f32, h: f32) -> Self {
        Self::Oklch { l, c, h }
    }

    /// Create an HSL color
    pub const fn hsl(h: f32, s: f32, l: f32) -> Self {
        Self::Hsl { h, s, l }
    }

    /// Create an RGB color
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::Rgb { r, g, b }
    }

    /// Create a color from hex value (0xRRGGBB)
    pub const fn hex(value: u32) -> Self {
        Self::Hex(value)
    }

    /// Parse a CSS color string
    pub fn parse(s: &str) -> Option<Self> {
        let parsed = color::parse_color(s).ok()?;
        let srgb: AlphaColor<Srgb> = parsed.to_alpha_color();
        let [r, g, b, _] = srgb.components;
        Some(Self::Rgb {
            r: (r.clamp(0.0, 1.0) * 255.0) as u8,
            g: (g.clamp(0.0, 1.0) * 255.0) as u8,
            b: (b.clamp(0.0, 1.0) * 255.0) as u8,
        })
    }

    /// Convert to RGB tuple
    pub fn to_rgb(self) -> (u8, u8, u8) {
        match self {
            Self::Rgb { r, g, b } => (r, g, b),
            Self::Hex(value) => {
                let r = ((value >> 16) & 0xFF) as u8;
                let g = ((value >> 8) & 0xFF) as u8;
                let b = (value & 0xFF) as u8;
                (r, g, b)
            }
            Self::Oklch { l, c, h } => oklch_to_rgb(l, c, h),
            Self::Hsl { h, s, l } => hsl_to_rgb(h, s, l),
            Self::Black => (0, 0, 0),
            Self::Red => (128, 0, 0),
            Self::Green => (0, 128, 0),
            Self::Yellow => (128, 128, 0),
            Self::Blue => (0, 0, 128),
            Self::Magenta => (128, 0, 128),
            Self::Cyan => (0, 128, 128),
            Self::White => (192, 192, 192),
            Self::BrightBlack => (128, 128, 128),
            Self::BrightRed => (255, 0, 0),
            Self::BrightGreen => (0, 255, 0),
            Self::BrightYellow => (255, 255, 0),
            Self::BrightBlue => (0, 0, 255),
            Self::BrightMagenta => (255, 0, 255),
            Self::BrightCyan => (0, 255, 255),
            Self::BrightWhite => (255, 255, 255),
            Self::Indexed(i) => indexed_to_rgb(i),
            Self::Named(_) => (255, 255, 255), // Named colors resolved at render time
            Self::Reset => (255, 255, 255),
        }
    }

    /// Parse a hex color string like "#FF0000" or "FF0000"
    pub fn from_hex(s: &str) -> Option<Self> {
        let s = s.trim_start_matches('#');
        let value = u32::from_str_radix(s, 16).ok()?;
        Some(Self::Hex(value))
    }

    /// Convert to ratatui color
    pub fn to_ratatui(self) -> RatatuiColor {
        match self {
            Self::Rgb { r, g, b } => RatatuiColor::Rgb(r, g, b),
            Self::Hex(value) => {
                let r = ((value >> 16) & 0xFF) as u8;
                let g = ((value >> 8) & 0xFF) as u8;
                let b = (value & 0xFF) as u8;
                RatatuiColor::Rgb(r, g, b)
            }
            Self::Oklch { l, c, h } => {
                let (r, g, b) = oklch_to_rgb(l, c, h);
                RatatuiColor::Rgb(r, g, b)
            }
            Self::Hsl { h, s, l } => {
                let (r, g, b) = hsl_to_rgb(h, s, l);
                RatatuiColor::Rgb(r, g, b)
            }
            Self::Indexed(i) => RatatuiColor::Indexed(i),
            Self::Black => RatatuiColor::Black,
            Self::Red => RatatuiColor::Red,
            Self::Green => RatatuiColor::Green,
            Self::Yellow => RatatuiColor::Yellow,
            Self::Blue => RatatuiColor::Blue,
            Self::Magenta => RatatuiColor::Magenta,
            Self::Cyan => RatatuiColor::Cyan,
            Self::White => RatatuiColor::White,
            Self::BrightBlack => RatatuiColor::DarkGray,
            Self::BrightRed => RatatuiColor::LightRed,
            Self::BrightGreen => RatatuiColor::LightGreen,
            Self::BrightYellow => RatatuiColor::LightYellow,
            Self::BrightBlue => RatatuiColor::LightBlue,
            Self::BrightMagenta => RatatuiColor::LightMagenta,
            Self::BrightCyan => RatatuiColor::LightCyan,
            Self::BrightWhite => RatatuiColor::White,
            Self::Named(_) => RatatuiColor::Reset, // Named colors resolved at render time
            Self::Reset => RatatuiColor::Reset,
        }
    }
}

/// Convert OKLCH to RGB using the color crate
fn oklch_to_rgb(l: f32, c: f32, h: f32) -> (u8, u8, u8) {
    let oklch = AlphaColor::<Oklch>::new([l, c, h, 1.0]);
    let srgb: AlphaColor<Srgb> = oklch.convert();
    let [r, g, b, _] = srgb.components;
    (
        (r.clamp(0.0, 1.0) * 255.0) as u8,
        (g.clamp(0.0, 1.0) * 255.0) as u8,
        (b.clamp(0.0, 1.0) * 255.0) as u8,
    )
}

/// Convert HSL to RGB using the color crate
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let hsl = AlphaColor::<Hsl>::new([h, s, l, 1.0]);
    let srgb: AlphaColor<Srgb> = hsl.convert();
    let [r, g, b, _] = srgb.components;
    (
        (r.clamp(0.0, 1.0) * 255.0) as u8,
        (g.clamp(0.0, 1.0) * 255.0) as u8,
        (b.clamp(0.0, 1.0) * 255.0) as u8,
    )
}

/// Convert 256-color index to approximate RGB
fn indexed_to_rgb(i: u8) -> (u8, u8, u8) {
    match i {
        0 => (0, 0, 0),
        1 => (128, 0, 0),
        2 => (0, 128, 0),
        3 => (128, 128, 0),
        4 => (0, 0, 128),
        5 => (128, 0, 128),
        6 => (0, 128, 128),
        7 => (192, 192, 192),
        8 => (128, 128, 128),
        9 => (255, 0, 0),
        10 => (0, 255, 0),
        11 => (255, 255, 0),
        12 => (0, 0, 255),
        13 => (255, 0, 255),
        14 => (0, 255, 255),
        15 => (255, 255, 255),
        16..=231 => {
            // 6x6x6 color cube
            let i = i - 16;
            let r = (i / 36) % 6;
            let g = (i / 6) % 6;
            let b = i % 6;
            let r = if r == 0 { 0 } else { 55 + r * 40 };
            let g = if g == 0 { 0 } else { 55 + g * 40 };
            let b = if b == 0 { 0 } else { 55 + b * 40 };
            (r, g, b)
        }
        232..=255 => {
            // Grayscale
            let g = 8 + (i - 232) * 10;
            (g, g, g)
        }
    }
}
