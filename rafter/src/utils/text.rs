//! Text utilities for wrapping and formatting.

/// Wrap text to a given width, respecting existing line breaks.
///
/// Uses word-wrapping: breaks at word boundaries when possible,
/// and breaks long words that exceed the width.
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![];
    }

    let mut result = Vec::new();

    for line in text.lines() {
        if line.is_empty() {
            result.push(String::new());
            continue;
        }

        let words: Vec<&str> = line.split_whitespace().collect();
        if words.is_empty() {
            result.push(String::new());
            continue;
        }

        let mut current_line = String::new();
        for word in words {
            if current_line.is_empty() {
                if word.len() > width {
                    // Word is longer than width, break it
                    let mut remaining = word;
                    while remaining.len() > width {
                        result.push(remaining[..width].to_string());
                        remaining = &remaining[width..];
                    }
                    if !remaining.is_empty() {
                        current_line = remaining.to_string();
                    }
                } else {
                    current_line = word.to_string();
                }
            } else if current_line.len() + 1 + word.len() <= width {
                // Word fits on current line
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                // Start new line
                result.push(current_line);
                if word.len() > width {
                    // Word is longer than width, break it
                    let mut remaining = word;
                    while remaining.len() > width {
                        result.push(remaining[..width].to_string());
                        remaining = &remaining[width..];
                    }
                    current_line = remaining.to_string();
                } else {
                    current_line = word.to_string();
                }
            }
        }

        if !current_line.is_empty() {
            result.push(current_line);
        }
    }

    result
}
