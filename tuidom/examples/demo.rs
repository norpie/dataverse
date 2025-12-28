use tuidom::{
    Align, Border, Color, Direction, Edges, Element, Event, FocusState, Justify, Key, Position,
    Size, Style, Terminal, TextAlign, TextWrap,
};

fn main() -> std::io::Result<()> {
    let mut term = Terminal::new()?;
    let mut focus = FocusState::new();
    let mut last_event: Option<String> = None;

    loop {
        let root = ui(focus.focused(), last_event.as_deref());
        term.render(&root)?;

        let raw_events = term.poll(None)?;
        let events = focus.process_events(&raw_events, &root, term.layout());

        for event in events {
            match &event {
                Event::Key { key: Key::Char('q'), .. } | Event::Key { key: Key::Escape, .. } => {
                    return Ok(());
                }
                Event::Key { target, key, modifiers } => {
                    last_event = Some(format!(
                        "Key {:?} (shift={}, ctrl={}) on {:?}",
                        key, modifiers.shift, modifiers.ctrl, target
                    ));
                }
                Event::Click { target, x, y, button } => {
                    last_event = Some(format!("Click {:?} at ({}, {}) on {:?}", button, x, y, target));
                }
                Event::Focus { target } => {
                    last_event = Some(format!("Focus: {}", target));
                }
                Event::Blur { target } => {
                    last_event = Some(format!("Blur: {}", target));
                }
                Event::Scroll { target, delta, .. } => {
                    last_event = Some(format!("Scroll {} on {:?}", delta, target));
                }
                _ => {}
            }
        }
    }
}

fn ui(focused: Option<&str>, last_event: Option<&str>) -> Element {
    Element::col()
        .width(Size::Fill)
        .height(Size::Fill)
        .child(header())
        .child(content(focused))
        .child(footer(last_event))
}

fn header() -> Element {
    Element::box_()
        .width(Size::Fill)
        .height(Size::Fixed(3))
        .style(
            Style::new()
                .background(Color::oklch(0.3, 0.1, 250.0))
                .border(Border::Rounded),
        )
        .padding(Edges::symmetric(0, 1))
        .child(Element::text("tuidom demo - Events & Focus").style(Style::new().bold()))
}

fn content(focused: Option<&str>) -> Element {
    Element::row()
        .width(Size::Fill)
        .height(Size::Fill)
        .gap(1)
        .child(sidebar(focused))
        .child(main_panel(focused))
}

fn sidebar(focused: Option<&str>) -> Element {
    Element::col()
        .width(Size::Fixed(24))
        .height(Size::Fill)
        .style(
            Style::new()
                .background(Color::oklch(0.2, 0.02, 250.0))
                .border(Border::Single),
        )
        .padding(Edges::all(1))
        .gap(1)
        .child(Element::text("Navigation (Tab to move)"))
        .child(nav_item("dashboard", "Dashboard", focused))
        .child(nav_item("settings", "Settings", focused))
        .child(nav_item("about", "About", focused))
        .child(Element::text(""))
        .child(Element::text("Buttons"))
        .child(button("btn_ok", "[ OK ]", focused))
        .child(button("btn_cancel", "[ Cancel ]", focused))
}

fn nav_item(id: &str, label: &str, focused: Option<&str>) -> Element {
    let is_focused = focused == Some(id);
    let prefix = if is_focused { ">" } else { " " };
    let bg = if is_focused {
        Color::oklch(0.35, 0.08, 250.0)
    } else {
        Color::oklch(0.25, 0.02, 250.0)
    };

    Element::text(format!("{} {}", prefix, label))
        .id(id)
        .focusable(true)
        .clickable(true)
        .width(Size::Fill)
        .style(Style::new().background(bg))
}

fn button(id: &str, label: &str, focused: Option<&str>) -> Element {
    let is_focused = focused == Some(id);
    let bg = if is_focused {
        Color::oklch(0.4, 0.12, 140.0)
    } else {
        Color::oklch(0.3, 0.06, 140.0)
    };

    Element::text(label)
        .id(id)
        .focusable(true)
        .clickable(true)
        .text_align(TextAlign::Center)
        .width(Size::Fill)
        .style(Style::new().background(bg).bold())
}

fn main_panel(focused: Option<&str>) -> Element {
    Element::col()
        .width(Size::Fill)
        .height(Size::Fill)
        .style(
            Style::new()
                .background(Color::oklch(0.15, 0.01, 250.0))
                .border(Border::Single),
        )
        .padding(Edges::all(1))
        .child(Element::text("Layout Features Demo").style(Style::new().bold()))
        .child(Element::text("q=quit | Tab=navigate | Hover=focus"))
        .child(layout_examples(focused))
}

