#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn from_size(width: u16, height: u16) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    pub const fn area(&self) -> u32 {
        self.width as u32 * self.height as u32
    }

    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub const fn left(&self) -> u16 {
        self.x
    }

    pub const fn right(&self) -> u16 {
        self.x + self.width
    }

    pub const fn top(&self) -> u16 {
        self.y
    }

    pub const fn bottom(&self) -> u16 {
        self.y + self.height
    }

    pub fn shrink(self, top: u16, right: u16, bottom: u16, left: u16) -> Self {
        let x = self.x.saturating_add(left);
        let y = self.y.saturating_add(top);
        let width = self.width.saturating_sub(left + right);
        let height = self.height.saturating_sub(top + bottom);
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn contains(&self, x: u16, y: u16) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    /// Get the center point of this rectangle.
    pub const fn center(&self) -> (u16, u16) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }
}
