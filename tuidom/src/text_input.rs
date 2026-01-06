use std::collections::HashMap;

use crate::element::{find_element, Element};
use crate::event::{Event, Key, Modifiers};
use crate::layout::LayoutResult;

/// Data for a single text input: text content and cursor state.
#[derive(Debug, Clone, Default)]
pub struct TextInputData {
    pub text: String,
    pub cursor: usize,
    /// Anchor position for selection. When Some and != cursor, text is selected.
    pub anchor: Option<usize>,
}

impl TextInputData {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor = text.len();
        Self {
            text,
            cursor,
            anchor: None,
        }
    }

    /// Get the selection range as (start, end) where start <= end.
    pub fn selection(&self) -> Option<(usize, usize)> {
        self.anchor.and_then(|a| {
            if a != self.cursor {
                Some(if a < self.cursor {
                    (a, self.cursor)
                } else {
                    (self.cursor, a)
                })
            } else {
                None
            }
        })
    }

    /// Check if there's an active selection.
    pub fn has_selection(&self) -> bool {
        self.selection().is_some()
    }

    /// Clear the selection anchor.
    pub fn clear_selection(&mut self) {
        self.anchor = None;
    }

    /// Select all text.
    pub fn select_all(&mut self) {
        if !self.text.is_empty() {
            self.anchor = Some(0);
            self.cursor = self.text.len();
        }
    }
}

/// Tracks text input state for multiple elements.
#[derive(Debug, Default)]
pub struct TextInputState {
    inputs: HashMap<String, TextInputData>,
}

