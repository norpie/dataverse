use tuidom::{
    Border, Color, Edges, Element, Event, FocusState, Key, Size, Style, Terminal, TextInputState,
};

fn main() -> std::io::Result<()> {
    let mut term = Terminal::new()?;
    let mut focus = FocusState::new();
    let mut text_inputs = TextInputState::new();

    text_inputs.set("input", "");

    loop {
        let is_focused = focus.focused() == Some("input");
        let data = text_inputs.get_data("input");

        let root = Element::col()
            .width(Size::Fill)
            .height(Size::Fill)
            .style(Style::new().background(Color::oklch(0.15, 0.01, 250.0)))
            .padding(Edges::all(2))
            .gap(1)
            .child(Element::text("Text Input Demo - type something, Esc to quit"))
            .child(Element::text("Shift+Arrow to select, Ctrl+A to select all"))
            .child(Element::text(""))
            .child(
                Element::text_input("")
                    .id("input")
                    .width(Size::Fixed(40))
                    .placeholder("Type here...")
                    .input_state(data.unwrap_or(&Default::default()), is_focused)
                    .style(
                        Style::new()
                            .background(Color::oklch(0.2, 0.02, 250.0))
                            .border(Border::Single),
                    ),
            )
            .child(Element::text(""))
            .child(Element::text(format!(
                "You typed: {}",
                text_inputs.get("input")
            )));

        term.render(&root)?;

        let raw_events = term.poll(None)?;
        let events = focus.process_events(&raw_events, &root, term.layout());
        let events = text_inputs.process_events(&events, &root, term.layout());

        for event in &events {
            if let Event::Key {
                key: Key::Escape, ..
            } = event
            {
                return Ok(());
            }
        }
    }
}
