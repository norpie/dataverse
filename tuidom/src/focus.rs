use crossterm::event::{Event as CrosstermEvent, KeyEventKind, MouseEventKind};

use crate::element::{find_element, Content, Element};
use crate::event::{Event, Key, Modifiers, NavDirection, ScrollAction};
use crate::hit::{hit_test_focusable, hit_test_interaction_scope};
use crate::layout::{LayoutResult, Rect};
use crate::scroll::{find_scrollable_ancestor, find_scrollable_ancestor_with_type};

/// Find the active interaction scope.
/// - If current focus is inside a scope, returns the innermost scope containing it
/// - If no focus, returns the topmost (last in tree order) scope
/// - If no scopes exist, returns None (global scope)
fn find_active_scope(root: &Element, focused: Option<&str>) -> Option<String> {
    if let Some(focused_id) = focused {
        // Find the innermost scope containing the focused element
        find_scope_containing(root, focused_id)
    } else {
        // No focus - find the topmost (last in tree order) scope
        find_topmost_scope(root)
    }
}

/// Find the innermost interaction_scope element that contains the given element ID.
fn find_scope_containing(root: &Element, element_id: &str) -> Option<String> {
    fn find_recursive(
        element: &Element,
        target_id: &str,
        current_scope: Option<&str>,
    ) -> Option<String> {
        // Update current scope if this element is an interaction_scope
        let scope = if element.interaction_scope {
            Some(element.id.as_str())
        } else {
            current_scope
        };

        // Check if this is the target element
        if element.id == target_id {
            return scope.map(String::from);
        }

        // Search children
        if let Content::Children(children) = &element.content {
            for child in children {
                if let Some(found) = find_recursive(child, target_id, scope) {
                    return Some(found);
                }
            }
        }

        None
    }

    find_recursive(root, element_id, None)
}

/// Find the topmost (last in tree order) interaction_scope element.
fn find_topmost_scope(root: &Element) -> Option<String> {
    fn find_recursive(element: &Element, last_scope: &mut Option<String>) {
        if element.interaction_scope {
            *last_scope = Some(element.id.clone());
        }

        if let Content::Children(children) = &element.content {
            for child in children {
                find_recursive(child, last_scope);
            }
        }
    }

    let mut last_scope = None;
    find_recursive(root, &mut last_scope);
    last_scope
}

