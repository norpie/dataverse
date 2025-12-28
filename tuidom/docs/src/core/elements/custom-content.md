# Custom Content

The `CustomContent` trait allows you to implement specialized rendering for elements that need more control than the standard text or container elements provide.

## The CustomContent Trait

```rust
pub trait CustomContent: Send + Sync {
    fn render(&self, area: Rect, buf: &mut Buffer);
    fn intrinsic_size(&self) -> (u16, u16);
}
```

### Required Methods

#### `render(&self, area: Rect, buf: &mut Buffer)`

Called during the render phase to draw the content to the buffer.

- `area`: The rectangle allocated for this element (x, y, width, height)
- `buf`: The render buffer to write to

#### `intrinsic_size(&self) -> (u16, u16)`

Returns the natural (preferred) size of the content as `(width, height)`.

Used when the element has `Size::Auto` dimensions.

## Example: Progress Bar

```rust
use tuidom::{Buffer, Color, Element, Rect, Size};
use tuidom::element::CustomContent;
use tuidom::types::Rgb;

struct ProgressBar {
    progress: f32,        // 0.0 to 1.0
    filled_color: Rgb,
    empty_color: Rgb,
}

impl ProgressBar {
    fn new(progress: f32) -> Self {
        Self {
            progress: progress.clamp(0.0, 1.0),
            filled_color: Rgb::new(100, 200, 100),
            empty_color: Rgb::new(60, 60, 60),
        }
    }
}

impl CustomContent for ProgressBar {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let filled_width = (area.width as f32 * self.progress) as u16;

        for x in area.x..area.right().min(buf.width()) {
            let is_filled = x < area.x + filled_width;
            let char = if is_filled { '█' } else { '░' };
            let color = if is_filled { self.filled_color } else { self.empty_color };

            if let Some(cell) = buf.get_mut(x, area.y) {
                cell.char = char;
                cell.fg = color;
            }
        }
    }

    fn intrinsic_size(&self) -> (u16, u16) {
        (20, 1)  // Default: 20 cells wide, 1 row tall
    }
}

// Usage
fn progress_indicator(value: f32) -> Element {
    Element::custom(ProgressBar::new(value))
        .width(Size::Fixed(30))
        .height(Size::Fixed(1))
}
```

## Example: Sparkline

```rust
struct Sparkline {
    values: Vec<f32>,
}

impl CustomContent for Sparkline {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if self.values.is_empty() || area.width == 0 {
            return;
        }

        let max = self.values.iter().cloned().fold(0.0f32, f32::max);
        let chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

        for (i, &value) in self.values.iter().enumerate() {
            let x = area.x + i as u16;
            if x >= area.right() {
                break;
            }

            let normalized = if max > 0.0 { value / max } else { 0.0 };
            let char_idx = (normalized * 7.0) as usize;
            let char = chars[char_idx.min(7)];

            if let Some(cell) = buf.get_mut(x, area.y) {
                cell.char = char;
            }
        }
    }

    fn intrinsic_size(&self) -> (u16, u16) {
        (self.values.len() as u16, 1)
    }
}
```

## Working with the Buffer

The `Buffer` provides these methods for custom rendering:

```rust
// Get mutable reference to a cell
if let Some(cell) = buf.get_mut(x, y) {
    cell.char = '█';
    cell.fg = Rgb::new(255, 255, 255);
    cell.bg = Rgb::new(0, 0, 0);
    cell.style.bold = true;
}

// Buffer dimensions
let width = buf.width();
let height = buf.height();
```

## Respecting the Area

Always respect the `area` parameter:

```rust
fn render(&self, area: Rect, buf: &mut Buffer) {
    // Only draw within area bounds
    for y in area.y..area.bottom().min(buf.height()) {
        for x in area.x..area.right().min(buf.width()) {
            // Draw at (x, y)
        }
    }
}
```

The `Rect` type provides helper methods:

```rust
area.x          // Left edge
area.y          // Top edge
area.width      // Width
area.height     // Height
area.right()    // x + width
area.bottom()   // y + height
area.contains(x, y)  // Point containment test
```

## Thread Safety

`CustomContent` requires `Send + Sync` because elements may be constructed and rendered from different threads. Use `Arc<Mutex<T>>` for shared mutable state if needed.

## Combining with Builder Methods

Custom content elements support all standard builder methods:

```rust
Element::custom(MyChart::new(data))
    .id("main-chart")
    .width(Size::Fill)
    .height(Size::Fixed(10))
    .style(Style::new().border(Border::Rounded))
    .focusable(true)
```

The style's background and border are rendered by tuidom; your `render` method handles only the content area inside any padding and border.
