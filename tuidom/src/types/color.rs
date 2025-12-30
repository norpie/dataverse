#[derive(Debug, Clone, PartialEq)]
pub enum Color {
    Oklch { l: f32, c: f32, h: f32, a: f32 },
    Rgb { r: u8, g: u8, b: u8 },
    Var(String),
    Derived { base: Box<Color>, ops: Vec<ColorOp> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ColorOp {
    Lighten(f32),
    Darken(f32),
    Saturate(f32),
    Desaturate(f32),
    HueShift(f32),
    Alpha(f32),
    Mix(Color, f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// OKLCH color representation for perceptually uniform operations.
/// Colors are stored in this format throughout the rendering pipeline,
/// only converting to RGB at terminal flush time.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Oklch {
    pub l: f32, // lightness 0.0-1.0
    pub c: f32, // chroma 0.0-~0.4
    pub h: f32, // hue 0.0-360.0
}

impl Oklch {
    pub const fn new(l: f32, c: f32, h: f32) -> Self {
        Self { l, c, h }
    }

    /// Darken the color by reducing lightness.
    /// `amount` of 0.0 = no change, 1.0 = fully black.
    pub fn darken(self, amount: f32) -> Self {
        Self {
            l: (self.l * (1.0 - amount)).max(0.0),
            ..self
        }
    }

    /// Desaturate the color by reducing chroma.
    /// `amount` of 0.0 = no change, 1.0 = fully grayscale.
    pub fn desaturate(self, amount: f32) -> Self {
        Self {
            c: (self.c * (1.0 - amount)).max(0.0),
            ..self
        }
    }

    /// Convert to RGB for terminal output.
    pub fn to_rgb(self) -> Rgb {
        use palette::{IntoColor, Oklch as PaletteOklch, Srgb};
        let oklch = PaletteOklch::new(self.l, self.c, self.h);
        let srgb: Srgb = oklch.into_color();
        let (r, g, b) = srgb.into_format::<u8>().into_components();
        Rgb::new(r, g, b)
    }

    /// Convert from RGB.
    pub fn from_rgb(rgb: Rgb) -> Self {
        use palette::{IntoColor, Oklch as PaletteOklch, Srgb};
        let srgb = Srgb::new(
            rgb.r as f32 / 255.0,
            rgb.g as f32 / 255.0,
            rgb.b as f32 / 255.0,
        );
        let oklch: PaletteOklch = srgb.into_color();
        Self::new(oklch.l, oklch.chroma, oklch.hue.into_positive_degrees())
    }
}

/// Key for caching Color → Oklch conversions.
/// Uses quantized values for f32 hashing.
#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub enum ColorKey {
    Oklch(i32, i32, i32), // quantized l, c, h
    Rgb(u8, u8, u8),
    Var(String),
}

impl Color {
    pub fn oklch(l: f32, c: f32, h: f32) -> Self {
        Self::Oklch { l, c, h, a: 1.0 }
    }

    pub fn oklcha(l: f32, c: f32, h: f32, a: f32) -> Self {
        Self::Oklch { l, c, h, a }
    }

    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::Rgb { r, g, b }
    }

    pub fn var(name: impl Into<String>) -> Self {
        Self::Var(name.into())
    }

    pub fn lighten(self, amount: f32) -> Self {
        self.with_op(ColorOp::Lighten(amount))
    }

    pub fn darken(self, amount: f32) -> Self {
        self.with_op(ColorOp::Darken(amount))
    }

    pub fn saturate(self, amount: f32) -> Self {
        self.with_op(ColorOp::Saturate(amount))
    }

    pub fn desaturate(self, amount: f32) -> Self {
        self.with_op(ColorOp::Desaturate(amount))
    }

    pub fn hue_shift(self, degrees: f32) -> Self {
        self.with_op(ColorOp::HueShift(degrees))
    }

    pub fn alpha(self, a: f32) -> Self {
        self.with_op(ColorOp::Alpha(a))
    }

    pub fn mix(self, other: Color, amount: f32) -> Self {
        self.with_op(ColorOp::Mix(other, amount))
    }

    fn with_op(self, op: ColorOp) -> Self {
        match self {
            Self::Derived { base, mut ops } => {
                ops.push(op);
                Self::Derived { base, ops }
            }
            other => Self::Derived {
                base: Box::new(other),
                ops: vec![op],
            },
        }
    }

    pub fn to_dsl(&self) -> String {
        match self {
            Self::Oklch { l, c, h, a } => {
                if *a >= 1.0 {
                    format!("oklch({l}, {c}, {h})")
                } else {
                    format!("oklch({l}, {c}, {h}, {a})")
                }
            }
            Self::Rgb { r, g, b } => format!("rgb({r}, {g}, {b})"),
            Self::Var(name) => name.clone(),
            Self::Derived { base, ops } => {
                let mut s = base.to_dsl();
                for op in ops {
                    s.push_str(" | ");
                    s.push_str(&op.to_dsl());
                }
                s
            }
        }
    }

    pub fn to_rgb(&self) -> Rgb {
        match self {
            Self::Rgb { r, g, b } => Rgb::new(*r, *g, *b),
            Self::Oklch { l, c, h, .. } => oklch_to_rgb(*l, *c, *h),
            Self::Var(_) => Rgb::default(), // needs ColorContext to resolve
            Self::Derived { .. } => Rgb::default(), // needs ColorContext to resolve
        }
    }

    /// Convert to OKLCH representation for rendering pipeline.
    pub fn to_oklch(&self) -> Oklch {
        match self {
            Self::Oklch { l, c, h, .. } => Oklch::new(*l, *c, *h),
            Self::Rgb { r, g, b } => Oklch::from_rgb(Rgb::new(*r, *g, *b)),
            Self::Var(_) => Oklch::default(),          // needs ColorContext to resolve
            Self::Derived { .. } => Oklch::default(),  // TODO: resolve ops
        }
    }

    /// Generate a cache key for this color.
    /// Returns None for Derived colors which can't be cached without resolution.
    pub fn cache_key(&self) -> Option<ColorKey> {
        match self {
            Self::Oklch { l, c, h, .. } => Some(ColorKey::Oklch(
                (l * 1000.0) as i32,
                (c * 10000.0) as i32,
                (h * 10.0) as i32,
            )),
            Self::Rgb { r, g, b } => Some(ColorKey::Rgb(*r, *g, *b)),
            Self::Var(s) => Some(ColorKey::Var(s.clone())),
            Self::Derived { .. } => None, // derived colors not cached
        }
    }
}

impl ColorOp {
    pub fn to_dsl(&self) -> String {
        match self {
            Self::Lighten(v) => format!("lighten({v})"),
            Self::Darken(v) => format!("darken({v})"),
            Self::Saturate(v) => format!("saturate({v})"),
            Self::Desaturate(v) => format!("desaturate({v})"),
            Self::HueShift(v) => format!("hue({v})"),
            Self::Alpha(v) => format!("alpha({v})"),
            Self::Mix(color, amount) => format!("mix({}, {amount})", color.to_dsl()),
        }
    }
}

fn oklch_to_rgb(l: f32, c: f32, h: f32) -> Rgb {
    use palette::{IntoColor, Oklch, Srgb};

    let oklch = Oklch::new(l, c, h);
    let srgb: Srgb = oklch.into_color();
    let (r, g, b) = srgb.into_format::<u8>().into_components();

    Rgb::new(r, g, b)
}
