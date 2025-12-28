# Text Handling

tuidom provides utilities for text wrapping, alignment, and Unicode-aware width calculation.

## Text Wrap Modes

Configure text wrapping via `.text_wrap()`:

```rust
Element::text("Long text content...")
    .width(Size::Fixed(20))
    .text_wrap(TextWrap::WordWrap)
```

### `TextWrap::NoWrap`

No wrapping. Text may extend beyond element bounds (default).

### `TextWrap::WordWrap`

Wrap at word boundaries. Long words are broken with character wrap.

### `TextWrap::CharWrap`

Wrap at any character position.

### `TextWrap::Truncate`

Cut off with ellipsis ("…") when text exceeds width.

## Text Alignment

Configure horizontal alignment via `.text_align()`:

```rust
Element::text("Centered")
    .width(Size::Fixed(30))
    .text_align(TextAlign::Center)
```

### `TextAlign::Left`

Align to the left edge (default).

### `TextAlign::Center`

Center horizontally within the element.

### `TextAlign::Right`

Align to the right edge.

## Unicode Width

tuidom uses the `unicode-width` crate for accurate width calculation. This handles:

- Wide characters (CJK): Width 2
- Regular ASCII: Width 1
- Zero-width characters: Width 0
- Emoji: Width 2 (typically)

## Utility Functions

### `display_width(s: &str) -> usize`

Get the display width of a string:

```rust
use tuidom::text::display_width;

assert_eq!(display_width("hello"), 5);
assert_eq!(display_width("你好"), 4);   // 2 chars × 2 width
assert_eq!(display_width("👋"), 2);     // Emoji width
```

### `char_width(c: char) -> usize`

Get the display width of a single character:

```rust
use tuidom::text::char_width;

assert_eq!(char_width('a'), 1);
assert_eq!(char_width('中'), 2);
```

### `truncate_to_width(s: &str, max_width: usize) -> String`

Truncate a string to fit within a width, adding ellipsis:

```rust
use tuidom::text::truncate_to_width;

let text = "Hello, world!";
let truncated = truncate_to_width(text, 8);
assert_eq!(truncated, "Hello, …");
```

### `wrap_words(s: &str, max_width: usize) -> Vec<String>`

Word-wrap text into lines:

```rust
use tuidom::text::wrap_words;

let text = "The quick brown fox jumps over the lazy dog";
let lines = wrap_words(text, 15);
// ["The quick brown", "fox jumps over", "the lazy dog"]
```

### `wrap_chars(s: &str, max_width: usize) -> Vec<String>`

Character-wrap text into lines:

```rust
use tuidom::text::wrap_chars;

let text = "abcdefghij";
let lines = wrap_chars(text, 4);
// ["abcd", "efgh", "ij"]
```

### `align_offset(text_width: usize, available: usize, align: TextAlign) -> usize`

Calculate horizontal offset for alignment:

```rust
use tuidom::text::align_offset;

let offset = align_offset(5, 20, TextAlign::Center);
assert_eq!(offset, 7);  // (20 - 5) / 2 = 7
```

## Next Steps

- [Text Wrapping](./text/wrapping.md) - Detailed wrapping behavior
- [Unicode & Display Width](./text/unicode.md) - Unicode handling details
