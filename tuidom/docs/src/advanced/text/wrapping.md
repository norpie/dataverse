# Text Wrapping

Detailed behavior of text wrapping modes.

## TextWrap::NoWrap

Text renders on a single line without wrapping:

```rust
Element::text("This is a long line that will not wrap")
    .text_wrap(TextWrap::NoWrap)
```

If the element has a fixed width with `Overflow::Hidden`, text is clipped:

```rust
Element::text("Very long text...")
    .width(Size::Fixed(10))
    .overflow(Overflow::Hidden)
    .text_wrap(TextWrap::NoWrap)
```

## TextWrap::WordWrap

Wraps at word boundaries (spaces), preserving words when possible:

```rust
Element::text("The quick brown fox jumps over the lazy dog")
    .width(Size::Fixed(15))
    .text_wrap(TextWrap::WordWrap)
```

Result:
```
The quick brown
fox jumps over
the lazy dog
```

### Long Words

Words longer than the available width are broken with character wrap:

```rust
Element::text("supercalifragilisticexpialidocious")
    .width(Size::Fixed(10))
    .text_wrap(TextWrap::WordWrap)
```

Result:
```
supercalif
ragilistic
expialidoc
ious
```

### Preserving Newlines

Explicit newlines in the input are preserved:

```rust
Element::text("Line one\nLine two\nLine three")
    .text_wrap(TextWrap::WordWrap)
```

## TextWrap::CharWrap

Wraps at any character position, ignoring word boundaries:

```rust
Element::text("Hello, world!")
    .width(Size::Fixed(5))
    .text_wrap(TextWrap::CharWrap)
```

Result:
```
Hello
, wor
ld!
```

Use for:
- Fixed-width displays
- Non-word content (paths, codes)
- When word boundaries aren't meaningful

## TextWrap::Truncate

Cuts text at the width limit and adds an ellipsis:

```rust
Element::text("This is a very long piece of text")
    .width(Size::Fixed(15))
    .text_wrap(TextWrap::Truncate)
```

Result:
```
This is a ve…
```

### Ellipsis Handling

- The ellipsis (`…`) takes 1 cell
- Text is truncated to `width - 1` to fit the ellipsis
- If width is 0, returns empty string
- If text fits, no ellipsis is added

## Combining with Alignment

Text wrapping and alignment work together:

```rust
Element::text("Short\nMedium text\nA longer line here")
    .width(Size::Fixed(20))
    .text_wrap(TextWrap::WordWrap)
    .text_align(TextAlign::Center)
```

Each wrapped line is aligned independently.

## Multi-line Input

All wrap modes handle multi-line input (with `\n`):

```rust
let text = "First paragraph with some text.\n\nSecond paragraph.";

Element::text(text)
    .width(Size::Fixed(20))
    .text_wrap(TextWrap::WordWrap)
```

Empty lines are preserved.
