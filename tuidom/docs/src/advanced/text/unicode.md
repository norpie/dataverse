# Unicode & Display Width

tuidom handles Unicode text with proper width calculation.

## Display Width vs String Length

String length (`.len()` or `.chars().count()`) doesn't equal display width:

```rust
let s = "Hello";
s.len()          // 5 bytes
s.chars().count() // 5 characters
display_width(s)  // 5 cells

let s = "你好";
s.len()          // 6 bytes (3 per character)
s.chars().count() // 2 characters
display_width(s)  // 4 cells (2 per character)
```

## Character Widths

| Type | Width | Examples |
|------|-------|----------|
| ASCII | 1 | `a`, `1`, `@` |
| Latin Extended | 1 | `é`, `ñ`, `ü` |
| CJK | 2 | `中`, `日`, `한` |
| Hiragana/Katakana | 2 | `あ`, `ア` |
| Emoji | 2 | `👋`, `🎉`, `🚀` |
| Zero-width | 0 | Combining marks, ZWJ |

## Wide Characters

Wide characters (width 2) occupy two terminal cells:

```
┌─────────────┐
│Hello 你好   │
│12345 XX XX  │  ← Cell positions
└─────────────┘
```

When a wide character is partially clipped, tuidom renders a space:

```
┌─────┐
│Hell │  ← Clipped at position 5
└─────┘
```

## The `display_width` Function

Calculate the display width of any string:

```rust
use tuidom::text::display_width;

display_width("abc")      // 3
display_width("日本語")   // 6 (3 chars × 2 width)
display_width("Mix混合")  // 7 (3 + 4)
display_width("👨‍👩‍👧")  // 2 (family emoji, single grapheme)
```

## The `char_width` Function

Get width of a single character:

```rust
use tuidom::text::char_width;

char_width('a')   // 1
char_width('中')  // 2
char_width('\u{0301}')  // 0 (combining acute accent)
```

## Zero-Width Characters

Zero-width characters (combining marks, ZWJ) are handled:

```rust
let s = "e\u{0301}";  // 'e' + combining accent = é
display_width(s)      // 1 (visual width)
s.chars().count()     // 2 (character count)
```

## Practical Considerations

### Text Truncation

Truncation respects character boundaries:

```rust
truncate_to_width("Hello 世界", 8)
// "Hello 世…" (not "Hello 世") - would be 9 cells
// Returns "Hello …" (7 cells) to fit within 8
```

### Text Wrapping

Word wrap handles wide characters:

```rust
wrap_words("Hello 你好世界 World", 10)
// ["Hello 你好", "世界 World"]
```

### Buffer Operations

When writing to the buffer, wide characters mark the next cell as a continuation:

```rust
// Writing "中" at position (5, 0)
buf.set(5, 0, Cell::new('中'));
// Position (6, 0) is automatically marked as continuation
```

## Terminal Compatibility

Most modern terminals handle Unicode correctly. Some edge cases:

- **Old terminals**: May not support wide characters
- **Font issues**: Missing glyphs render as boxes
- **Emoji**: Newer emoji may have incorrect widths

tuidom uses the `unicode-width` crate which follows Unicode Standard Annex #11.
