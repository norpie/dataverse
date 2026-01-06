use crossterm::event::{Event as CrosstermEvent, KeyEventKind, MouseEventKind};

use crate::element::{find_element, Content, Element};
use crate::event::{Event, Key, Modifiers, NavDirection};
use crate::hit::hit_test_focusable;
use crate::layout::{LayoutResult, Rect};

/// Tracks which element is currently focused and processes events.
#[derive(Debug, Default)]
pub struct FocusState {
    focused: Option<String>,
}

impl FocusState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the currently focused element ID.
    pub fn focused(&self) -> Option<&str> {
        self.focused.as_deref()
    }

    /// Programmatically focus an element by ID.
    /// Returns true if focus changed.
    pub fn focus(&mut self, id: &str) -> bool {
        if self.focused.as_deref() == Some(id) {
            return false;
        }
        self.focused = Some(id.to_string());
        true
    }

    /// Clear focus.
    /// Returns true if there was something focused.
    pub fn blur(&mut self) -> bool {
        if self.focused.is_some() {
            self.focused = None;
            true
        } else {
            false
        }
    }

    /// Focus the next focusable element (Tab navigation).
    /// Returns the newly focused element ID if focus changed.
    pub fn focus_next(&mut self, root: &Element) -> Option<String> {
        let focusable = collect_focusable(root);
        if focusable.is_empty() {
            return None;
        }

        let new_focus = match &self.focused {
            None => focusable[0].clone(),
            Some(current) => {
                let idx = focusable.iter().position(|id| id == current);
                match idx {
                    Some(i) => focusable[(i + 1) % focusable.len()].clone(),
                    None => focusable[0].clone(),
                }
            }
        };

        if self.focused.as_ref() != Some(&new_focus) {
            self.focused = Some(new_focus.clone());
            Some(new_focus)
        } else {
            None
        }
    }

    /// Focus the previous focusable element (Shift+Tab navigation).
    /// Returns the newly focused element ID if focus changed.
    pub fn focus_prev(&mut self, root: &Element) -> Option<String> {
        let focusable = collect_focusable(root);
        if focusable.is_empty() {
            return None;
        }

        let new_focus = match &self.focused {
            None => focusable[focusable.len() - 1].clone(),
            Some(current) => {
                let idx = focusable.iter().position(|id| id == current);
                match idx {
                    Some(0) => focusable[focusable.len() - 1].clone(),
                    Some(i) => focusable[i - 1].clone(),
                    None => focusable[focusable.len() - 1].clone(),
                }
            }
        };

        if self.focused.as_ref() != Some(&new_focus) {
            self.focused = Some(new_focus.clone());
            Some(new_focus)
        } else {
            None
        }
    }

    /// Focus the nearest focusable element in the given direction.
    /// Returns the newly focused element ID if focus changed.
    pub fn focus_direction(
        &mut self,
        direction: NavDirection,
        root: &Element,
        layout: &LayoutResult,
    ) -> Option<String> {
        let current_id = self.focused.as_ref()?;
        let current_rect = layout.get(current_id)?;

        let focusable = collect_focusable(root);

        // Find the best candidate in the given direction
        let best = focusable
            .iter()
            .filter(|id| *id != current_id)
            .filter_map(|id| {
                let rect = layout.get(id)?;
                let score = direction_score(current_rect, rect, direction)?;
                Some((id, score))
            })
            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))?;

        let new_focus = best.0.clone();
        if self.focused.as_ref() != Some(&new_focus) {
            self.focused = Some(new_focus.clone());
            Some(new_focus)
        } else {
            None
        }
    }

    /// Process raw crossterm events and produce high-level events.
    /// Focus follows mouse - hovering over a focusable element focuses it.
    pub fn process_events(
        &mut self,
        raw: &[CrosstermEvent],
        root: &Element,
        layout: &LayoutResult,
    ) -> Vec<Event> {
        let mut events = Vec::new();

        for raw_event in raw {
            match raw_event {
                CrosstermEvent::Key(key_event) => {
                    // Only process key press events (not release/repeat on some terminals)
                    if key_event.kind != KeyEventKind::Press {
                        continue;
                    }

                    let key: Key = key_event.code.into();
                    let modifiers: Modifiers = key_event.modifiers.into();

                    // Handle Tab/BackTab for focus navigation
                    if key == Key::Tab {
                        if let Some(old) = self.focused.clone() {
                            if let Some(new) = self.focus_next(root) {
                                events.push(Event::Blur { target: old });
                                events.push(Event::Focus { target: new });
                            }
                        } else if let Some(new) = self.focus_next(root) {
                            events.push(Event::Focus { target: new });
                        }
                        continue;
                    }

                    if key == Key::BackTab {
                        if let Some(old) = self.focused.clone() {
                            if let Some(new) = self.focus_prev(root) {
                                events.push(Event::Blur { target: old });
                                events.push(Event::Focus { target: new });
                            }
                        } else if let Some(new) = self.focus_prev(root) {
                            events.push(Event::Focus { target: new });
                        }
                        continue;
                    }

                    // Handle arrow keys for spatial navigation (only without modifiers)
                    // Skip if focused element captures input (for text cursor movement)
                    let focused_captures_input = self
                        .focused
                        .as_ref()
                        .and_then(|id| find_element(root, id))
                        .map(|el| el.captures_input)
                        .unwrap_or(false);

                    if modifiers.none() && !focused_captures_input {
                        let direction = match key {
                            Key::Up => Some(NavDirection::Up),
                            Key::Down => Some(NavDirection::Down),
                            Key::Left => Some(NavDirection::Left),
                            Key::Right => Some(NavDirection::Right),
                            _ => None,
                        };

                        if let Some(dir) = direction {
                            if let Some(old) = self.focused.clone() {
                                if let Some(new) = self.focus_direction(dir, root, layout) {
                                    events.push(Event::Blur { target: old });
                                    events.push(Event::Focus { target: new });
                                    continue;
                                }
                            }
                            // If no navigation happened, fall through to emit key event
                        }
                    }

                    // Regular key event
                    events.push(Event::Key {
                        target: self.focused.clone(),
                        key,
                        modifiers,
                    });
                }

                CrosstermEvent::Mouse(mouse_event) => {
                    let x = mouse_event.column;
                    let y = mouse_event.row;

                    match mouse_event.kind {
                        MouseEventKind::Down(button) => {
                            let target = crate::hit::hit_test(layout, root, x, y);
                            events.push(Event::Click {
                                target,
                                x,
                                y,
                                button: button.into(),
                            });
                        }

                        MouseEventKind::Moved => {
                            // Focus follows mouse - check if we're over a focusable element
                            if let Some(focusable_target) = hit_test_focusable(layout, root, x, y) {
                                // Only change focus if different
                                if self.focused.as_ref() != Some(&focusable_target) {
                                    if let Some(old) = self.focused.take() {
                                        events.push(Event::Blur { target: old });
                                    }
                                    self.focused = Some(focusable_target.clone());
                                    events.push(Event::Focus {
                                        target: focusable_target,
                                    });
                                }
                            }

                            events.push(Event::MouseMove { x, y });
                        }

                        MouseEventKind::ScrollUp => {
                            let target = crate::hit::hit_test(layout, root, x, y);
                            events.push(Event::Scroll {
                                target,
                                x,
                                y,
                                delta_x: 0,
                                delta_y: -1,
                            });
                        }

                        MouseEventKind::ScrollDown => {
                            let target = crate::hit::hit_test(layout, root, x, y);
                            events.push(Event::Scroll {
                                target,
                                x,
                                y,
                                delta_x: 0,
                                delta_y: 1,
                            });
                        }

                        MouseEventKind::ScrollLeft => {
                            let target = crate::hit::hit_test(layout, root, x, y);
                            events.push(Event::Scroll {
                                target,
                                x,
                                y,
                                delta_x: -1,
                                delta_y: 0,
                            });
                        }

                        MouseEventKind::ScrollRight => {
                            let target = crate::hit::hit_test(layout, root, x, y);
                            events.push(Event::Scroll {
                                target,
                                x,
                                y,
                                delta_x: 1,
                                delta_y: 0,
                            });
                        }

                        MouseEventKind::Drag(button) => {
                            let target = crate::hit::hit_test(layout, root, x, y);
                            events.push(Event::Drag {
                                target,
                                x,
                                y,
                                button: button.into(),
                            });
                        }

                        MouseEventKind::Up(button) => {
                            let target = crate::hit::hit_test(layout, root, x, y);
                            events.push(Event::Release {
                                target,
                                x,
                                y,
                                button: button.into(),
                            });
                        }
                    }
                }

                CrosstermEvent::Resize(width, height) => {
                    events.push(Event::Resize {
                        width: *width,
                        height: *height,
                    });
                }

                _ => {}
            }
        }

        events
    }
}

