use std::fs::File;
use std::time::Duration;

use simplelog::{Config, LevelFilter, WriteLogger};
use tuidom::{
    Border, Color, Easing, Edges, Element, Event, FocusState, Key, Position, Size, Style, Terminal,
    Transitions,
};

fn main() -> std::io::Result<()> {
    // Set up file logging
    let log_file = File::create("transitions.log")?;
    WriteLogger::init(LevelFilter::Debug, Config::default(), log_file)
        .expect("Failed to initialize logger");

    let mut term = Terminal::new()?;
    let mut focus = FocusState::new();

    // Track which button is "active" (clicked)
    let mut active_button: Option<String> = None;

    loop {
        let root = ui(focus.focused(), active_button.as_deref());
        term.render(&root)?;

        let raw_events = term.poll(None)?;
        let events = focus.process_events(&raw_events, &root, term.layout());

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
                Event::Key {
                    key: Key::Char('r'),
                    ..
                } => {
                    // Toggle reduced motion
                    let current = term.has_active_transitions();
                    term.set_reduced_motion(!current);
                }
                Event::Click { target, .. } => {
                    if let Some(id) = target {
                        if id.starts_with("btn_") {
                            active_button = Some(id.clone());
                        }
                    }
                }
                _ => {}
            }
        }

        // Clear active button after a short time (simulate button press)
        if active_button.is_some() && !term.has_active_transitions() {
            active_button = None;
        }
    }
}

fn ui(focused: Option<&str>, active: Option<&str>) -> Element {
    Element::col()
        .width(Size::Fill)
        .height(Size::Fill)
        .style(Style::new().background(Color::oklch(0.15, 0.01, 250.0)))
        .padding(Edges::all(2))
        .gap(2)
        .child(
            Element::text("Transitions Demo").style(
                Style::new()
                    .bold()
                    .foreground(Color::oklch(0.9, 0.05, 250.0)),
            ),
        )
        .child(Element::text("Tab to navigate, click buttons, q=quit"))
        .child(Element::text(""))
        .child(
            Element::row()
                .width(Size::Fill)
                .height(Size::Fill)
                .gap(2)
                .child(easing_demo(focused, active))
                .child(position_demo(focused))
                .child(color_boxes(focused)),
        )
}

fn easing_demo(focused: Option<&str>, active: Option<&str>) -> Element {
    Element::col()
        .width(Size::Fixed(40))
        .height(Size::Fill)
        .style(
            Style::new()
                .background(Color::oklch(0.2, 0.02, 250.0))
                .border(Border::Rounded),
        )
        .padding(Edges::all(1))
        .gap(1)
        .child(Element::text("Easing Functions").style(Style::new().bold()))
        .child(Element::text("Click buttons to see transitions"))
        .child(Element::text(""))
        .child(button(
            "btn_linear",
            "Linear",
            Easing::Linear,
            focused,
            active,
        ))
        .child(button(
            "btn_ease_in",
            "Ease In",
            Easing::EaseIn,
            focused,
            active,
        ))
        .child(button(
            "btn_ease_out",
            "Ease Out",
            Easing::EaseOut,
            focused,
            active,
        ))
        .child(button(
            "btn_ease_inout",
            "Ease In-Out",
            Easing::EaseInOut,
            focused,
            active,
        ))
}

fn button(
    id: &str,
    label: &str,
    easing: Easing,
    focused: Option<&str>,
    active: Option<&str>,
) -> Element {
    let is_focused = focused == Some(id);
    let is_active = active == Some(id);

    // Different colors based on state
    let bg = if is_active {
        Color::oklch(0.6, 0.15, 140.0) // Bright green when active
    } else if is_focused {
        Color::oklch(0.45, 0.12, 250.0) // Blue when focused
    } else {
        Color::oklch(0.3, 0.05, 250.0) // Dim when normal
    };

    let fg = if is_active {
        Color::oklch(0.1, 0.0, 0.0) // Dark text on bright bg
    } else {
        Color::oklch(0.9, 0.02, 250.0) // Light text
    };

    Element::text(format!("  {}  ", label))
        .id(id)
        .focusable(true)
        .clickable(true)
        .width(Size::Fill)
        .style(Style::new().background(bg).foreground(fg).bold())
        .transitions(
            Transitions::new()
                .background(Duration::from_millis(300), easing)
                .foreground(Duration::from_millis(300), easing),
        )
}

