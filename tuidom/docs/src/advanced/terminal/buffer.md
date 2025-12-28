# Buffer System

The `Buffer` and `Cell` types are the low-level rendering primitives.

## Buffer

A 2D grid of cells representing the terminal screen.

### Creating a Buffer

```rust
let buf = Buffer::new(80, 24);  // 80 columns × 24 rows
```

### Methods

#### `width() -> u16` / `height() -> u16`

Get buffer dimensions:

```rust
let w = buf.width();
let h = buf.height();
```

#### `get(x, y) -> Option<&Cell>`

Read a cell:

```rust
if let Some(cell) = buf.get(10, 5) {
    println!("Character: {}", cell.char);
}
```

#### `get_mut(x, y) -> Option<&mut Cell>`

Modify a cell:

```rust
if let Some(cell) = buf.get_mut(10, 5) {
    cell.char = '█';
    cell.fg = Rgb::new(255, 0, 0);
}
```

#### `set(x, y, cell)`

Replace a cell entirely:

```rust
buf.set(10, 5, Cell::new('X').with_fg(Rgb::new(0, 255, 0)));
```

#### `clear()`

Reset all cells to default:

```rust
buf.clear();
```

#### `diff(other) -> Iterator`

Get cells that differ from another buffer:

```rust
for (x, y, cell) in current.diff(&previous) {
    // Only changed cells
}
```

## Cell

A single terminal cell with character, colors, and style.

### Structure

```rust
pub struct Cell {
    pub char: char,
    pub fg: Rgb,
    pub bg: Rgb,
    pub style: TextStyle,
    pub wide_continuation: bool,
}
```

### Creating Cells

```rust
// Basic cell
let cell = Cell::new('A');

// With colors
let cell = Cell::new('A')
    .with_fg(Rgb::new(255, 255, 255))
    .with_bg(Rgb::new(0, 0, 0));

// With style
let cell = Cell::new('A')
    .with_style(TextStyle::new().bold());
```

### Default Cell

```rust
Cell::default()
// char: ' '
// fg: white (255, 255, 255)
// bg: black (0, 0, 0)
// style: normal
// wide_continuation: false
```

### Wide Continuation

Wide characters (CJK, emoji) occupy two cells. The second cell is marked:

```rust
// For '中' at position (5, 0):
buf.get(5, 0).char = '中';           // The character
buf.get(6, 0).wide_continuation = true;  // Continuation marker
```

The continuation cell is skipped during rendering.

## Usage in CustomContent

Implement custom rendering by writing to the buffer:

```rust
impl CustomContent for MyWidget {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                if let Some(cell) = buf.get_mut(x, y) {
                    cell.char = '▒';
                    cell.fg = Rgb::new(100, 100, 100);
                }
            }
        }
    }

    fn intrinsic_size(&self) -> (u16, u16) {
        (10, 5)
    }
}
```

## Performance

- Cells are stored in a flat `Vec<Cell>`
- Index calculation: `y * width + x`
- Comparison uses `PartialEq` on all cell fields
- Only changed cells are written to terminal
