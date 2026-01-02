use std::time::Duration;

use crate::buffer::Buffer;
use crate::layout::Rect;

#[derive(Default)]
pub enum Content {
    #[default]
    None,
    Text(String),
    Children(Vec<super::Element>),
    Custom(Box<dyn CustomContent>),
    /// Animated frames - cycles through children at the specified interval.
    /// Only the current frame is laid out and rendered.
    Frames {
        children: Vec<super::Element>,
        interval: Duration,
    },
}

impl std::fmt::Debug for Content {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Text(s) => write!(f, "Text({s:?})"),
            Self::Children(c) => write!(f, "Children({c:?})"),
            Self::Custom(_) => write!(f, "Custom(...)"),
            Self::Frames { children, interval } => {
                write!(f, "Frames({} frames, {:?})", children.len(), interval)
            }
        }
    }
}

pub trait CustomContent: Send + Sync {
    fn render(&self, area: Rect, buf: &mut Buffer);
    fn intrinsic_size(&self) -> (u16, u16);
}
