//! Terminal setup and teardown with panic safety.

use std::io::{self, Stdout};
use std::panic;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

/// A wrapper around the terminal that ensures proper cleanup on drop.
pub struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalGuard {
    /// Initialize the terminal for TUI rendering.
    pub fn new() -> io::Result<Self> {
        // Set up panic hook to restore terminal on panic
        let original_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            // Attempt to restore terminal
            let _ = restore_terminal();
            // Call original panic handler
            original_hook(panic_info);
        }));

        // Enable raw mode and alternate screen
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self { terminal })
    }

    /// Get mutable access to the terminal for rendering.
    pub fn terminal(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }

    /// Get the terminal size.
    #[allow(dead_code)]
    pub fn size(&self) -> io::Result<ratatui::layout::Size> {
        self.terminal.size()
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = restore_terminal();
    }
}

/// Restore the terminal to its original state.
fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}
