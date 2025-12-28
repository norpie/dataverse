use std::fs::File;

use crossterm::event::{Event as CrosstermEvent, MouseButton, MouseEventKind};
use simplelog::{Config, LevelFilter, WriteLogger};
use tuidom::{
    Border, Color, Edges, Element, Event, FocusState, Key, LayoutResult, Overflow, ScrollState,
    Size, Style, Terminal,
};

/// Tracks scrollbar dragging state
struct DragState {
    /// Which scrollable element is being dragged
    element_id: Option<String>,
    /// Is it a vertical (true) or horizontal (false) scrollbar
    is_vertical: bool,
    /// Offset from the start of the thumb where the user clicked (for smooth dragging)
    thumb_offset: u16,
}

fn main() -> std::io::Result<()> {
    // Set up file logging
    let log_file = File::create("scroll.log")?;
    WriteLogger::init(LevelFilter::Debug, Config::default(), log_file)
        .expect("Failed to initialize logger");

    let mut term = Terminal::new()?;
    let mut focus = FocusState::new();
    let mut scroll = ScrollState::new();
    let mut drag = DragState {
        element_id: None,
        is_vertical: true,
        thumb_offset: 0,
    };

    loop {
        // Get scroll offsets for the scrollable containers
        let scroll1 = scroll.get("scroll1");
        let scroll2 = scroll.get("scroll2");
        let scroll3 = scroll.get("scroll3");

        let root = ui(scroll1.y, scroll2.y, scroll3.x);
        term.render(&root)?;

        let raw_events = term.poll(None)?;

        // Handle scrollbar dragging from raw events
        for raw_event in &raw_events {
            handle_scrollbar_interaction(
                raw_event,
                &mut scroll,
                &mut drag,
                term.layout(),
            );
        }

        let events = focus.process_events(&raw_events, &root, term.layout());

        // Process scroll wheel events
        scroll.process_events(&events, &root, term.layout());

        for event in &events {
            match event {
                Event::Key {
                    key: Key::Char('q'),
                    ..
                }
                | Event::Key {
                    key: Key::Escape, ..
                } => {
                    return Ok(());
                }
                _ => {}
            }
        }
    }
}

/// Handle mouse interactions with scrollbars (click and drag)
fn handle_scrollbar_interaction(
    event: &CrosstermEvent,
    scroll: &mut ScrollState,
    drag: &mut DragState,
    layout: &LayoutResult,
) {
    match event {
        CrosstermEvent::Mouse(mouse) => {
            let x = mouse.column;
            let y = mouse.row;

            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    // Check if click is on a scrollbar
                    for id in &["scroll1", "scroll2", "scroll3"] {
                        if let Some((is_vertical, thumb_offset, on_thumb, thumb_size)) =
                            check_scrollbar_hit(id, x, y, layout, scroll)
                        {
                            drag.element_id = Some(id.to_string());
                            drag.is_vertical = is_vertical;

                            if on_thumb {
                                // Clicked on thumb - remember offset for smooth dragging
                                drag.thumb_offset = thumb_offset;
                            } else {
                                // Clicked on track - center thumb on click position
                                drag.thumb_offset = thumb_size / 2;
                                if let Some(scroll_pos) = calculate_scroll_from_position(
                                    id, x, y, is_vertical, drag.thumb_offset, layout,
                                ) {
                                    let current = scroll.get(id);
                                    if is_vertical {
                                        scroll.set(id, current.x, scroll_pos);
                                    } else {
                                        scroll.set(id, scroll_pos, current.y);
                                    }
                                }
                            }
                            break;
                        }
                    }
                }

                MouseEventKind::Up(MouseButton::Left) => {
                    // Stop dragging
                    drag.element_id = None;
                }

                MouseEventKind::Drag(MouseButton::Left) => {
                    // Update scroll position while dragging
                    if let Some(ref id) = drag.element_id {
                        if let Some(scroll_pos) = calculate_scroll_from_position(
                            id, x, y, drag.is_vertical, drag.thumb_offset, layout,
                        ) {
                            let current = scroll.get(id);
                            if drag.is_vertical {
                                scroll.set(id, current.x, scroll_pos);
                            } else {
                                scroll.set(id, scroll_pos, current.y);
                            }
                        }
                    }
                }

                _ => {}
            }
        }
        _ => {}
    }
}

