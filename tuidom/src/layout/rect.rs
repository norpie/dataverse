#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    pub const fn new(x: i16, y: i16, width: u16, height: u16) -> Self {
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

    pub const fn left(&self) -> i16 {
        self.x
    }

    pub const fn right(&self) -> i16 {
        self.x + self.width as i16
    }

    pub const fn top(&self) -> i16 {
        self.y
    }

    pub const fn bottom(&self) -> i16 {
        self.y + self.height as i16
    }

    pub fn shrink(self, top: u16, right: u16, bottom: u16, left: u16) -> Self {
        let x = self.x.saturating_add(left as i16);
        let y = self.y.saturating_add(top as i16);
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
        x as i16 >= self.x
            && (x as i16) < self.right()
            && y as i16 >= self.y
            && (y as i16) < self.bottom()
    }

    /// Intersect this rect with an optional clip rect.
    /// If `other` is None, returns self unchanged.
    /// Returns the overlapping area (may be empty/zero-sized).
    pub fn intersect(self, other: Option<Rect>) -> Rect {
        match other {
            None => self,
            Some(clip) => {
                let x = self.x.max(clip.x);
                let y = self.y.max(clip.y);
                let right = self.right().min(clip.right());
                let bottom = self.bottom().min(clip.bottom());
                Rect::new(x, y, (right - x).max(0) as u16, (bottom - y).max(0) as u16)
            }
        }
    }

    /// Get the center point of this rectangle.
    pub const fn center(&self) -> (i16, i16) {
        (
            self.x + self.width as i16 / 2,
            self.y + self.height as i16 / 2,
        )
    }
}