fn layout_examples(_focused: Option<&str>) -> Element {
    Element::row()
        .width(Size::Fill)
        .height(Size::Fill)
        .gap(1)
        // Margin demo
        .child(
            Element::col()
                .width(Size::Fixed(22))
                .height(Size::Fill)
                .style(
                    Style::new()
                        .background(Color::oklch(0.2, 0.03, 200.0))
                        .border(Border::Single),
                )
                .padding(Edges::all(1))
                .child(Element::text("Margin:").style(Style::new().bold()))
                .child(
                    Element::box_()
                        .height(Size::Fixed(3))
                        .margin(Edges::new(1, 2, 1, 2))
                        .style(Style::new().background(Color::oklch(0.4, 0.1, 120.0)))
                        .child(Element::text("margin: 1,2")),
                )
                .child(
                    Element::box_()
                        .height(Size::Fixed(3))
                        .style(Style::new().background(Color::oklch(0.4, 0.1, 180.0)))
                        .child(Element::text("no margin")),
                ),
        )
        // Align demo
        .child(
            Element::col()
                .width(Size::Fixed(24))
                .height(Size::Fill)
                .style(
                    Style::new()
                        .background(Color::oklch(0.2, 0.03, 280.0))
                        .border(Border::Single),
                )
                .padding(Edges::all(1))
                .child(Element::text("Cross-Axis Align:").style(Style::new().bold()))
                .child(
                    Element::row()
                        .height(Size::Fixed(5))
                        .align(Align::Start)
                        .style(Style::new().background(Color::oklch(0.35, 0.05, 280.0)))
                        .child(Element::text("Start").style(Style::new().background(Color::oklch(0.5, 0.12, 60.0)))),
                )
                .child(
                    Element::row()
                        .height(Size::Fixed(5))
                        .align(Align::Center)
                        .style(Style::new().background(Color::oklch(0.35, 0.05, 280.0)))
                        .child(Element::text("Center").style(Style::new().background(Color::oklch(0.5, 0.12, 120.0)))),
                )
                .child(
                    Element::row()
                        .height(Size::Fixed(5))
                        .align(Align::End)
                        .style(Style::new().background(Color::oklch(0.35, 0.05, 280.0)))
                        .child(Element::text("End").style(Style::new().background(Color::oklch(0.5, 0.12, 180.0)))),
                ),
        )
        // Min/Max demo
        .child(
            Element::col()
                .width(Size::Fixed(24))
                .height(Size::Fill)
                .style(
                    Style::new()
                        .background(Color::oklch(0.2, 0.03, 320.0))
                        .border(Border::Single),
                )
                .padding(Edges::all(1))
                .child(Element::text("Min/Max Width:").style(Style::new().bold()))
                .child(
                    Element::box_()
                        .width(Size::Fill)
                        .max_width(20)
                        .height(Size::Fixed(3))
                        .style(Style::new().background(Color::oklch(0.35, 0.1, 30.0)))
                        .child(Element::text("max: 20")),
                )
                .child(
                    Element::box_()
                        .width(Size::Fixed(5))
                        .min_width(15)
                        .height(Size::Fixed(3))
                        .style(Style::new().background(Color::oklch(0.35, 0.1, 90.0)))
                        .child(Element::text("min: 15")),
                )
                .child(Element::text(""))
                .child(Element::text("align_self:").style(Style::new().bold()))
                .child(
                    Element::row()
                        .height(Size::Fixed(5))
                        .align(Align::Start)
                        .style(Style::new().background(Color::oklch(0.15, 0.02, 320.0)))
                        .child(Element::text("Start").style(Style::new().background(Color::oklch(0.3, 0.06, 200.0))))
                        .child(
                            Element::text("End")
                                .align_self(Align::End)
                                .style(Style::new().background(Color::oklch(0.3, 0.06, 260.0))),
                        ),
                ),
        )
        // z_index demo - overlapping boxes with different z_index
        .child(
            Element::box_()
                .width(Size::Fill)
                .height(Size::Fill)
                .style(
                    Style::new()
                        .background(Color::oklch(0.2, 0.03, 60.0))
                        .border(Border::Single),
                )
                .padding(Edges::all(1))
                .child(Element::text("z_index:").style(Style::new().bold()))
                // Three overlapping boxes with different z_index
                // Red box at z=0 (lowest)
                .child(
                    Element::box_()
                        .position(Position::Absolute)
                        .left(2)
                        .top(3)
                        .width(Size::Fixed(14))
                        .height(Size::Fixed(6))
                        .z_index(0)
                        .style(Style::new().background(Color::oklch(0.4, 0.15, 25.0))) // Red
                        .child(Element::text("z=0 (back)")),
                )
                // Green box at z=1 (middle)
                .child(
                    Element::box_()
                        .position(Position::Absolute)
                        .left(6)
                        .top(5)
                        .width(Size::Fixed(14))
                        .height(Size::Fixed(6))
                        .z_index(1)
                        .style(Style::new().background(Color::oklch(0.45, 0.15, 140.0))) // Green
                        .child(Element::text("z=1 (mid)")),
                )
                // Blue box at z=2 (front)
                .child(
                    Element::box_()
                        .position(Position::Absolute)
                        .left(10)
                        .top(7)
                        .width(Size::Fixed(14))
                        .height(Size::Fixed(6))
                        .z_index(2)
                        .style(Style::new().background(Color::oklch(0.4, 0.15, 250.0))) // Blue
                        .child(Element::text("z=2 (front)")),
                )
                // Label at bottom
                .child(
                    Element::text("Higher z = on top")
                        .position(Position::Absolute)
                        .left(2)
                        .bottom(2)
                        .style(Style::new().dim()),
                ),
        )
}

fn footer(last_event: Option<&str>) -> Element {
    let event_text = last_event.unwrap_or("No events yet");

    Element::box_()
        .width(Size::Fill)
        .height(Size::Fixed(1))
        .direction(Direction::Row)
        .justify(Justify::SpaceBetween)
        .style(Style::new().background(Color::oklch(0.25, 0.02, 250.0)))
        .child(Element::text(format!("Event: {}", event_text)).text_wrap(TextWrap::Truncate))
        .child(Element::text("q=quit"))
}
