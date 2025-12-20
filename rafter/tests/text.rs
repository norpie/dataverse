use rafter::utils::text::wrap_text;

#[test]
fn test_wrap_short_text() {
    let result = wrap_text("hello world", 20);
    assert_eq!(result, vec!["hello world"]);
}

#[test]
fn test_wrap_at_word_boundary() {
    let result = wrap_text("hello world", 8);
    assert_eq!(result, vec!["hello", "world"]);
}

#[test]
fn test_wrap_long_word() {
    let result = wrap_text("supercalifragilistic", 5);
    assert_eq!(result, vec!["super", "calif", "ragil", "istic"]);
}

#[test]
fn test_wrap_preserves_empty_lines() {
    let result = wrap_text("hello\n\nworld", 20);
    assert_eq!(result, vec!["hello", "", "world"]);
}

#[test]
fn test_wrap_zero_width() {
    let result = wrap_text("hello", 0);
    assert!(result.is_empty());
}

#[test]
fn test_wrap_multiple_words_per_line() {
    let result = wrap_text("the quick brown fox jumps", 15);
    assert_eq!(result, vec!["the quick brown", "fox jumps"]);
}

#[test]
fn test_wrap_exact_fit() {
    let result = wrap_text("hello", 5);
    assert_eq!(result, vec!["hello"]);
}
