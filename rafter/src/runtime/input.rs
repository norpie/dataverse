//! Keybind matching and input handling.

use std::time::{Duration, Instant};

use log::{debug, trace};

use crate::keybinds::{HandlerId, KeyCombo, Keybinds};

/// Timeout for key sequences (e.g., "gg")
const SEQUENCE_TIMEOUT: Duration = Duration::from_millis(500);

/// Input state for tracking key sequences
pub struct InputState {
    /// Current key sequence buffer
    sequence: Vec<KeyCombo>,
    /// When the sequence started
    sequence_start: Option<Instant>,
}

impl InputState {
    /// Create a new input state
    pub fn new() -> Self {
        Self {
            sequence: Vec::new(),
            sequence_start: None,
        }
    }

    /// Process a key press and check for matching keybinds.
    /// Returns the handler ID if a match is found.
    pub fn process_key(&mut self, key: KeyCombo, keybinds: &Keybinds) -> KeybindMatch {
        debug!("Processing key: {:?}", key);

        // Check if sequence has timed out
        if let Some(start) = self.sequence_start
            && start.elapsed() > SEQUENCE_TIMEOUT
        {
            debug!("Sequence timed out, clearing");
            self.sequence.clear();
            self.sequence_start = None;
        }

        // Add key to sequence
        self.sequence.push(key.clone());
        if self.sequence_start.is_none() {
            self.sequence_start = Some(Instant::now());
        }

        debug!("Current sequence: {:?}", self.sequence);

        // Try to match against keybinds
        let mut exact_match: Option<HandlerId> = None;
        let mut prefix_match = false;

        for bind in keybinds.all() {
            trace!(
                "Comparing sequence {:?} with bind {:?}",
                self.sequence, bind.keys
            );
            if bind.keys == self.sequence {
                // Exact match
                debug!("Exact match found: {:?}", bind.handler);
                exact_match = Some(bind.handler.clone());
            } else if bind.keys.len() > self.sequence.len()
                && bind.keys[..self.sequence.len()] == self.sequence[..]
            {
                // This sequence is a prefix of a longer binding
                debug!("Prefix match for {:?}", bind.handler);
                prefix_match = true;
            }
        }

        if let Some(handler) = exact_match {
            // Found a match - clear sequence and return handler
            self.sequence.clear();
            self.sequence_start = None;
            KeybindMatch::Match(handler)
        } else if prefix_match {
            // Waiting for more keys
            KeybindMatch::Pending
        } else {
            // No match possible - clear sequence
            debug!("No match found, clearing sequence");
            self.sequence.clear();
            self.sequence_start = None;
            KeybindMatch::NoMatch
        }
    }

    /// Clear any pending sequence
    #[allow(dead_code)]
    pub fn clear_sequence(&mut self) {
        self.sequence.clear();
        self.sequence_start = None;
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of attempting to match a keybind
#[derive(Debug, Clone)]
pub enum KeybindMatch {
    /// A keybind was matched
    Match(HandlerId),
    /// Sequence is pending (waiting for more keys)
    Pending,
    /// No match found
    NoMatch,
}