/// Collect all focusable element IDs in tree order.
pub fn collect_focusable(element: &Element) -> Vec<String> {
    let mut result = Vec::new();
    collect_focusable_recursive(element, &mut result);
    result
}

fn collect_focusable_recursive(element: &Element, result: &mut Vec<String>) {
    if element.focusable {
        result.push(element.id.clone());
    }
    if let Content::Children(children) = &element.content {
        for child in children {
            collect_focusable_recursive(child, result);
        }
    }
}

/// Score how good a candidate is for the given direction.
/// Lower is better. Returns None if candidate is not in the direction.
fn direction_score(from: &Rect, to: &Rect, direction: NavDirection) -> Option<f64> {
    let (from_cx, from_cy) = from.center();
    let (to_cx, to_cy) = to.center();

    // Check if candidate is in the right direction
    let in_direction = match direction {
        NavDirection::Up => to_cy < from_cy,
        NavDirection::Down => to_cy > from_cy,
        NavDirection::Left => to_cx < from_cx,
        NavDirection::Right => to_cx > from_cx,
    };

    if !in_direction {
        return None;
    }

    // Score based on:
    // 1. Primary axis distance (must move in direction)
    // 2. Secondary axis alignment (prefer aligned elements)
    let (primary_dist, secondary_dist) = match direction {
        NavDirection::Up | NavDirection::Down => (
            (to_cy as f64 - from_cy as f64).abs(),
            (to_cx as f64 - from_cx as f64).abs(),
        ),
        NavDirection::Left | NavDirection::Right => (
            (to_cx as f64 - from_cx as f64).abs(),
            (to_cy as f64 - from_cy as f64).abs(),
        ),
    };

    // Weight: primary distance + secondary distance * 0.5
    // This prefers elements that are closer and more aligned
    Some(primary_dist + secondary_dist * 0.5)
}
