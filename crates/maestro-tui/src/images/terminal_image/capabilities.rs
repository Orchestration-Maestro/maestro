//! Environment-based terminal support detection.
use super::{ImageProtocol, TerminalCapabilities, TerminalImage};

impl TerminalImage {
    /// Reads current terminal hints without populating the cache.
    #[must_use]
    pub fn detect_capabilities(&self) -> TerminalCapabilities {
        let program = (self.environment)("TERM_PROGRAM")
            .unwrap_or_default()
            .to_lowercase();
        let term = (self.environment)("TERM")
            .unwrap_or_default()
            .to_lowercase();
        let color = (self.environment)("COLORTERM")
            .unwrap_or_default()
            .to_lowercase();
        let present = |key| (self.environment)(key).is_some_and(|value| !value.is_empty());
        let true_color = matches!(color.as_str(), "truecolor" | "24bit");
        if present("TMUX") || term.starts_with("tmux") || term.starts_with("screen") {
            return TerminalCapabilities {
                true_color,
                ..TerminalCapabilities::default()
            };
        }
        let images = if present("KITTY_WINDOW_ID")
            || program == "kitty"
            || program == "ghostty"
            || term.contains("ghostty")
            || present("GHOSTTY_RESOURCES_DIR")
            || present("WEZTERM_PANE")
            || program == "wezterm"
        {
            Some(ImageProtocol::Kitty)
        } else if present("ITERM_SESSION_ID") || program == "iterm.app" {
            Some(ImageProtocol::Iterm2)
        } else {
            None
        };
        let recognized = images.is_some() || matches!(program.as_str(), "vscode" | "alacritty");
        TerminalCapabilities {
            images,
            true_color: recognized || true_color,
            hyperlinks: recognized,
        }
    }
}
