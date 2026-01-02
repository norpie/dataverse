use std::fs::File;
use std::time::Duration;

use simplelog::{Config, LevelFilter, WriteLogger};
use tuidom::{Align, Color, Edges, Element, Event, FocusState, Key, Size, Style, Terminal};

fn main() -> std::io::Result<()> {
    // Set up file logging
    let log_file = File::create("spinner.log")?;
    WriteLogger::init(LevelFilter::Debug, Config::default(), log_file)
        .expect("Failed to initialize logger");

    let mut term = Terminal::new()?;
    let mut focus = FocusState::new();

    loop {
        let root = ui();
        term.render(&root)?;

        // Poll with a timeout - frame animations will wake us up when needed
        let raw_events = term.poll(Some(Duration::from_secs(1)))?;
        let events = focus.process_events(&raw_events, &root, term.layout());

        for event in events {
            if let Event::Key {
                key: Key::Char('q'),
                ..
            }
            | Event::Key {
                key: Key::Escape, ..
            } = &event
            {
                return Ok(());
            }
        }
    }
}

fn ui() -> Element {
    Element::col()
        .width(Size::Fill)
        .height(Size::Fill)
        .style(Style::new().background(Color::oklch(0.15, 0.01, 250.0)))
        .padding(Edges::all(2))
        .gap(2)
        .child(
            Element::text("Frame Animations Demo")
                .style(Style::new().bold().foreground(Color::oklch(0.9, 0.0, 0.0))),
        )
        .child(Element::text("Press 'q' to quit").style(Style::new().dim()))
        .child(Element::text(""))
        // Braille spinner
        .child(spinner_row(
            "Braille dots",
            braille_spinner().id("spinner_braille"),
            "Loading...",
        ))
        // Line spinner
        .child(spinner_row(
            "Line spinner",
            line_spinner().id("spinner_line"),
            "Processing...",
        ))
        // Arrow spinner
        .child(spinner_row(
            "Arrow spinner",
            arrow_spinner().id("spinner_arrow"),
            "Working...",
        ))
        // Bounce spinner
        .child(spinner_row(
            "Bounce",
            bounce_spinner().id("spinner_bounce"),
            "Bouncing...",
        ))
        // Clock spinner
        .child(spinner_row(
            "Clock",
            clock_spinner().id("spinner_clock"),
            "Time passes...",
        ))
        // Progress bar animation
        .child(spinner_row(
            "Progress",
            progress_spinner().id("spinner_progress"),
            "Progressing...",
        ))
        // Custom emoji spinner
        .child(spinner_row(
            "Moon phases",
            moon_spinner().id("spinner_moon"),
            "Lunar cycle...",
        ))
        // Snake with gradient
        .child(snake_row())
        .child(Element::text(""))
        .child(
            Element::text("All spinners animate at different speeds")
                .style(Style::new().italic().dim()),
        )
}

fn spinner_row(label: &str, spinner: Element, status: &str) -> Element {
    Element::row()
        .width(Size::Fill)
        .height(Size::Fixed(1))
        .gap(2)
        .align(Align::Center)
        .child(
            Element::text(format!("{:15}", label))
                .width(Size::Fixed(17))
                .style(Style::new().foreground(Color::oklch(0.7, 0.1, 200.0))),
        )
        .child(
            spinner
                .width(Size::Fixed(4))
                .style(Style::new().foreground(Color::oklch(0.8, 0.15, 140.0))),
        )
        .child(Element::text(status).style(Style::new().dim()))
}

fn snake_row() -> Element {
    Element::row()
        .width(Size::Fill)
        .height(Size::Fixed(1))
        .gap(2)
        .align(Align::Center)
        .child(
            Element::text(format!("{:15}", "Snake"))
                .width(Size::Fixed(17))
                .style(Style::new().foreground(Color::oklch(0.7, 0.1, 200.0))),
        )
        .child(snake_spinner().id("spinner_snake").width(Size::Fixed(8)))
        .child(Element::text("Bouncing...").style(Style::new().dim()))
}

/// Classic braille dot spinner
fn braille_spinner() -> Element {
    Element::frames(
        vec![
            Element::text("\u{28F7}"), // ⣷
            Element::text("\u{28EF}"), // ⣯
            Element::text("\u{28DF}"), // ⣟
            Element::text("\u{287F}"), // ⡿
            Element::text("\u{28BF}"), // ⢿
            Element::text("\u{28FB}"), // ⣻
            Element::text("\u{28FD}"), // ⣽
            Element::text("\u{28FE}"), // ⣾
        ],
        Duration::from_millis(80),
    )
}

/// Simple line spinner
fn line_spinner() -> Element {
    Element::frames(
        vec![
            Element::text("|"),
            Element::text("/"),
            Element::text("-"),
            Element::text("\\"),
        ],
        Duration::from_millis(100),
    )
}

/// Arrow spinner
fn arrow_spinner() -> Element {
    Element::frames(
        vec![
            Element::text("\u{2190}"), // ←
            Element::text("\u{2196}"), // ↖
            Element::text("\u{2191}"), // ↑
            Element::text("\u{2197}"), // ↗
            Element::text("\u{2192}"), // →
            Element::text("\u{2198}"), // ↘
            Element::text("\u{2193}"), // ↓
            Element::text("\u{2199}"), // ↙
        ],
        Duration::from_millis(120),
    )
}