fn position_demo(focused: Option<&str>) -> Element {
    Element::col()
        .width(Size::Fixed(35))
        .height(Size::Fill)
        .style(
            Style::new()
                .background(Color::oklch(0.2, 0.02, 60.0))
                .border(Border::Rounded),
        )
        .padding(Edges::all(1))
        .gap(1)
        .child(Element::text("Position Transitions").style(Style::new().bold()))
        .child(Element::text("Focus boxes to move them"))
        .child(Element::text(""))
        .child(
            // Container for absolute positioned elements
            Element::box_()
                .width(Size::Fill)
                .height(Size::Fill)
                .style(Style::new().background(Color::oklch(0.25, 0.03, 60.0)))
                .child(moving_box("move_1", 2, 1, focused))
                .child(moving_box("move_2", 2, 4, focused))
                .child(moving_box("move_3", 2, 7, focused)),
        )
}

fn moving_box(id: &str, base_left: i16, base_top: i16, focused: Option<&str>) -> Element {
    let is_focused = focused == Some(id);

    // Move right when focused
    let left = if is_focused {
        base_left + 15
    } else {
        base_left
    };

    let bg = if is_focused {
        Color::oklch(0.55, 0.15, 140.0)
    } else {
        Color::oklch(0.4, 0.1, 200.0)
    };

    Element::box_()
        .id(id)
        .focusable(true)
        .position(Position::Absolute)
        .left(left)
        .top(base_top)
        .width(Size::Fixed(12))
        .height(Size::Fixed(3))
        .style(
            Style::new()
                .background(bg)
                .border(Border::Rounded)
                .foreground(Color::oklch(0.9, 0.02, 0.0)),
        )
        .transitions(
            Transitions::new()
                .left(Duration::from_millis(400), Easing::EaseOut)
                .background(Duration::from_millis(300), Easing::EaseOut),
        )
        .child(Element::text(if is_focused {
            "→ Moving"
        } else {
            "  Static"
        }))
}

fn color_boxes(focused: Option<&str>) -> Element {
    Element::col()
        .width(Size::Fill)
        .height(Size::Fill)
        .style(
            Style::new()
                .background(Color::oklch(0.2, 0.02, 280.0))
                .border(Border::Rounded),
        )
        .padding(Edges::all(1))
        .gap(1)
        .child(Element::text("Color Transitions").style(Style::new().bold()))
        .child(Element::text("Focus boxes to see smooth color changes"))
        .child(Element::text(""))
        .child(
            Element::row()
                .width(Size::Fill)
                .height(Size::Fill)
                .gap(1)
                .child(color_box("box_red", 25.0, focused))
                .child(color_box("box_orange", 60.0, focused))
                .child(color_box("box_yellow", 90.0, focused))
                .child(color_box("box_green", 140.0, focused))
                .child(color_box("box_cyan", 190.0, focused))
                .child(color_box("box_blue", 250.0, focused))
                .child(color_box("box_purple", 300.0, focused)),
        )
}

fn color_box(id: &str, base_hue: f32, focused: Option<&str>) -> Element {
    let is_focused = focused == Some(id);

    // When focused, shift hue and increase lightness/chroma
    let (l, c, h) = if is_focused {
        (0.7, 0.18, (base_hue + 30.0) % 360.0)
    } else {
        (0.4, 0.1, base_hue)
    };

    Element::box_()
        .id(id)
        .focusable(true)
        .width(Size::Fill)
        .height(Size::Fill)
        .style(
            Style::new()
                .background(Color::oklch(l, c, h))
                .border(Border::Single),
        )
        .transitions(Transitions::new().background(Duration::from_millis(500), Easing::EaseInOut))
        .child(
            Element::text(if is_focused { "★" } else { "" })
                .style(Style::new().foreground(Color::oklch(0.1, 0.0, 0.0))),
        )
}
