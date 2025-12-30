use std::collections::HashMap;
use std::io::{self, Write};
use std::time::{Duration, Instant};

use crossterm::{
    cursor,
    event::{self, Event as CrosstermEvent},
    execute,
    style::{Attribute, Color as CtColor, SetAttribute, SetBackgroundColor, SetForegroundColor},
    terminal,
};

use crate::animation::{collect_element_ids, AnimationState};
use crate::buffer::Buffer;
use crate::element::Element;
use crate::layout::{layout, LayoutResult, Rect};
use crate::render::render_to_buffer;
use crate::text::char_width;
use crate::types::{Oklch, Rgb};

/// Cache for OKLCH → RGB conversions within a frame.
/// Uses quantized key to allow f32 hashing.
struct RgbCache {
    cache: HashMap<(i32, i32, i32), Rgb>,
}

impl RgbCache {
    fn new() -> Self {
        Self {
            cache: HashMap::with_capacity(64),
        }
    }

    fn get(&mut self, oklch: Oklch) -> Rgb {
        // Quantize to reasonable precision (avoids f32 hash issues)
        let key = (
            (oklch.l * 1000.0) as i32,
            (oklch.c * 10000.0) as i32, // chroma needs more precision
            (oklch.h * 10.0) as i32,
        );
        *self.cache.entry(key).or_insert_with(|| oklch.to_rgb())
    }
}

pub struct Terminal {
    stdout: io::Stdout,
    current_buffer: Buffer,
    previous_buffer: Buffer,
    last_layout: LayoutResult,
    animation: AnimationState,
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
            animation: AnimationState::new(),
        })
    }

    pub fn size(&self) -> (u16, u16) {
        (self.current_buffer.width(), self.current_buffer.height())
    }

    pub fn poll(&self, timeout: Option<Duration>) -> io::Result<Vec<CrosstermEvent>> {
        let mut events = Vec::new();

        // Use short timeout when transitions are active for smooth animation
        let effective_timeout = if self.animation.has_active_transitions() {
            Some(
                timeout
                    .map(|t| t.min(Duration::from_millis(16)))
                    .unwrap_or(Duration::from_millis(16)),
            )
        } else {
            timeout
        };

        let has_event = match effective_timeout {
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

    /// Enable or disable reduced motion (accessibility).
    /// When enabled, transitions complete instantly.
    pub fn set_reduced_motion(&mut self, enabled: bool) {
        self.animation.set_reduced_motion(enabled);
    }

    /// Returns true if any transitions are currently active.
    pub fn has_active_transitions(&self) -> bool {
        self.animation.has_active_transitions()
    }

    pub fn render(&mut self, root: &Element) -> io::Result<&LayoutResult> {
        let t_start = Instant::now();

        // Check if terminal size changed
        let (width, height) = terminal::size()?;
        if width != self.current_buffer.width() || height != self.current_buffer.height() {
            self.current_buffer = Buffer::new(width, height);
            self.previous_buffer = Buffer::new(width, height);
        }
        let t_resize = Instant::now();

        // Update animation state (detect property changes, start/complete transitions)
        self.animation.update(root);
        let t_animation = Instant::now();

        // Clear current buffer
        self.current_buffer.clear();
        let t_clear = Instant::now();

        // Layout
        let available = Rect::from_size(width, height);
        self.last_layout = layout(root, available);
        let t_layout = Instant::now();

        // Render to buffer with animation state for interpolated values
        render_to_buffer(root, &self.last_layout, &mut self.current_buffer, &self.animation);
        let t_render = Instant::now();

        // Diff and write changes
        self.flush_diff()?;
        let t_flush = Instant::now();

        // Swap buffers
        std::mem::swap(&mut self.current_buffer, &mut self.previous_buffer);

        // Cleanup animation state for removed elements
        let current_ids = collect_element_ids(root);
        self.animation.cleanup(&current_ids);
        let t_cleanup = Instant::now();

        log::debug!(
            "render: resize={:>6.2}µs animation={:>6.2}µs clear={:>6.2}µs layout={:>6.2}µs render={:>6.2}µs flush={:>6.2}µs cleanup={:>6.2}µs total={:>6.2}µs",
            t_resize.duration_since(t_start).as_secs_f64() * 1_000_000.0,
            t_animation.duration_since(t_resize).as_secs_f64() * 1_000_000.0,
            t_clear.duration_since(t_animation).as_secs_f64() * 1_000_000.0,
            t_layout.duration_since(t_clear).as_secs_f64() * 1_000_000.0,
            t_render.duration_since(t_layout).as_secs_f64() * 1_000_000.0,
            t_flush.duration_since(t_render).as_secs_f64() * 1_000_000.0,
            t_cleanup.duration_since(t_flush).as_secs_f64() * 1_000_000.0,
            t_cleanup.duration_since(t_start).as_secs_f64() * 1_000_000.0,
        );

        Ok(&self.last_layout)
    }

    /// Get the layout from the last render.
    pub fn layout(&self) -> &LayoutResult {
        &self.last_layout
    }

    fn flush_diff(&mut self) -> io::Result<()> {
        let mut rgb_cache = RgbCache::new();
        let mut last_x = u16::MAX;
        let mut last_y = u16::MAX;
        let mut last_char_width: u16 = 1;
        let mut last_fg = Oklch::new(1.0, 0.0, 0.0); // white
        let mut last_bg = Oklch::new(0.0, 0.0, 0.0); // black
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

            // Update colors if changed (convert to RGB via cache)
            if cell.fg != last_fg {
                let rgb = rgb_cache.get(cell.fg);
                execute!(
                    self.stdout,
                    SetForegroundColor(CtColor::Rgb {
                        r: rgb.r,
                        g: rgb.g,
                        b: rgb.b,
                    })
                )?;
                last_fg = cell.fg;
            }

            if cell.bg != last_bg {
                let rgb = rgb_cache.get(cell.bg);
                execute!(
                    self.stdout,
                    SetBackgroundColor(CtColor::Rgb {
                        r: rgb.r,
                        g: rgb.g,
                        b: rgb.b,
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
