use std::time::Duration;

use tuidom::{
    Border, Color, Direction, Edges, Element, Justify, Size, Style, Terminal,
};

fn main() -> std::io::Result<()> {
    let mut term = Terminal::new()?;

    loop {
        let events = term.poll(Some(Duration::from_millis(100)))?;

        // Exit on 'q' or Escape
        for event in &events {
            if let crossterm::event::Event::Key(key) = event {
                match key.code {
                    crossterm::event::KeyCode::Char('q') | crossterm::event::KeyCode::Esc => {
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }

        let root = ui();
        term.render(&root)?;
    }
}

fn ui() -> Element {
    Element::col()
        .padding(Edges::all(1))
        .gap(1)
        .child(header())
        .child(content())
        .child(footer())
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
        .child(Element::text("tuidom demo").style(Style::new().bold()))
}

fn content() -> Element {
    Element::row()
        .width(Size::Fill)
        .height(Size::Fill)
        .gap(1)
        .child(sidebar())
        .child(main_panel())
}

fn sidebar() -> Element {
    Element::col()
        .width(Size::Fixed(20))
        .height(Size::Fill)
        .style(
            Style::new()
                .background(Color::oklch(0.2, 0.02, 250.0))
                .border(Border::Single),
        )
        .padding(Edges::all(1))
        .gap(1)
        .child(Element::text("Navigation"))
        .child(Element::text("- Dashboard"))
        .child(Element::text("- Settings"))
        .child(Element::text("- About"))
}

fn main_panel() -> Element {
    Element::col()
        .width(Size::Fill)
        .height(Size::Fill)
        .style(
            Style::new()
                .background(Color::oklch(0.15, 0.01, 250.0))
                .border(Border::Single),
        )
        .padding(Edges::all(1))
        .child(Element::text("Main Content Area"))
        .child(Element::text(""))
        .child(Element::text("Press 'q' to quit"))
}

fn footer() -> Element {
    Element::box_()
        .width(Size::Fill)
        .height(Size::Fixed(1))
        .direction(Direction::Row)
        .justify(Justify::SpaceBetween)
        .style(Style::new().background(Color::oklch(0.25, 0.02, 250.0)))
        .child(Element::text("Status: Ready"))
        .child(Element::text("tuidom v0.1"))
}
