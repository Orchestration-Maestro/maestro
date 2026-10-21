//! Retained selection ownership across focus navigation.

use maestro_workspace::layout::PaneId;

/// Active daemon interaction context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Workspace navigation without terminal input.
    Navigate,
    /// Input directed to the active terminal.
    Terminal,
    /// Keyboard selection in the retained copy pane.
    Copy,
}

/// Selection shape retained by keyboard copy navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyModeSelection {
    /// Selection between character cells.
    Character,
    /// Whole-line selection from an absolute buffer row.
    Linewise {
        /// First selected absolute buffer row.
        anchor_row: u32,
    },
}

/// Copy-pane ownership retained while focus visits another pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyModeState {
    /// Pane to which copy navigation returns.
    pub pane_id: PaneId,
    /// Active selection shape, cleared when leaving the pane.
    pub selection: Option<CopyModeSelection>,
}
