use tuidom::{
    Border, Color, Direction, Edges, Element, Event, FocusState, Justify, Key, Size, Style,
    Terminal, TextAlign, TextWrap,
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
        .padding(Edges::all(1))
        .gap(1)
        .child(header())
        .child(content(focused))
        .child(footer(last_event))
        .child(overlay())
}

fn overlay() -> Element {
    Element::box_()
        .id("overlay")
        .position(tuidom::Position::Absolute)
        .left(30)
        .top(8)
        .width(Size::Fixed(20))
        .height(Size::Fixed(5))
        .style(
            Style::new()
                .background(Color::oklch(0.4, 0.15, 30.0))
                .border(Border::Double),
        )
        .padding(Edges::all(1))
        .child(Element::text("Absolute overlay!"))
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
        .gap(1)
        .child(Element::text("Focus & Events Demo").style(Style::new().bold()))
        .child(Element::text(""))
        .child(Element::text("Instructions:"))
        .child(Element::text("- Hover over items to focus (Linux WM style)"))
        .child(Element::text("- Tab/Shift+Tab for keyboard navigation"))
        .child(Element::text("- Click on buttons"))
        .child(Element::text("- Press 'q' or Escape to quit"))
        .child(Element::text(""))
        .child(text_examples(focused))
}

fn text_examples(focused: Option<&str>) -> Element {
    Element::row()
        .width(Size::Fill)
        .height(Size::Fill)
        .gap(1)
        .child(
            Element::col()
                .width(Size::Fixed(25))
                .height(Size::Fill)
                .style(
                    Style::new()
                        .background(Color::oklch(0.2, 0.03, 200.0))
                        .border(Border::Single),
                )
                .padding(Edges::all(1))
                .child(Element::text("Word Wrap:").style(Style::new().bold()))
                .child(
                    Element::text(
                        "This is a longer sentence that will wrap at word boundaries.",
                    )
                    .text_wrap(TextWrap::WordWrap),
                ),
        )
        .child(
            Element::col()
                .id("input_area")
                .focusable(true)
                .width(Size::Fill)
                .height(Size::Fill)
                .style(
                    Style::new()
                        .background(if focused == Some("input_area") {
                            Color::oklch(0.25, 0.05, 100.0)
                        } else {
                            Color::oklch(0.2, 0.03, 100.0)
                        })
                        .border(if focused == Some("input_area") {
                            Border::Double
                        } else {
                            Border::Single
                        }),
                )
                .padding(Edges::all(1))
                .gap(1)
                .child(Element::text("Input Area (focusable)").style(Style::new().bold()))
                .child(Element::text("Type when focused:"))
                .child(Element::text(format!(
                    "Status: {}",
                    if focused == Some("input_area") {
                        "FOCUSED"
                    } else {
                        "not focused"
                    }
                )))
                .child(Element::text(""))
                .child(Element::text("Unicode: 日本語 한글 😀")),
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