/// Check if a click is on a scrollbar, returns (is_vertical, thumb_offset, on_thumb, thumb_size) if hit
fn check_scrollbar_hit(
    id: &str,
    x: u16,
    y: u16,
    layout: &LayoutResult,
    scroll: &ScrollState,
) -> Option<(bool, u16, bool, u16)> {
    let rect = layout.get(id)?;
    let (content_width, content_height) = layout.content_size(id)?;
    let (inner_width, inner_height) = layout.viewport_size(id)?;
    let current = scroll.get(id);

    let border_size = 1u16; // For track positioning

    // Check vertical scrollbar (right edge)
    if content_height > inner_height {
        let scrollbar_x = rect.right() - 1 - border_size;
        let track_start = rect.y + border_size;
        let track_end = rect.bottom() - border_size;

        if x == scrollbar_x && y >= track_start && y < track_end {
            let track_height = track_end - track_start;
            let max_scroll = content_height.saturating_sub(inner_height);

            // Calculate thumb size and position
            let thumb_size = ((inner_height as u32 * track_height as u32) / content_height as u32)
                .max(1)
                .min(track_height as u32) as u16;
            let scroll_range = track_height.saturating_sub(thumb_size);
            let thumb_pos = if max_scroll > 0 && scroll_range > 0 {
                ((current.y as u32 * scroll_range as u32) / max_scroll as u32).min(scroll_range as u32) as u16
            } else {
                0
            };

            let thumb_start = track_start + thumb_pos;
            let thumb_end = thumb_start + thumb_size;

            // Check if click is on thumb
            let on_thumb = y >= thumb_start && y < thumb_end;
            let thumb_offset = if on_thumb { y - thumb_start } else { 0 };

            return Some((true, thumb_offset, on_thumb, thumb_size));
        }
    }

    // Check horizontal scrollbar (bottom edge)
    if content_width > inner_width {
        let scrollbar_y = rect.bottom() - 1 - border_size;
        let track_start = rect.x + border_size;
        let track_end = rect.right() - border_size;
        // Reduce for vertical scrollbar if present
        let track_end = if content_height > inner_height {
            track_end.saturating_sub(1)
        } else {
            track_end
        };

        if y == scrollbar_y && x >= track_start && x < track_end {
            let track_width = track_end - track_start;
            let max_scroll = content_width.saturating_sub(inner_width);

            // Calculate thumb size and position
            let thumb_size = ((inner_width as u32 * track_width as u32) / content_width as u32)
                .max(1)
                .min(track_width as u32) as u16;
            let scroll_range = track_width.saturating_sub(thumb_size);
            let thumb_pos = if max_scroll > 0 && scroll_range > 0 {
                ((current.x as u32 * scroll_range as u32) / max_scroll as u32).min(scroll_range as u32) as u16
            } else {
                0
            };

            let thumb_start = track_start + thumb_pos;
            let thumb_end = thumb_start + thumb_size;

            // Check if click is on thumb
            let on_thumb = x >= thumb_start && x < thumb_end;
            let thumb_offset = if on_thumb { x - thumb_start } else { 0 };

            return Some((false, thumb_offset, on_thumb, thumb_size));
        }
    }

    None
}

/// Calculate scroll position from mouse position during drag
fn calculate_scroll_from_position(
    id: &str,
    x: u16,
    y: u16,
    is_vertical: bool,
    thumb_offset: u16,
    layout: &LayoutResult,
) -> Option<u16> {
    let rect = layout.get(id)?;
    let (content_width, content_height) = layout.content_size(id)?;
    let (inner_width, inner_height) = layout.viewport_size(id)?;

    let border_size = 1u16; // For track positioning

    if is_vertical {
        let track_start = rect.y + border_size;
        let track_end = rect.bottom() - border_size;
        let track_height = track_end.saturating_sub(track_start);

        if track_height == 0 {
            return Some(0);
        }

        let max_scroll = content_height.saturating_sub(inner_height);

        // Calculate thumb size for proper offset handling
        let thumb_size = ((inner_height as u32 * track_height as u32) / content_height as u32)
            .max(1)
            .min(track_height as u32) as u16;
        let scroll_range = track_height.saturating_sub(thumb_size);

        if scroll_range == 0 {
            return Some(0);
        }

        // Adjust for thumb offset - the mouse position minus offset gives thumb start
        let thumb_start_y = y.saturating_sub(thumb_offset);
        let clamped_thumb_start = thumb_start_y.clamp(track_start, track_start + scroll_range);
        let thumb_offset_in_track = clamped_thumb_start - track_start;

        let scroll_pos = (thumb_offset_in_track as u32 * max_scroll as u32 / scroll_range as u32) as u16;
        Some(scroll_pos.min(max_scroll))
    } else {
        let track_start = rect.x + border_size;
        let track_end = rect.right() - border_size;
        // Reduce for vertical scrollbar if present
        let track_end = if content_height > inner_height {
            track_end.saturating_sub(1)
        } else {
            track_end
        };
        let track_width = track_end.saturating_sub(track_start);

        if track_width == 0 {
            return Some(0);
        }

        let max_scroll = content_width.saturating_sub(inner_width);

        // Calculate thumb size for proper offset handling
        let thumb_size = ((inner_width as u32 * track_width as u32) / content_width as u32)
            .max(1)
            .min(track_width as u32) as u16;
        let scroll_range = track_width.saturating_sub(thumb_size);

        if scroll_range == 0 {
            return Some(0);
        }

        // Adjust for thumb offset - the mouse position minus offset gives thumb start
        let thumb_start_x = x.saturating_sub(thumb_offset);
        let clamped_thumb_start = thumb_start_x.clamp(track_start, track_start + scroll_range);
        let thumb_offset_in_track = clamped_thumb_start - track_start;

        let scroll_pos = (thumb_offset_in_track as u32 * max_scroll as u32 / scroll_range as u32) as u16;
        Some(scroll_pos.min(max_scroll))
    }
}