/// Bounce animation
fn bounce_spinner() -> Element {
    Element::frames(
        vec![
            Element::text("\u{28C0}"), // ⣀
            Element::text("\u{2880}"), // ⢀
            Element::text("\u{2800}"), // ⠀ (empty braille)
            Element::text("\u{2801}"), // ⠁
            Element::text("\u{2809}"), // ⠉
            Element::text("\u{2819}"), // ⠙
            Element::text("\u{281B}"), // ⠛
            Element::text("\u{2819}"), // ⠙
            Element::text("\u{2809}"), // ⠉
            Element::text("\u{2801}"), // ⠁
            Element::text("\u{2800}"), // ⠀
            Element::text("\u{2880}"), // ⢀
        ],
        Duration::from_millis(100),
    )
}

/// Clock face spinner
fn clock_spinner() -> Element {
    Element::frames(
        vec![
            Element::text("\u{1F55B}"), // 🕛
            Element::text("\u{1F550}"), // 🕐
            Element::text("\u{1F551}"), // 🕑
            Element::text("\u{1F552}"), // 🕒
            Element::text("\u{1F553}"), // 🕓
            Element::text("\u{1F554}"), // 🕔
            Element::text("\u{1F555}"), // 🕕
            Element::text("\u{1F556}"), // 🕖
            Element::text("\u{1F557}"), // 🕗
            Element::text("\u{1F558}"), // 🕘
            Element::text("\u{1F559}"), // 🕙
            Element::text("\u{1F55A}"), // 🕚
        ],
        Duration::from_millis(200),
    )
}

/// Progress bar animation
fn progress_spinner() -> Element {
    Element::frames(
        vec![
            Element::text("[  ]"),
            Element::text("[= ]"),
            Element::text("[==]"),
            Element::text("[ =]"),
        ],
        Duration::from_millis(150),
    )
}

/// Moon phases
fn moon_spinner() -> Element {
    Element::frames(
        vec![
            Element::text("\u{1F311}"), // 🌑
            Element::text("\u{1F312}"), // 🌒
            Element::text("\u{1F313}"), // 🌓
            Element::text("\u{1F314}"), // 🌔
            Element::text("\u{1F315}"), // 🌕
            Element::text("\u{1F316}"), // 🌖
            Element::text("\u{1F317}"), // 🌗
            Element::text("\u{1F318}"), // 🌘
        ],
        Duration::from_millis(200),
    )
}

/// Configuration for the snake spinner
struct SnakeConfig {
    track_width: i32,
    snake_len: i32,
    right_pause: usize,
    left_pause: usize,
    frame_ms: u64,
    hue: f32,
}

impl Default for SnakeConfig {
    fn default() -> Self {
        Self {
            track_width: 8,
            snake_len: 6,
            right_pause: 1,
            left_pause: 20,
            frame_ms: 60,
            hue: 320.0,
        }
    }
}

/// Bouncing snake with gradient - uses default config
fn snake_spinner() -> Element {
    snake_spinner_with_config(SnakeConfig::default())
}

/// Bouncing snake with custom configuration
fn snake_spinner_with_config(config: SnakeConfig) -> Element {
    Element::frames(
        generate_snake_frames(&config),
        Duration::from_millis(config.frame_ms),
    )
}

fn generate_snake_frames(config: &SnakeConfig) -> Vec<Element> {
    let mut frames = Vec::new();

    // Right pass: snake enters from left, travels across, exits right
    for head_pos in 0..=(config.track_width + config.snake_len - 2) {
        frames.push(make_snake_frame(head_pos, config, true));
    }

    // Pause after going right
    for _ in 0..config.right_pause {
        frames.push(make_empty_frame(config));
    }

    // Left pass: snake enters from right, travels across, exits left
    for head_pos in (0..=(config.track_width + config.snake_len - 2)).rev() {
        frames.push(make_snake_frame(head_pos, config, false));
    }

    // Pause after going left
    for _ in 0..config.left_pause {
        frames.push(make_empty_frame(config));
    }

    frames
}

fn make_empty_frame(config: &SnakeConfig) -> Element {
    let bg_color = Color::oklch(0.45, 0.12, config.hue);
    let mut row = Element::row();
    for _ in 0..config.track_width {
        row = row.child(Element::text("⬝").style(Style::new().foreground(bg_color.clone())));
    }
    row
}

fn make_snake_frame(head_pos: i32, config: &SnakeConfig, moving_right: bool) -> Element {
    let bg_color = Color::oklch(0.45, 0.12, config.hue);

    let mut row = Element::row();

    for i in 0..config.track_width {
        // Snake body is at positions (head_pos - snake_len + 1) to head_pos
        let snake_start = head_pos - config.snake_len + 1;

        let child = if i >= snake_start && i <= head_pos {
            // This position has snake
            let snake_idx = i - snake_start; // 0 = leftmost, snake_len-1 = rightmost

            // When moving right: head is on right (high index = bright)
            // When moving left: head is on left (low index = bright)
            let t = if moving_right {
                snake_idx as f32 / (config.snake_len - 1) as f32
            } else {
                1.0 - (snake_idx as f32 / (config.snake_len - 1) as f32)
            };

            // Interpolate color: tail is dim (t=0), head is bright (t=1)
            let l = 0.30 + t * 0.45; // 0.30 to 0.75
            let c = 0.06 + t * 0.14; // 0.06 to 0.20

            Element::text("■").style(Style::new().foreground(Color::oklch(l, c, config.hue)))
        } else {
            // Background
            Element::text("⬝").style(Style::new().foreground(bg_color.clone()))
        };

        row = row.child(child);
    }

    row
}
