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
    ///
    /// The `current_view` parameter is used to filter keybinds by scope.
    /// View-scoped keybinds take priority over global keybinds.
    pub fn process_key(
        &mut self,
        key: KeyCombo,
        keybinds: &Keybinds,
        current_view: Option<&str>,
    ) -> KeybindMatch {
        debug!("Processing key: {:?} (view: {:?})", key, current_view);

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

        // Try to match against keybinds (view-scoped first, then global)
        let mut view_exact_match: Option<HandlerId> = None;
        let mut global_exact_match: Option<HandlerId> = None;
        let mut prefix_match = false;

        for bind in keybinds.all() {
            // Skip keybinds that aren't active for the current view
            if !bind.is_active_for(current_view) {
                continue;
            }

            // Get current keys (skip if disabled)
            let Some(keys) = bind.current_keys() else {
                continue;
            };

            trace!(
                "Comparing sequence {:?} with bind {:?}",
                self.sequence, keys
            );
            if keys == self.sequence {
                // Exact match - prefer view-scoped over global
                debug!(
                    "Exact match found: {:?} (scope: {:?})",
                    bind.handler, bind.scope
                );
                if bind.scope != crate::keybinds::KeybindScope::Global {
                    view_exact_match = Some(bind.handler.clone());
                } else if global_exact_match.is_none() {
                    global_exact_match = Some(bind.handler.clone());
                }
            } else if keys.len() > self.sequence.len()
                && keys[..self.sequence.len()] == self.sequence[..]
            {
                // This sequence is a prefix of a longer binding
                debug!("Prefix match for {:?}", bind.handler);
                prefix_match = true;
            }
        }

        // View-scoped matches take priority
        let exact_match = view_exact_match.or(global_exact_match);

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