impl TextInputState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the text value for an input.
    pub fn get(&self, id: &str) -> &str {
        self.inputs
            .get(id)
            .map(|d| d.text.as_str())
            .unwrap_or("")
    }

    /// Get the full input data (text, cursor, selection).
    pub fn get_data(&self, id: &str) -> Option<&TextInputData> {
        self.inputs.get(id)
    }

    /// Get mutable access to input data.
    pub fn get_data_mut(&mut self, id: &str) -> &mut TextInputData {
        self.inputs.entry(id.to_string()).or_default()
    }

    /// Set the text value for an input, placing cursor at end.
    pub fn set(&mut self, id: &str, text: impl Into<String>) {
        let text = text.into();
        let cursor = text.len();
        self.inputs.insert(
            id.to_string(),
            TextInputData {
                text,
                cursor,
                anchor: None,
            },
        );
    }

    /// Process events and handle text input.
    /// Returns events that were generated (Change, Submit) or passed through.
    pub fn process_events(
        &mut self,
        events: &[Event],
        root: &Element,
        _layout: &LayoutResult,
    ) -> Vec<Event> {
        let mut output = Vec::new();

        for event in events {
            match event {
                Event::Key {
                    target: Some(target),
                    key,
                    modifiers,
                } => {
                    // Check if target element captures input
                    if let Some(element) = find_element(root, target) {
                        if element.captures_input {
                            match self.handle_key(target, *key, *modifiers) {
                                TextEditResult::Changed => {
                                    output.push(Event::Change {
                                        target: target.clone(),
                                        text: self.get(target).to_string(),
                                    });
                                    continue;
                                }
                                TextEditResult::Submitted => {
                                    output.push(Event::Submit {
                                        target: target.clone(),
                                    });
                                    continue;
                                }
                                TextEditResult::Handled => {
                                    // Cursor moved, no event needed
                                    continue;
                                }
                                TextEditResult::Ignored => {
                                    // Pass through
                                }
                            }
                        }
                    }
                    output.push(event.clone());
                }

                // TODO: Handle click to position cursor
                // TODO: Handle double-click to select word
                // TODO: Handle triple-click to select all
                _ => output.push(event.clone()),
            }
        }

        output
    }

    /// Handle a key press for text editing.
    fn handle_key(&mut self, id: &str, key: Key, modifiers: Modifiers) -> TextEditResult {
        match key {
            Key::Char(c) if modifiers.none() || (modifiers.shift && !modifiers.ctrl) => {
                self.insert_char(id, c);
                TextEditResult::Changed
            }

            Key::Backspace if modifiers.none() => {
                if self.delete_back(id) {
                    TextEditResult::Changed
                } else {
                    TextEditResult::Handled
                }
            }

            Key::Delete if modifiers.none() => {
                if self.delete_forward(id) {
                    TextEditResult::Changed
                } else {
                    TextEditResult::Handled
                }
            }

            Key::Left if !modifiers.ctrl => {
                self.move_cursor(id, -1, modifiers.shift);
                TextEditResult::Handled
            }

            Key::Right if !modifiers.ctrl => {
                self.move_cursor(id, 1, modifiers.shift);
                TextEditResult::Handled
            }

            Key::Home if !modifiers.ctrl => {
                self.move_to_start(id, modifiers.shift);
                TextEditResult::Handled
            }

            Key::End if !modifiers.ctrl => {
                self.move_to_end(id, modifiers.shift);
                TextEditResult::Handled
            }

            Key::Char('a') if modifiers.ctrl => {
                let data = self.get_data_mut(id);
                data.select_all();
                TextEditResult::Handled
            }

            Key::Enter => TextEditResult::Submitted,

            _ => TextEditResult::Ignored,
        }
    }

    /// Insert a character at cursor, replacing selection if any.
    fn insert_char(&mut self, id: &str, c: char) {
        let data = self.get_data_mut(id);

        if let Some((start, end)) = data.selection() {
            // Replace selection
            let mut new_text = String::with_capacity(data.text.len() - (end - start) + c.len_utf8());
            new_text.push_str(&data.text[..start]);
            new_text.push(c);
            new_text.push_str(&data.text[end..]);
            data.text = new_text;
            data.cursor = start + c.len_utf8();
            data.clear_selection();
        } else {
            // Insert at cursor
            let byte_pos = char_to_byte_index(&data.text, data.cursor);
            data.text.insert(byte_pos, c);
            data.cursor += 1;
        }
    }

    /// Delete character before cursor or delete selection.
    /// Returns true if text changed.
    fn delete_back(&mut self, id: &str) -> bool {
        let data = self.get_data_mut(id);

        if let Some((start, end)) = data.selection() {
            // Delete selection
            let mut new_text = String::with_capacity(data.text.len() - (end - start));
            new_text.push_str(&data.text[..start]);
            new_text.push_str(&data.text[end..]);
            data.text = new_text;
            data.cursor = start;
            data.clear_selection();
            true
        } else if data.cursor > 0 {
            // Delete char before cursor
            let char_indices: Vec<_> = data.text.char_indices().collect();
            if data.cursor <= char_indices.len() {
                let byte_pos = if data.cursor == char_indices.len() {
                    data.text.len()
                } else {
                    char_indices[data.cursor].0
                };
                let prev_byte_pos = char_indices[data.cursor - 1].0;
                data.text = format!("{}{}", &data.text[..prev_byte_pos], &data.text[byte_pos..]);
                data.cursor -= 1;
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Delete character after cursor or delete selection.
    /// Returns true if text changed.
    fn delete_forward(&mut self, id: &str) -> bool {
        let data = self.get_data_mut(id);

        if let Some((start, end)) = data.selection() {
            // Delete selection
            let mut new_text = String::with_capacity(data.text.len() - (end - start));
            new_text.push_str(&data.text[..start]);
            new_text.push_str(&data.text[end..]);
            data.text = new_text;
            data.cursor = start;
            data.clear_selection();
            true
        } else {
            let char_count = data.text.chars().count();
            if data.cursor < char_count {
                // Delete char at cursor
                let char_indices: Vec<_> = data.text.char_indices().collect();
                let byte_pos = char_indices[data.cursor].0;
                let next_byte_pos = if data.cursor + 1 == char_indices.len() {
                    data.text.len()
                } else {
                    char_indices[data.cursor + 1].0
                };
                data.text = format!("{}{}", &data.text[..byte_pos], &data.text[next_byte_pos..]);
                true
            } else {
                false
            }
        }
    }

    /// Move cursor by delta characters.
    fn move_cursor(&mut self, id: &str, delta: i32, extend_selection: bool) {
        let data = self.get_data_mut(id);
        let char_count = data.text.chars().count();

        if extend_selection && data.anchor.is_none() {
            data.anchor = Some(data.cursor);
        } else if !extend_selection {
            // If we have a selection and not extending, move to edge of selection
            if let Some((start, end)) = data.selection() {
                data.cursor = if delta < 0 { start } else { end };
                data.clear_selection();
                return;
            }
            data.clear_selection();
        }

        let new_pos = (data.cursor as i32 + delta).clamp(0, char_count as i32) as usize;
        data.cursor = new_pos;
    }

    /// Move cursor to start of text.
    fn move_to_start(&mut self, id: &str, extend_selection: bool) {
        let data = self.get_data_mut(id);

        if extend_selection && data.anchor.is_none() {
            data.anchor = Some(data.cursor);
        } else if !extend_selection {
            data.clear_selection();
        }

        data.cursor = 0;
    }

    /// Move cursor to end of text.
    fn move_to_end(&mut self, id: &str, extend_selection: bool) {
        let data = self.get_data_mut(id);
        let char_count = data.text.chars().count();

        if extend_selection && data.anchor.is_none() {
            data.anchor = Some(data.cursor);
        } else if !extend_selection {
            data.clear_selection();
        }

        data.cursor = char_count;
    }
}

/// Result of handling a text editing key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEditResult {
    /// Text was modified.
    Changed,
    /// Enter was pressed.
    Submitted,
    /// Key was handled but text didn't change (e.g., cursor movement).
    Handled,
    /// Key was not handled, should be passed through.
    Ignored,
}

/// Convert character index to byte index in a string.
fn char_to_byte_index(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}
