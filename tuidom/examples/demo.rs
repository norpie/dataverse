use std::time::Duration;

use tuidom::{
    Border, Color, Direction, Edges, Element, Justify, Size, Style, Terminal, TextAlign, TextWrap,
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
        .gap(1)
        .child(Element::text("Text Features Demo").style(Style::new().bold()))
        .child(text_examples())
}

fn text_examples() -> Element {
    Element::row()
        .width(Size::Fill)
        .height(Size::Fill)
        .gap(1)
        .child(
            // Word wrap example
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
                    Element::text("This is a longer sentence that will wrap at word boundaries automatically.")
                        .text_wrap(TextWrap::WordWrap),
                ),
        )
        .child(
            // Truncation example
            Element::col()
                .width(Size::Fixed(20))
                .height(Size::Fill)
                .style(
                    Style::new()
                        .background(Color::oklch(0.2, 0.03, 150.0))
                        .border(Border::Single),
                )
                .padding(Edges::all(1))
                .child(Element::text("Truncate:").style(Style::new().bold()))
                .child(
                    Element::text("This text will be truncated with ellipsis")
                        .text_wrap(TextWrap::Truncate),
                ),
        )
        .child(
            // Alignment examples
            Element::col()
                .width(Size::Fill)
                .height(Size::Fill)
                .style(
                    Style::new()
                        .background(Color::oklch(0.2, 0.03, 100.0))
                        .border(Border::Single),
                )
                .padding(Edges::all(1))
                .gap(1)
                .child(Element::text("Alignment:").style(Style::new().bold()))
                .child(Element::text("Left aligned").text_align(TextAlign::Left))
                .child(Element::text("Center aligned").text_align(TextAlign::Center))
                .child(Element::text("Right aligned").text_align(TextAlign::Right))
                .child(Element::text(""))
                .child(Element::text("Unicode:").style(Style::new().bold()))
                .child(Element::text("日本語テスト"))
                .child(Element::text("한글 테스트"))
                .child(Element::text("Emoji: 😀🎉✨")),
        )
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
