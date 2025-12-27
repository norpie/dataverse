use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

use crate::types::TextAlign;

pub fn display_width(s: &str) -> usize {
    s.width()
}

pub fn char_width(c: char) -> usize {
    c.width().unwrap_or(0)
}

pub fn truncate_to_width(s: &str, max_width: usize) -> String {
    let current_width = display_width(s);
    if current_width <= max_width {
        return s.to_string();
    }

    if max_width == 0 {
        return String::new();
    }

    let ellipsis = "…";
    let ellipsis_width = 1;
    let target_width = max_width.saturating_sub(ellipsis_width);

    let mut result = String::new();
    let mut width = 0;

    for ch in s.chars() {
        let ch_width = char_width(ch);
        if width + ch_width > target_width {
            break;
        }
        result.push(ch);
        width += ch_width;
    }

    result.push_str(ellipsis);
    result
}

pub fn wrap_words(s: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![];
    }

    let mut lines = Vec::new();

    for input_line in s.split('\n') {
        if input_line.is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current_line = String::new();
        let mut current_width = 0;

        for word in input_line.split_whitespace() {
            let word_width = display_width(word);

            if word_width > max_width {
                // Word is longer than max width, need to break it
                if !current_line.is_empty() {
                    lines.push(current_line);
                    current_line = String::new();
                    current_width = 0;
                }

                // Break the long word using char wrap
                let broken = wrap_chars(word, max_width);
                let broken_len = broken.len();
                for (i, part) in broken.into_iter().enumerate() {
                    if i < broken_len - 1 {
                        lines.push(part);
                    } else {
                        current_line = part;
                        current_width = display_width(&current_line);
                    }
                }
                continue;
            }

            let space_width = if current_line.is_empty() { 0 } else { 1 };
            let needed_width = space_width + word_width;

            if current_width + needed_width > max_width {
                // Start new line
                if !current_line.is_empty() {
                    lines.push(current_line);
                }
                current_line = word.to_string();
                current_width = word_width;
            } else {
                // Add to current line
                if !current_line.is_empty() {
                    current_line.push(' ');
                    current_width += 1;
                }
                current_line.push_str(word);
                current_width += word_width;
            }
        }

        if !current_line.is_empty() {
            lines.push(current_line);
        } else if input_line.chars().all(|c| c.is_whitespace()) && !input_line.is_empty() {
            // Preserve lines that are only whitespace as empty lines
            lines.push(String::new());
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

pub fn wrap_chars(s: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![];
    }

    let mut lines = Vec::new();

    for input_line in s.split('\n') {
        if input_line.is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current_line = String::new();
        let mut current_width = 0;

        for ch in input_line.chars() {
            let ch_width = char_width(ch);

            if ch_width == 0 {
                // Zero-width char (combining, etc.) - just add it
                current_line.push(ch);
                continue;
            }

            if current_width + ch_width > max_width {
                // Start new line
                if !current_line.is_empty() {
                    lines.push(current_line);
                }
                current_line = String::new();
                current_width = 0;
            }

            current_line.push(ch);
            current_width += ch_width;
        }

        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

pub fn align_offset(text_width: usize, available_width: usize, align: TextAlign) -> usize {
    if text_width >= available_width {
        return 0;
    }

    match align {
        TextAlign::Left => 0,
        TextAlign::Center => (available_width - text_width) / 2,
        TextAlign::Right => available_width - text_width,
    }
}
