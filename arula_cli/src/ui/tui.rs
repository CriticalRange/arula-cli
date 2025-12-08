use std::io::{self, Stdout};
use anyhow::Result;

use ratatui::{
    backend::CrosstermBackend,
    Terminal,
    Viewport,
    TerminalOptions,
};

/// A renderer that draws TUI widgets inline at the bottom of the terminal
/// using Ratatui's native inline viewport.
pub struct InlineRenderer {
    pub terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl InlineRenderer {
    /// Create a new inline renderer with a fixed height viewport
    pub fn new(height: u16) -> Result<Self> {
        let stdout = io::stdout();
        let backend = CrosstermBackend::new(stdout);
        
        let viewport = Viewport::Inline(height);
        let terminal = Terminal::with_options(
            backend,
            TerminalOptions { viewport }
        )?;
        
        Ok(Self {
            terminal,
        })
    }

    /// Resize the inline viewport
    pub fn resize(&mut self, height: u16) -> Result<()> {
        let stdout = io::stdout();
        let backend = CrosstermBackend::new(stdout);
        let viewport = Viewport::Inline(height);
        self.terminal = Terminal::with_options(backend, TerminalOptions { viewport })?;
        Ok(())
    }
    
    /// Clear the inline viewport (remove it from view)
    pub fn clear(&mut self) -> Result<()> {
        self.terminal.clear()?;
        Ok(())
    }
}