/// Check if an element is within (a descendant of) the given scope.
fn is_in_scope(root: &Element, element_id: &str, scope_id: &str) -> bool {
    fn find_in_scope(element: &Element, target_id: &str, scope_id: &str, in_scope: bool) -> bool {
        // Check if we've entered the scope
        let now_in_scope = in_scope || element.id == scope_id;

        // If we found the target, return whether we're in scope
        if element.id == target_id {
            return now_in_scope;
        }

        // Search children
        if let Content::Children(children) = &element.content {
            for child in children {
                if find_in_scope(child, target_id, scope_id, now_in_scope) {
                    return true;
                }
            }
        }

        false
    }

    find_in_scope(root, element_id, scope_id, false)
}

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
    /// Respects interaction scopes - navigation is constrained to the active scope.
    pub fn focus_next(&mut self, root: &Element) -> Option<String> {
        let active_scope = find_active_scope(root, self.focused.as_deref());
        let focusable = collect_focusable(root, active_scope.as_deref());
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
    /// Respects interaction scopes - navigation is constrained to the active scope.
    pub fn focus_prev(&mut self, root: &Element) -> Option<String> {
        let active_scope = find_active_scope(root, self.focused.as_deref());
        let focusable = collect_focusable(root, active_scope.as_deref());
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
    /// Higher z-index elements are prioritized (e.g., dropdown overlays).
    /// Elements within the same scrollable container are preferred to avoid
    /// jumping to frozen/fixed elements that may be visually closer.
    /// Respects interaction scopes - navigation is constrained to the active scope.
    pub fn focus_direction(
        &mut self,
        direction: NavDirection,
        root: &Element,
        layout: &LayoutResult,
    ) -> Option<String> {
        let current_id = self.focused.as_ref()?;
        // Use absolute screen position for cross-container navigation
        let current_rect = get_absolute_rect(current_id, layout, root)?;

        let active_scope = find_active_scope(root, self.focused.as_deref());
        let focusable = collect_focusable_with_z(root, active_scope.as_deref());

        // Find scrollable ancestor of current element (if any)
        let current_scrollable = find_scrollable_ancestor(root, current_id);
        log::debug!(
            "[focus_direction] current={} direction={:?} scrollable_ancestor={:?} abs_rect={:?}",
            current_id, direction, current_scrollable, current_rect
        );

        // Score candidates, preferring those in the same scrollable container
        // Score tuple: (not_same_container, negative_z_index, spatial_score)
        // - not_same_container: 0 if same container, 1 if different (prefer same)
        // - negative_z_index: higher z-index sorts first
        // - spatial_score: closer is better
        let best = focusable
            .iter()
            .filter(|(id, _)| id != current_id)
            .filter_map(|(id, z_index)| {
                // Use absolute screen position for cross-container navigation
                let rect = get_absolute_rect(id, layout, root)?;
                let spatial_score = direction_score(&current_rect, &rect, direction)?;

                // Check if candidate is in the same scrollable container
                let candidate_scrollable = find_scrollable_ancestor(root, id);
                let same_container = match (&current_scrollable, &candidate_scrollable) {
                    (Some(a), Some(b)) => a == b,
                    (None, None) => true, // Both not in scrollable = same "container"
                    _ => false,
                };
                let container_penalty = if same_container { 0 } else { 1 };

                log::debug!(
                    "[focus_direction]   candidate={} scrollable={:?} same_container={} penalty={} spatial={:.2} abs_rect={:?}",
                    id, candidate_scrollable, same_container, container_penalty, spatial_score, rect
                );

                Some((id, (container_penalty, -(*z_index as i32), spatial_score)))
            })
            .min_by(|(_, a), (_, b)| {
                a.0.cmp(&b.0)
                    .then_with(|| a.1.cmp(&b.1))
                    .then_with(|| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
            })?;

        log::debug!(
            "[focus_direction] best={} score=({}, {}, {:.2})",
            best.0, (best.1).0, (best.1).1, (best.1).2
        );

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
                                events.push(Event::Blur { target: old, new_target: Some(new.clone()) });
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
                                events.push(Event::Blur { target: old, new_target: Some(new.clone()) });
                                events.push(Event::Focus { target: new });
                            }
                        } else if let Some(new) = self.focus_prev(root) {
                            events.push(Event::Focus { target: new });
                        }
                        continue;
                    }

                    // Escape blurs focused element; only emits key event if nothing focused
                    if key == Key::Escape {
                        if let Some(old) = self.focused.take() {
                            log::debug!("[focus] Escape key pressed, blurring {} with None target", old);
                            events.push(Event::Blur { target: old, new_target: None });
                            continue;
                        }
                        // Fall through to emit key event
                    }

                    // Handle arrow keys for spatial navigation (only without modifiers)
                    // Skip Left/Right if focused element captures input (for text cursor movement)
                    // But allow Up/Down even for text inputs (they don't move cursor in single-line inputs)
                    let focused_captures_input = self
                        .focused
                        .as_ref()
                        .and_then(|id| find_element(root, id))
                        .map(|el| el.captures_input)
                        .unwrap_or(false);

                    if modifiers.none() {
                        let direction = match key {
                            Key::Up => Some(NavDirection::Up),
                            Key::Down => Some(NavDirection::Down),
                            // Left/Right only navigate when not in a text input
                            Key::Left if !focused_captures_input => Some(NavDirection::Left),
                            Key::Right if !focused_captures_input => Some(NavDirection::Right),
                            _ => None,
                        };

                        if let Some(dir) = direction {
                            if let Some(old) = self.focused.clone() {
                                if let Some(new) = self.focus_direction(dir, root, layout) {
                                    // For Up/Down in a .scrollable() element, check if navigation
                                    // would leave the scrollable container. If so, emit Key event
                                    // instead to let the widget handle boundary scrolling.
                                    if matches!(key, Key::Up | Key::Down) {
                                        if let Some((scrollable_id, is_fake)) = find_scrollable_ancestor_with_type(root, &old) {
                                            if is_fake {
                                                // Check if new target is still inside the scrollable
                                                let new_in_scrollable = find_scrollable_ancestor_with_type(root, &new)
                                                    .map(|(id, _)| id == scrollable_id)
                                                    .unwrap_or(false);

                                                if !new_in_scrollable {
                                                    // Navigation would leave the scrollable - emit Key event first
                                                    // so widget's on_key_up/on_key_down handler can fire and override focus
                                                    // Note: use `old` not `self.focused` since focus_direction mutates it
                                                    log::debug!("[focus] Up/Down would leave scrollable {}, emitting Key event for {} (focus moved to {})", scrollable_id, old, new);
                                                    events.push(Event::Key {
                                                        target: Some(old.clone()),
                                                        key,
                                                        modifiers,
                                                    });
                                                    // Also emit the focus change - if handler sets focus, post-dispatch will override
                                                    events.push(Event::Blur { target: old, new_target: Some(new.clone()) });
                                                    events.push(Event::Focus { target: new });
                                                    continue;
                                                }
                                            }
                                        }
                                    }

                                    events.push(Event::Blur { target: old, new_target: Some(new.clone()) });
                                    events.push(Event::Focus { target: new });
                                    continue;
                                }
                            }
                            // If no navigation happened, fall through to emit key event
                        }
                    }

                    // Handle scroll keys for .scrollable() elements
                    // Convert PageUp/PageDown/Home/End to Event::Scroll with action
                    if modifiers.none() {
                        let scroll_action = match key {
                            Key::PageUp => Some(ScrollAction::PageUp),
                            Key::PageDown => Some(ScrollAction::PageDown),
                            Key::Home => Some(ScrollAction::Home),
                            Key::End => Some(ScrollAction::End),
                            _ => None,
                        };

                        if let Some(action) = scroll_action {
                            // Check if there's a .scrollable() ancestor
                            if let Some(target_id) = &self.focused {
                                if let Some((scrollable_id, is_fake)) = find_scrollable_ancestor_with_type(root, target_id) {
                                    if is_fake {
                                        // For .scrollable() elements, emit Event::Scroll with action
                                        // so the widget handles it (they know virtual content size)
                                        events.push(Event::Scroll {
                                            target: Some(scrollable_id),
                                            x: 0,
                                            y: 0,
                                            delta_x: 0,
                                            delta_y: 0,
                                            action: Some(action),
                                        });
                                        continue;
                                    }
                                    // For overflow elements, let ScrollState handle it via Event::Key
                                }
                            }
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
                            let clickable_target = crate::hit::hit_test(layout, root, x, y);
                            let focusable_target = hit_test_focusable(layout, root, x, y);

                            log::debug!(
                                "[focus] MouseDown at ({}, {}), clickable={:?}, focusable={:?}, current_focus={:?}",
                                x, y, clickable_target, focusable_target, self.focused
                            );

                            // Blur focus if clicking on non-focusable area
                            if focusable_target.is_none() {
                                if let Some(old) = self.focused.take() {
                                    log::debug!("[focus] MouseDown on non-focusable area, blurring {}", old);
                                    events.push(Event::Blur { target: old, new_target: None });
                                }
                            }

                            // Check if click is within an interaction_scope but not on a clickable element
                            // This emits ScopeClick for backdrop clicks on modals, etc.
                            if clickable_target.is_none() {
                                if let Some(scope_id) = hit_test_interaction_scope(layout, root, x, y) {
                                    events.push(Event::ScopeClick {
                                        target: scope_id,
                                        x,
                                        y,
                                    });
                                }
                            }

                            events.push(Event::Click {
                                target: clickable_target,
                                x,
                                y,
                                button: button.into(),
                            });
                        }

                        MouseEventKind::Moved => {
                            // Focus follows mouse - check if we're over a focusable element
                            let focusable_target = hit_test_focusable(layout, root, x, y);
                            log::trace!(
                                "[focus] MouseMove at ({}, {}), focusable_target={:?}, current_focus={:?}",
                                x, y, focusable_target, self.focused
                            );
                            if let Some(focusable_target) = focusable_target {
                                // Respect interaction scope - only focus elements within active scope
                                let active_scope = find_active_scope(root, self.focused.as_deref());
                                let in_scope = match &active_scope {
                                    Some(scope_id) => is_in_scope(root, &focusable_target, scope_id),
                                    None => true, // No active scope, all elements eligible
                                };

                                log::debug!(
                                    "[focus] MouseMove hover: target={}, active_scope={:?}, in_scope={}, current_focus={:?}",
                                    focusable_target, active_scope, in_scope, self.focused
                                );

                                // Only change focus if different AND target is in scope
                                if in_scope && self.focused.as_ref() != Some(&focusable_target) {
                                    log::debug!("[focus] Changing focus from {:?} to {}", self.focused, focusable_target);
                                    if let Some(old) = self.focused.take() {
                                        events.push(Event::Blur { target: old, new_target: Some(focusable_target.clone()) });
                                    }
                                    self.focused = Some(focusable_target.clone());
                                    events.push(Event::Focus {
                                        target: focusable_target,
                                    });
                                } else if !in_scope {
                                    log::debug!("[focus] MouseMove blocked - target {} not in active scope {:?}", focusable_target, active_scope);
                                }
                            }

                            events.push(Event::MouseMove { x, y });
                        }

                        MouseEventKind::ScrollUp => {
                            let target = crate::hit::hit_test_scrollable(layout, root, x, y);
                            events.push(Event::Scroll {
                                target,
                                x,
                                y,
                                delta_x: 0,
                                delta_y: -1,
                                action: None,
                            });
                        }

                        MouseEventKind::ScrollDown => {
                            let target = crate::hit::hit_test_scrollable(layout, root, x, y);
                            events.push(Event::Scroll {
                                target,
                                x,
                                y,
                                delta_x: 0,
                                delta_y: 1,
                                action: None,
                            });
                        }

                        MouseEventKind::ScrollLeft => {
                            let target = crate::hit::hit_test_scrollable(layout, root, x, y);
                            events.push(Event::Scroll {
                                target,
                                x,
                                y,
                                delta_x: -1,
                                delta_y: 0,
                                action: None,
                            });
                        }

                        MouseEventKind::ScrollRight => {
                            let target = crate::hit::hit_test_scrollable(layout, root, x, y);
                            events.push(Event::Scroll {
                                target,
                                x,
                                y,
                                delta_x: 1,
                                delta_y: 0,
                                action: None,
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
/// If `active_scope` is Some, only collects from within that scope.
pub fn collect_focusable(element: &Element, active_scope: Option<&str>) -> Vec<String> {
    let mut result = Vec::new();
    let in_scope = active_scope.is_none(); // Start in scope if no scope specified
    collect_focusable_recursive(element, active_scope, in_scope, &mut result);
    result
}

fn collect_focusable_recursive(
    element: &Element,
    active_scope: Option<&str>,
    in_scope: bool,
    result: &mut Vec<String>,
) {
    // Check if we've entered the active scope
    let now_in_scope = in_scope || active_scope.map_or(false, |s| element.id == s);

    if element.focusable && now_in_scope {
        result.push(element.id.clone());
    }
    if let Content::Children(children) = &element.content {
        for child in children {
            collect_focusable_recursive(child, active_scope, now_in_scope, result);
        }
    }
}

/// Collect all focusable elements with their effective z-index.
/// Used for keyboard navigation to prioritize higher z-index elements (overlays).
/// If `active_scope` is Some, only collects from within that scope.
fn collect_focusable_with_z(element: &Element, active_scope: Option<&str>) -> Vec<(String, i16)> {
    let mut result = Vec::new();
    let in_scope = active_scope.is_none(); // Start in scope if no scope specified
    collect_focusable_with_z_recursive(element, 0, active_scope, in_scope, &mut result);
    result
}

fn collect_focusable_with_z_recursive(
    element: &Element,
    inherited_z: i16,
    active_scope: Option<&str>,
    in_scope: bool,
    result: &mut Vec<(String, i16)>,
) {
    // Check if we've entered the active scope
    let now_in_scope = in_scope || active_scope.map_or(false, |s| element.id == s);

    // Effective z-index: use element's z_index if set, otherwise inherit
    let effective_z = if element.z_index != 0 {
        element.z_index
    } else {
        inherited_z
    };

    if element.focusable && now_in_scope {
        result.push((element.id.clone(), effective_z));
    }
    if let Content::Children(children) = &element.content {
        for child in children {
            collect_focusable_with_z_recursive(child, effective_z, active_scope, now_in_scope, result);
        }
    }
}

/// Get the absolute screen position of an element, accounting for scroll container offset.
/// If the element is inside a scroll container, its layout position may be relative to the
/// container's content area on the scrolling axis. We add the container's screen position
/// only for the axis that scrolls.
fn get_absolute_rect(
    element_id: &str,
    layout: &LayoutResult,
    root: &Element,
) -> Option<Rect> {
    let rect = layout.get(element_id)?;

    // Find the scrollable ancestor (if any)
    if let Some(scrollable_id) = find_scrollable_ancestor(root, element_id) {
        // Get the scrollable container's screen position and overflow settings
        if let Some(container_rect) = layout.get(&scrollable_id) {
            // Find the container element to check its overflow settings
            if let Some(container_el) = find_element(root, &scrollable_id) {
                let scrolls_x = container_el.overflow_x == crate::types::Overflow::Scroll
                    || container_el.overflow_x == crate::types::Overflow::Auto;
                let scrolls_y = container_el.overflow_y == crate::types::Overflow::Scroll
                    || container_el.overflow_y == crate::types::Overflow::Auto
                    || container_el.scrollable;

                // Only add container offset for axes that scroll
                // (non-scrolling axes already have absolute positions)
                let abs_x = if scrolls_x {
                    container_rect.x + rect.x
                } else {
                    rect.x
                };
                let abs_y = if scrolls_y {
                    container_rect.y + rect.y
                } else {
                    rect.y
                };

                return Some(Rect::new(abs_x, abs_y, rect.width, rect.height));
            }
        }
    }

    // No scroll container, position is already absolute
    Some(*rect)
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
