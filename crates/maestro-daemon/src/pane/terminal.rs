//! Process bytes and retain pane policy over the terminal engine.

mod colors;
mod cursor;
mod modes;
#[cfg(any(windows, test))]
mod prompt_cwd;
mod reads;
mod resize;
mod retained;
mod stream;

pub use modes::{InputMode, InputState};
pub use reads::TerminalReadSnapshot;
pub use retained::{TerminalTextMatch, TerminalTextPoint, TerminalWordMotion};

use super::{
    osc::{AgentOscStateTracker, DefaultColorEventTracker, OscDebugTracker},
    xtgettcap::XtgettcapQueryTracker,
};
use crate::terminal_theme::TerminalTheme;
use maestro_terminal::{Terminal, render::RenderState};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

/// Scroll position within retained terminal rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollMetrics {
    /// Rows between the viewport bottom and the live bottom.
    pub offset_from_bottom: usize,
    /// Greatest available bottom-relative offset.
    pub max_offset_from_bottom: usize,
    /// Rows in the viewport.
    pub viewport_rows: usize,
}

/// Cursor position and presentation within a pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalCursorState {
    /// Viewport column.
    pub x: u16,
    /// Viewport row.
    pub y: u16,
    /// Whether to draw the cursor.
    pub visible: bool,
    /// DECSCUSR parameter, with zero selecting the terminal default.
    pub shape: u8,
}

/// Effects of one live PTY write, independent of input delivery.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProcessBytesResult {
    /// Whether the renderer should redraw now.
    pub request_render: bool,
    /// Optional follow-up delay for image or cursor settling.
    pub render_delay: Option<Duration>,
    /// Whether a completed title changed the display title.
    pub terminal_title_changed: bool,
    /// Live bell count, saturated by the engine.
    pub terminal_bells: u16,
    /// Accepted live clipboard writes.
    pub clipboard_writes: Vec<Vec<u8>>,
    /// Latest accepted live working directory.
    pub reported_cwd: Option<PathBuf>,
    /// Ordered replies to send directly to the child PTY.
    pub terminal_responses: Vec<Vec<u8>>,
}

/// One pane's concrete terminal and retained presentation policy.
pub struct PaneTerminal {
    /// Single owner for screen state and its policy observations.
    core: Mutex<PaneCore>,
}

/// Screen state and policy updated atomically for a pane.
struct PaneCore {
    /// Concrete emulator, including parser and queued effects.
    terminal: Terminal,
    /// Retained viewport snapshot for rendering.
    render_state: RenderState,
    /// Replies preserved while a host appearance update drains its own report.
    pending_pty_responses: Vec<Vec<u8>>,
    /// Most recently observed host colours.
    host_terminal_theme: TerminalTheme,
    /// Foreground process group that changed default colours.
    transient_default_color_owner_pgid: Option<u32>,
    /// Whether the child owns the foreground override.
    child_default_foreground_changed: bool,
    /// Whether the child owns the background override.
    child_default_background_changed: bool,
    /// Host colour policy observations in byte order.
    default_color_event_tracker: DefaultColorEventTracker,
    /// Diagnostic OSC evidence.
    osc_debug_tracker: OscDebugTracker,
    /// Retained display title and agent evidence.
    agent_osc_state: AgentOscStateTracker,
    /// Capability queries not answered by the emulator.
    xtgettcap_query_tracker: XtgettcapQueryTracker,
    /// Timed cursor position presentation.
    cursor_settle_state: super::cursor::CursorPositionSettleState,
    /// Whether image rendering needs the 20 ms redraw settle backstop.
    kitty_graphics_enabled: bool,
    /// Whether this known shell reports cwd through its default prompt.
    #[cfg(any(windows, test))]
    windows_powershell_prompt_cwd_reporting: InputMode,
}

impl PaneTerminal {
    /// Own an initialized emulator with one lock for pane policy.
    #[must_use]
    pub fn new(terminal: Terminal) -> Self {
        Self {
            core: Mutex::new(PaneCore {
                terminal,
                render_state: RenderState::default(),
                pending_pty_responses: Vec::new(),
                host_terminal_theme: TerminalTheme::default(),
                transient_default_color_owner_pgid: None,
                child_default_foreground_changed: false,
                child_default_background_changed: false,
                default_color_event_tracker: DefaultColorEventTracker::default(),
                osc_debug_tracker: OscDebugTracker::default(),
                agent_osc_state: AgentOscStateTracker::default(),
                xtgettcap_query_tracker: XtgettcapQueryTracker::default(),
                cursor_settle_state: super::cursor::CursorPositionSettleState::default(),
                kitty_graphics_enabled: false,
                #[cfg(any(windows, test))]
                windows_powershell_prompt_cwd_reporting: InputMode::Disabled,
            }),
        }
    }

    /// Restore retained ANSI history before live processing.
    pub fn seed_history_ansi(&self, ansi: &str) {
        if let Ok(mut core) = self.core.lock() {
            core.terminal.write(ansi.as_bytes());
        }
    }

    /// Latest display title, independent of cleared agent evidence.
    #[must_use]
    pub fn terminal_title(&self) -> Option<String> {
        self.core
            .lock()
            .ok()?
            .agent_osc_state
            .terminal_title()
            .map(str::to_owned)
    }

    /// Restore only the display title from a handoff.
    pub fn seed_terminal_title(&self, title: Option<String>) {
        if let Ok(mut core) = self.core.lock() {
            core.agent_osc_state.seed_terminal_title(title);
        }
    }

    /// Latest agent title, empty when no evidence is retained.
    #[must_use]
    pub fn agent_osc_title(&self) -> String {
        self.core
            .lock()
            .map(|core| core.agent_osc_state.latest_title().to_owned())
            .unwrap_or_default()
    }

    /// Latest agent progress, empty when no evidence is retained.
    #[must_use]
    pub fn agent_osc_progress(&self) -> String {
        self.core
            .lock()
            .map(|core| core.agent_osc_state.latest_progress().to_owned())
            .unwrap_or_default()
    }

    /// Clear agent evidence without changing display title or incomplete framing.
    pub fn clear_agent_osc_state(&self) {
        if let Ok(mut core) = self.core.lock() {
            core.agent_osc_state.clear_retained();
        }
    }
}

#[cfg(test)]
mod tests;
