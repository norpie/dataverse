use crate::buffer::Buffer;
use crate::layout::Rect;

pub enum Content {
    None,
    Text(String),
    Children(Vec<super::Element>),
    Custom(Box<dyn CustomContent>),
}

impl Default for Content {
    fn default() -> Self {
        Self::None
    }
}

impl std::fmt::Debug for Content {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Text(s) => write!(f, "Text({s:?})"),
            Self::Children(c) => write!(f, "Children({c:?})"),
            Self::Custom(_) => write!(f, "Custom(...)"),
        }
    }
}

pub trait CustomContent: Send + Sync {
    fn render(&self, area: Rect, buf: &mut Buffer);
    fn intrinsic_size(&self) -> (u16, u16);
}
