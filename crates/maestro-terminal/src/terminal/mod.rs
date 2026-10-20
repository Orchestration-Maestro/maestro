//! Direct operations on terminal screens and retained protocol state.

mod alignment;
mod chars;
mod colors;
mod cursor;
mod erase;
mod line_edit;
mod lines;
mod margins;
mod print;
mod prompt;
mod pwd_title;
mod read;
mod reads;
mod resize;
mod screens;
pub use read::{ScreenTextCell, ScreenTextRow, TextRange, TextShape};
pub use reads::ScrollViewport;
pub use resize::ResizeError;
pub use screens::{DeccolmMode, SwitchScreenMode};

use std::num::NonZeroU16;

use crate::{
    Mode, ScreenSet,
    color::{DynamicPalette, DynamicRgb},
    modes::ModeState,
    mouse,
    screen::Options,
    tabstops::Tabstops,
};

/// Default interval between tab destinations.
const TABSTOP_INTERVAL: usize = 8;

/// A terminal dimension was zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SizeError;

impl std::fmt::Display for SizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("terminal dimensions must be nonzero")
    }
}

impl std::error::Error for SizeError {}

/// Renderer invalidation for terminal-wide state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dirty {
    /// Palette or dynamic colours changed.
    pub palette: bool,
    /// The screen requires a full redraw.
    pub clear: bool,
}

/// Inclusive boundaries of the scrolling region, in zero-based cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScrollingRegion {
    /// First scrolling row.
    pub top: u16,
    /// Last scrolling row.
    pub bottom: u16,
    /// First scrolling column.
    pub left: u16,
    /// Last scrolling column.
    pub right: u16,
}

/// Independently retained terminal colours.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Colors {
    /// Background default and override.
    pub background: DynamicRgb,
    /// Foreground default and override.
    pub foreground: DynamicRgb,
    /// Cursor default and override.
    pub cursor: DynamicRgb,
    /// Indexed palette defaults and overrides.
    pub palette: DynamicPalette,
}

impl Default for Colors {
    fn default() -> Self {
        Self {
            background: DynamicRgb::UNSET,
            foreground: DynamicRgb::UNSET,
            cursor: DynamicRgb::UNSET,
            palette: DynamicPalette::DEFAULT,
        }
    }
}

/// Terminal-wide input and renderer state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flags {
    /// Whether the window has focus.
    pub focused: bool,
    /// Whether modified keys use the extended encoding.
    pub modify_other_keys_2: bool,
    /// Explicit selection of shift capture, or no request yet.
    pub mouse_shift_capture: Option<bool>,
    /// The last selected mouse event mode.
    pub mouse_event: mouse::Event,
    /// The last selected mouse encoding.
    pub mouse_format: mouse::Format,
    /// Explicit shell permission to clear prompt content when resizing.
    pub shell_redraws_prompt: crate::osc::semantic_prompt::Redraw,
    /// Terminal-wide renderer invalidation.
    pub dirty: Dirty,
}

impl Default for Flags {
    fn default() -> Self {
        Self {
            focused: true,
            mouse_shift_capture: None,
            modify_other_keys_2: false,
            mouse_event: mouse::Event::None,
            mouse_format: mouse::Format::X10,
            shell_redraws_prompt: crate::osc::semantic_prompt::Redraw::False,
            dirty: Dirty::default(),
        }
    }
}

/// Retained terminal screens with direct text and cursor operations.
pub struct Terminal {
    /// Whether this terminal admits PNG decoding during image execution.
    pub(crate) kitty_png_enabled: bool,
    /// APC routing and caller-selected image payload limit.
    pub apc_handler: crate::apc::Handler,
    /// Caller-selected scheme for status reports.
    pub(crate) color_scheme: Option<crate::device_status::ColorScheme>,
    /// Last caller geometry, before clamping cell pixels for the screen.
    pub(crate) size_report: crate::size_report::Size,
    /// Pending effects captured by the byte consumer.
    pub(crate) effects: crate::effects::Effects,
    /// Screen storage and selection.
    pub screens: ScreenSet,
    /// Current, saved and reset-default mode values.
    pub modes: ModeState,
    /// Terminal input and renderer flags.
    pub flags: Flags,
    /// Current scrolling boundaries.
    pub scrolling_region: ScrollingRegion,
    /// Dynamic colour state.
    pub colors: Colors,
    /// Selected text display.
    pub status_display: crate::ansi::StatusDisplay,
    /// Last printed scalar used for repetition.
    pub previous_char: Option<char>,
    /// Screen width in pixels; zero until supplied by the host.
    pub width_px: u32,
    /// Screen height in pixels; zero until supplied by the host.
    pub height_px: u32,
    /// Directory bytes supplied by the shell.
    pwd: Vec<u8>,
    /// Title bytes supplied by the caller.
    title: Vec<u8>,
    /// Retained tab destinations.
    tabstops: Tabstops,
    /// Configured grid width.
    cols: u16,
    /// Configured grid height.
    rows: u16,
    /// Parser and cursor defaults retained between byte writes.
    pub(crate) write_state: Option<crate::stream_terminal::WriteState>,
    /// Configured history byte limit, before storage rounding.
    max_scrollback: usize,
    /// Previous viewport row anchor with its owning screen lifetime.
    tracked_row: Option<(crate::screen_set::Key, usize, crate::page_list::TrackedPin)>,
}

impl Terminal {
    /// Construct a blank terminal with grapheme clustering enabled.
    ///
    /// # Errors
    /// Returns `SizeError` if either dimension is zero.
    pub fn new(cols: u16, rows: u16, scrollback_limit: usize) -> Result<Self, SizeError> {
        let opts = Options {
            cols: NonZeroU16::new(cols).ok_or(SizeError)?,
            rows: NonZeroU16::new(rows).ok_or(SizeError)?,
            max_scrollback: scrollback_limit,
        };
        let mut modes = ModeState::default();
        modes.set(Mode::GraphemeCluster, true);
        modes.defaults[Mode::GraphemeCluster as usize] = true;
        Ok(Self {
            kitty_png_enabled: false,
            apc_handler: crate::apc::Handler::default(),
            color_scheme: None,
            size_report: crate::size_report::Size {
                columns: cols,
                rows,
                cell_width: 0,
                cell_height: 0,
            },
            effects: crate::effects::Effects::default(),
            screens: ScreenSet::init(opts),
            modes,
            flags: Flags::default(),
            scrolling_region: ScrollingRegion {
                top: 0,
                bottom: rows - 1,
                left: 0,
                right: cols - 1,
            },
            colors: Colors::default(),
            pwd: Vec::new(),
            title: Vec::new(),
            previous_char: None,
            status_display: crate::ansi::StatusDisplay::Main,
            tabstops: Tabstops::init(usize::from(cols), TABSTOP_INTERVAL),
            width_px: 0,
            height_px: 0,
            cols,
            rows,
            max_scrollback: scrollback_limit,
            tracked_row: None,
            write_state: None,
        })
    }

    /// Configured grid width.
    #[must_use]
    pub fn cols(&self) -> u16 {
        self.cols
    }

    /// Configured grid height.
    #[must_use]
    pub fn rows(&self) -> u16 {
        self.rows
    }

    /// Configured history limit in bytes.
    #[must_use]
    pub fn max_scrollback(&self) -> usize {
        self.max_scrollback
    }
}

#[cfg(test)]
mod tests;
