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
