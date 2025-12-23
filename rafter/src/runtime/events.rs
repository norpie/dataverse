//! Event handling - convert crossterm events to rafter events.

use crossterm::event::{
    Event as CrosstermEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton,
    MouseEvent, MouseEventKind,
};
use log::trace;

use crate::input::events::{
    ClickEvent, ClickKind, Modifiers, Position, ScrollDirection, ScrollEvent,
};
use crate::input::keybinds::{Key, KeyCombo};

/// Drag event for mouse drag operations
#[derive(Debug, Clone)]
pub struct DragEvent {
    /// Current drag position
    pub position: Position,
    /// Modifier keys held during drag
    pub modifiers: Modifiers,
}

/// Rafter event types
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Event {
    /// Key press event
    Key(KeyCombo),
    /// Mouse click event
    Click(ClickEvent),
    /// Mouse button release event
    Release(Position),
    /// Mouse hover/move event
    Hover(Position),
    /// Mouse drag event (button held while moving)
    Drag(DragEvent),
    /// Mouse scroll event
    Scroll(ScrollEvent),
    /// Terminal resize event
    Resize { width: u16, height: u16 },
    /// Request to quit
    Quit,
}

/// Convert crossterm KeyModifiers to rafter Modifiers
fn convert_modifiers(mods: KeyModifiers) -> Modifiers {
    Modifiers {
        ctrl: mods.contains(KeyModifiers::CONTROL),
        shift: mods.contains(KeyModifiers::SHIFT),
        alt: mods.contains(KeyModifiers::ALT),
    }
}

/// Convert crossterm KeyCode to rafter Key
fn convert_key(code: KeyCode) -> Option<Key> {
    match code {
        KeyCode::Char(c) => Some(Key::Char(c)),
        KeyCode::F(n) => Some(Key::F(n)),
        KeyCode::Enter => Some(Key::Enter),
        KeyCode::Esc => Some(Key::Escape),
        KeyCode::Backspace => Some(Key::Backspace),
        KeyCode::Tab | KeyCode::BackTab => Some(Key::Tab),
        KeyCode::Up => Some(Key::Up),
        KeyCode::Down => Some(Key::Down),
        KeyCode::Left => Some(Key::Left),
        KeyCode::Right => Some(Key::Right),
        KeyCode::Home => Some(Key::Home),
        KeyCode::End => Some(Key::End),
        KeyCode::PageUp => Some(Key::PageUp),
        KeyCode::PageDown => Some(Key::PageDown),
        KeyCode::Insert => Some(Key::Insert),
        KeyCode::Delete => Some(Key::Delete),
        _ => None,
    }
}

/// Convert a crossterm KeyEvent to a rafter KeyCombo
pub fn convert_key_event(event: KeyEvent) -> Option<KeyCombo> {
    let key = convert_key(event.code)?;
    let modifiers = convert_modifiers(event.modifiers);

    // Handle space specially (KeyCode::Char(' ') should become Key::Space)
    let key = if let Key::Char(' ') = key {
        Key::Space
    } else {
        key
    };

    Some(KeyCombo::new(key, modifiers))
}

/// Convert a crossterm MouseEvent to a rafter Event
pub fn convert_mouse_event(event: MouseEvent) -> Option<Event> {
    let position = Position::new(event.column, event.row);
    let modifiers = convert_modifiers(event.modifiers);

    match event.kind {
        MouseEventKind::Down(button) => {
            let kind = match button {
                MouseButton::Left => ClickKind::Primary,
                MouseButton::Right => ClickKind::Secondary,
                MouseButton::Middle => return None, // Not supported yet
            };
            Some(Event::Click(ClickEvent {
                kind,
                position,
                modifiers,
            }))
        }
        MouseEventKind::ScrollUp => Some(Event::Scroll(ScrollEvent {
            direction: ScrollDirection::Up,
            position,
            amount: 3,
        })),
        MouseEventKind::ScrollDown => Some(Event::Scroll(ScrollEvent {
            direction: ScrollDirection::Down,
            position,
            amount: 3,
        })),
        MouseEventKind::ScrollLeft => Some(Event::Scroll(ScrollEvent {
            direction: ScrollDirection::Left,
            position,
            amount: 3,
        })),
        MouseEventKind::ScrollRight => Some(Event::Scroll(ScrollEvent {
            direction: ScrollDirection::Right,
            position,
            amount: 3,
        })),
        MouseEventKind::Moved => Some(Event::Hover(position)),
        MouseEventKind::Drag(_) => Some(Event::Drag(DragEvent {
            position,
            modifiers,
        })),
        MouseEventKind::Up(_) => Some(Event::Release(position)),
    }
}

/// Convert a crossterm Event to a rafter Event
pub fn convert_event(event: CrosstermEvent) -> Option<Event> {
    match event {
        CrosstermEvent::Key(key_event) => {
            log::debug!(
                "Crossterm key: code={:?}, modifiers={:?}, kind={:?}",
                key_event.code,
                key_event.modifiers,
                key_event.kind
            );

            // Only handle key press events, not release or repeat
            if key_event.kind != KeyEventKind::Press {
                trace!("Ignoring non-press key event");
                return None;
            }

            convert_key_event(key_event).map(Event::Key)
        }
        CrosstermEvent::Mouse(mouse_event) => convert_mouse_event(mouse_event),
        CrosstermEvent::Resize(width, height) => Some(Event::Resize { width, height }),
        _ => None,
    }
}
