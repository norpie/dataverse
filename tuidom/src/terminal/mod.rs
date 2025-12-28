use std::io::{self, Write};
use std::time::Duration;

use crossterm::{
    cursor,
    event::{self, Event as CrosstermEvent},
    execute,
    style::{Attribute, Color as CtColor, SetAttribute, SetBackgroundColor, SetForegroundColor},
    terminal,
};

use crate::buffer::Buffer;
use crate::element::Element;
use crate::layout::{layout, LayoutResult, Rect};
use crate::render::render_to_buffer;
use crate::text::char_width;
use crate::types::Rgb;

pub struct Terminal {
    stdout: io::Stdout,
    current_buffer: Buffer,
    previous_buffer: Buffer,
    last_layout: LayoutResult,
}

impl Terminal {
    pub fn new() -> io::Result<Self> {
        let mut stdout = io::stdout();

        terminal::enable_raw_mode()?;
        execute!(
            stdout,
            terminal::EnterAlternateScreen,
            cursor::Hide,
            event::EnableMouseCapture
        )?;

        let (width, height) = terminal::size()?;
        let current_buffer = Buffer::new(width, height);
        let previous_buffer = Buffer::new(width, height);

        Ok(Self {
            stdout,
            current_buffer,
            previous_buffer,
            last_layout: LayoutResult::new(),
        })
    }

    pub fn size(&self) -> (u16, u16) {
        (self.current_buffer.width(), self.current_buffer.height())
    }

    pub fn poll(&self, timeout: Option<Duration>) -> io::Result<Vec<CrosstermEvent>> {
        let mut events = Vec::new();

        let has_event = match timeout {
            Some(dur) => event::poll(dur)?,
            None => {
                // Block until event
                events.push(event::read()?);
                return Ok(events);
            }
        };

        if has_event {
            events.push(event::read()?);
            // Drain any additional pending events
            while event::poll(Duration::ZERO)? {
                events.push(event::read()?);
            }
        }

        Ok(events)
    }

    pub fn render(&mut self, root: &Element) -> io::Result<&LayoutResult> {
        // Check if terminal size changed
        let (width, height) = terminal::size()?;
        if width != self.current_buffer.width() || height != self.current_buffer.height() {
            self.current_buffer = Buffer::new(width, height);
            self.previous_buffer = Buffer::new(width, height);
        }

        // Clear current buffer
        self.current_buffer.clear();

        // Layout
        let available = Rect::from_size(width, height);
        self.last_layout = layout(root, available);

        // Render to buffer
        render_to_buffer(root, &self.last_layout, &mut self.current_buffer);

        // Diff and write changes
        self.flush_diff()?;

        // Swap buffers
        std::mem::swap(&mut self.current_buffer, &mut self.previous_buffer);

        Ok(&self.last_layout)
    }

    /// Get the layout from the last render.
    pub fn layout(&self) -> &LayoutResult {
        &self.last_layout
    }

    fn flush_diff(&mut self) -> io::Result<()> {
        let mut last_x = u16::MAX;
        let mut last_y = u16::MAX;
        let mut last_char_width: u16 = 1;
        let mut last_fg = Rgb::new(255, 255, 255);
        let mut last_bg = Rgb::new(0, 0, 0);
        let mut last_style = crate::types::TextStyle::new();

        // Reset to known state at start
        execute!(self.stdout, SetAttribute(Attribute::Reset))?;

        for (x, y, cell) in self.current_buffer.diff(&self.previous_buffer) {
            // Skip wide character continuation cells - the wide char already occupies this space
            if cell.wide_continuation {
                continue;
            }

            // Move cursor if not sequential (accounting for wide chars)
            if y != last_y || x != last_x + last_char_width {
                execute!(self.stdout, cursor::MoveTo(x, y))?;
            }

            // Update colors if changed
            if cell.fg != last_fg {
                execute!(
                    self.stdout,
                    SetForegroundColor(CtColor::Rgb {
                        r: cell.fg.r,
                        g: cell.fg.g,
                        b: cell.fg.b,
                    })
                )?;
                last_fg = cell.fg;
            }

            if cell.bg != last_bg {
                execute!(
                    self.stdout,
                    SetBackgroundColor(CtColor::Rgb {
                        r: cell.bg.r,
                        g: cell.bg.g,
                        b: cell.bg.b,
                    })
                )?;
                last_bg = cell.bg;
            }

            // Apply text style changes
            if cell.style.bold != last_style.bold {
                if cell.style.bold {
                    execute!(self.stdout, SetAttribute(Attribute::Bold))?;
                } else {
                    execute!(self.stdout, SetAttribute(Attribute::NormalIntensity))?;
                }
            }
            if cell.style.dim != last_style.dim {
                if cell.style.dim {
                    execute!(self.stdout, SetAttribute(Attribute::Dim))?;
                } else {
                    execute!(self.stdout, SetAttribute(Attribute::NormalIntensity))?;
                }
            }
            if cell.style.italic != last_style.italic {
                if cell.style.italic {
                    execute!(self.stdout, SetAttribute(Attribute::Italic))?;
                } else {
                    execute!(self.stdout, SetAttribute(Attribute::NoItalic))?;
                }
            }
            if cell.style.underline != last_style.underline {
                if cell.style.underline {
                    execute!(self.stdout, SetAttribute(Attribute::Underlined))?;
                } else {
                    execute!(self.stdout, SetAttribute(Attribute::NoUnderline))?;
                }
            }
            last_style = cell.style;

            // Write character
            write!(self.stdout, "{}", cell.char)?;

            last_x = x;
            last_y = y;
            last_char_width = char_width(cell.char).max(1) as u16;
        }

        // Reset at end
        execute!(self.stdout, SetAttribute(Attribute::Reset))?;
        self.stdout.flush()?;
        Ok(())
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = execute!(
            self.stdout,
            event::DisableMouseCapture,
            cursor::Show,
            terminal::LeaveAlternateScreen
        );
        let _ = terminal::disable_raw_mode();
    }
}
