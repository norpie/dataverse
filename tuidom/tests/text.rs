use tuidom::text::{
    align_offset, char_width, display_width, truncate_to_width, wrap_chars, wrap_words,
};
use tuidom::TextAlign;

#[test]
fn test_display_width_ascii() {
    assert_eq!(display_width("hello"), 5);
    assert_eq!(display_width(""), 0);
    assert_eq!(display_width("a b c"), 5);
}

#[test]
fn test_display_width_cjk() {
    // CJK characters are typically 2 cells wide
    assert_eq!(display_width("日本語"), 6);
    assert_eq!(display_width("テスト"), 6);
    assert_eq!(display_width("한글"), 4);
}

#[test]
fn test_display_width_mixed() {
    assert_eq!(display_width("hello日本語"), 11); // 5 + 6
    assert_eq!(display_width("a日b"), 4); // 1 + 2 + 1
}

#[test]
fn test_display_width_emoji() {
    // Basic emoji are typically 2 cells wide
    assert_eq!(display_width("😀"), 2);
}

#[test]
fn test_char_width() {
    assert_eq!(char_width('a'), 1);
    assert_eq!(char_width('日'), 2);
    assert_eq!(char_width('😀'), 2);
}

#[test]
fn test_truncate_fits() {
    assert_eq!(truncate_to_width("hello", 10), "hello");
    assert_eq!(truncate_to_width("hello", 5), "hello");
}

#[test]
fn test_truncate_overflow() {
    assert_eq!(truncate_to_width("hello world", 8), "hello w…");
    assert_eq!(truncate_to_width("hello", 3), "he…");
}

#[test]
fn test_truncate_edge_cases() {
    assert_eq!(truncate_to_width("hello", 1), "…");
    assert_eq!(truncate_to_width("hello", 0), "");
    assert_eq!(truncate_to_width("", 5), "");
}

#[test]
fn test_truncate_cjk() {
    // "日本語" is 6 cells wide
    // With max_width=5, we need to fit ellipsis (1) + chars (max 4)
    // "日本" is 4 cells, so result should be "日本…"
    assert_eq!(truncate_to_width("日本語", 5), "日本…");
}

#[test]
fn test_wrap_words_simple() {
    let lines = wrap_words("hello world", 20);
    assert_eq!(lines, vec!["hello world"]);
}

#[test]
fn test_wrap_words_breaks() {
    let lines = wrap_words("hello world foo bar", 11);
    assert_eq!(lines, vec!["hello world", "foo bar"]);
}

#[test]
fn test_wrap_words_single_word_too_long() {
    let lines = wrap_words("superlongword", 5);
    // Should break the word using char wrap
    assert_eq!(lines, vec!["super", "longw", "ord"]);
}

#[test]
fn test_wrap_words_preserves_newlines() {
    let lines = wrap_words("line1\nline2", 20);
    assert_eq!(lines, vec!["line1", "line2"]);
}

#[test]
fn test_wrap_words_empty() {
    let lines = wrap_words("", 10);
    assert_eq!(lines, vec![""]);
}

#[test]
fn test_wrap_chars_simple() {
    let lines = wrap_chars("hello", 3);
    assert_eq!(lines, vec!["hel", "lo"]);
}

#[test]
fn test_wrap_chars_exact() {
    let lines = wrap_chars("hello", 5);
    assert_eq!(lines, vec!["hello"]);
}

#[test]
fn test_wrap_chars_cjk() {
    // "日本語" is 6 cells, each char is 2 cells
    let lines = wrap_chars("日本語", 4);
    assert_eq!(lines, vec!["日本", "語"]);
}

#[test]
fn test_wrap_chars_preserves_newlines() {
    let lines = wrap_chars("ab\ncd", 10);
    assert_eq!(lines, vec!["ab", "cd"]);
}

#[test]
fn test_align_offset_left() {
    assert_eq!(align_offset(5, 10, TextAlign::Left), 0);
}

#[test]
fn test_align_offset_center() {
    assert_eq!(align_offset(4, 10, TextAlign::Center), 3);
    assert_eq!(align_offset(5, 10, TextAlign::Center), 2);
}

#[test]
fn test_align_offset_right() {
    assert_eq!(align_offset(5, 10, TextAlign::Right), 5);
}

#[test]
fn test_align_offset_text_wider_than_available() {
    // If text is wider than available, offset should be 0
    assert_eq!(align_offset(15, 10, TextAlign::Center), 0);
    assert_eq!(align_offset(15, 10, TextAlign::Right), 0);
}