fn ui(scroll1_y: u16, scroll2_y: u16, scroll3_x: u16) -> Element {
    Element::col()
        .width(Size::Fill)
        .height(Size::Fill)
        .style(Style::new().background(Color::oklch(0.15, 0.01, 250.0)))
        .padding(Edges::all(1))
        .gap(1)
        .child(
            Element::text("Scroll Demo - wheel/click/drag scrollbars, q=quit")
                .style(Style::new().bold()),
        )
        .child(
            Element::row()
                .width(Size::Fill)
                .height(Size::Fill)
                .gap(2)
                // Vertical scrollable list
                .child(scroll_list("scroll1", scroll1_y))
                // Vertical scrollable content
                .child(scroll_content("scroll2", scroll2_y))
                // Horizontal scroll demo
                .child(horizontal_scroll("scroll3", scroll3_x)),
        )
}

fn scroll_list(id: &str, scroll_y: u16) -> Element {
    Element::col()
        .id(id)
        .width(Size::Fixed(25))
        .height(Size::Fill)
        .overflow(Overflow::Auto)
        .scroll_offset(0, scroll_y)
        .style(
            Style::new()
                .background(Color::oklch(0.2, 0.03, 200.0))
                .border(Border::Rounded),
        )
        .padding(Edges::all(1))
        .gap(0)
        .child(Element::text("Scrollable List").style(Style::new().bold()))
        .child(Element::text(""))
        .children((1..=150).map(|i| list_item(i)))
}

fn list_item(n: u32) -> Element {
    let hue = (n as f32 * 18.0) % 360.0;
    Element::text(format!("Item {}", n)).style(Style::new().background(Color::oklch(0.35, 0.08, hue)))
}

fn scroll_content(id: &str, scroll_y: u16) -> Element {
    Element::col()
        .id(id)
        .width(Size::Fill)
        .height(Size::Fill)
        .overflow(Overflow::Auto)
        .scroll_offset(0, scroll_y)
        .style(
            Style::new()
                .background(Color::oklch(0.2, 0.03, 280.0))
                .border(Border::Rounded),
        )
        .padding(Edges::all(1))
        .child(Element::text("Scrollable Content").style(Style::new().bold()))
        .child(Element::text(""))
        .child(Element::text("This is a longer piece of text that"))
        .child(Element::text("demonstrates scrolling within a"))
        .child(Element::text("container. The content extends"))
        .child(Element::text("beyond the visible area."))
        .child(Element::text(""))
        .child(Element::text("Use your mouse wheel to scroll"))
        .child(Element::text("up and down through this content."))
        .child(Element::text(""))
        .child(Element::text("The scrollbar on the right shows"))
        .child(Element::text("your current position."))
        .child(Element::text(""))
        .child(Element::text("You can also click on the scrollbar"))
        .child(Element::text("track to jump to that position, or"))
        .child(Element::text("drag the thumb to scroll smoothly."))
        .child(Element::text(""))
        .child(Element::text("--- More content below ---"))
        .child(Element::text(""))
        .children((1..=30).map(|i| {
            Element::text(format!("Line {}: Some content here", i))
        }))
        .child(Element::text(""))
        .child(Element::text("--- End of content ---"))
}

fn horizontal_scroll(id: &str, scroll_x: u16) -> Element {
    Element::col()
        .width(Size::Fixed(30))
        .height(Size::Fill)
        .style(
            Style::new()
                .background(Color::oklch(0.2, 0.03, 60.0))
                .border(Border::Rounded),
        )
        .padding(Edges::all(1))
        .child(Element::text("Horizontal Scroll").style(Style::new().bold()))
        .child(Element::text("(drag scrollbar)"))
        .child(Element::text(""))
        .child(
            Element::row()
                .id(id)
                .width(Size::Fill)
                .height(Size::Fill)
                .overflow(Overflow::Auto)
                .scroll_offset(scroll_x, 0)
                .style(
                    Style::new()
                        .background(Color::oklch(0.25, 0.05, 60.0))
                        .border(Border::Single),
                )
                .gap(1)
                .children((1..=15).map(|i| {
                    let hue = (i as f32 * 24.0) % 360.0;
                    Element::col()
                        .width(Size::Fixed(12))
                        .height(Size::Fill)
                        .style(Style::new().background(Color::oklch(0.4, 0.12, hue)))
                        .padding(Edges::all(1))
                        .child(Element::text(format!("Card {}", i)).style(Style::new().bold()))
                        .child(Element::text(""))
                        .child(Element::text("Content"))
                        .child(Element::text(format!("Value: {}", i * 10)))
                        .child(Element::text(""))
                        .child(Element::text("More info"))
                })),
        )
}
