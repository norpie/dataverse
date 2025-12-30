use std::fs::File;

use crossterm::event::{Event as CtEvent, KeyCode, KeyEventKind};
use simplelog::{Config, LevelFilter, WriteLogger};
use tuidom::{
    Align, Backdrop, Border, Color, Edges, Element, Justify, Position, Size, Style, Terminal,
};

fn main() -> std::io::Result<()> {
    // Set up file logging
    let log_file = File::create("modal.log")?;
    WriteLogger::init(LevelFilter::Debug, Config::default(), log_file)
        .expect("Failed to initialize logger");

    let mut term = Terminal::new()?;

    loop {
        let (width, height) = term.size();
        let root = ui(width, height);
        term.render(&root)?;

        let raw_events = term.poll(None)?;

        for event in &raw_events {
            if let CtEvent::Key(key_event) = event {
                if key_event.kind == KeyEventKind::Press {
                    match key_event.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        _ => {}
                    }
                }
            }
        }
    }
}

fn ui(width: u16, height: u16) -> Element {
    Element::box_()
        .width(Size::Fill)
        .height(Size::Fill)
        // Background content (will be dimmed by modal)
        .child(background_content())
        // Modal overlay with dim backdrop
        .child(modal(width, height))
}

fn background_content() -> Element {
    Element::col()
        .width(Size::Fill)
        .height(Size::Fill)
        .style(Style::new().background(Color::oklch(0.25, 0.08, 250.0)))
        .padding(Edges::all(2))
        .gap(1)
        .child(
            Element::text("Background Application")
                .style(Style::new().bold().foreground(Color::oklch(0.9, 0.05, 250.0))),
        )
        .child(Element::text("This is some content behind the modal."))
        .child(Element::text("It will be dimmed when the modal is shown."))
        .child(Element::text(""))
        .child(
            Element::row()
                .gap(2)
                .child(colored_box("Red", 25.0))
                .child(colored_box("Orange", 60.0))
                .child(colored_box("Yellow", 90.0))
                .child(colored_box("Green", 140.0))
                .child(colored_box("Blue", 250.0))
                .child(colored_box("Purple", 300.0)),
        )
        .child(Element::text(""))
        .child(Element::text("Press 'q' to quit"))
}

fn colored_box(label: &str, hue: f32) -> Element {
    Element::col()
        .width(Size::Fixed(12))
        .height(Size::Fixed(5))
        .style(
            Style::new()
                .background(Color::oklch(0.5, 0.15, hue))
                .border(Border::Rounded)
                .foreground(Color::oklch(0.95, 0.02, hue)),
        )
        .justify(Justify::Center)
        .align(Align::Center)
        .child(Element::text(label))
}

fn modal(width: u16, height: u16) -> Element {
    // Calculate center position
    let modal_width = 50u16;
    let modal_height = 9u16;
    let left = (width.saturating_sub(modal_width)) / 2;
    let top = (height.saturating_sub(modal_height)) / 2;

    Element::col()
        .id("modal")
        .position(Position::Absolute)
        .left(left as i16)
        .top(top as i16)
        .width(Size::Fixed(modal_width))
        .height(Size::Fixed(modal_height))
        // This dims the entire buffer before rendering the modal
        .backdrop(Backdrop::Dim(0.5))
        .style(
            Style::new()
                .background(Color::oklch(0.2, 0.02, 250.0))
                .border(Border::Rounded)
                .foreground(Color::oklch(0.9, 0.02, 250.0)),
        )
        .padding(Edges::all(1))
        .gap(1)
        .child(
            Element::text("Modal Dialog")
                .style(Style::new().bold().foreground(Color::oklch(0.95, 0.08, 140.0))),
        )
        .child(Element::text("This modal has a dimmed backdrop."))
        .child(Element::text("Notice the colorful boxes are now darker."))
}
