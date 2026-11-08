//! Retained selection ownership across focus navigation.

use maestro_workspace::layout::PaneId;

/// Active daemon interaction context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Workspace navigation without terminal input.
    Navigate,
    /// Input directed to the active terminal.
    Terminal,
    /// Confirmation before closing a parent repository workspace group.
    ConfirmClose,
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

/// Direction retained for selection drag scrolling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionAutoscrollDirection {
    /// Extend toward earlier buffer rows.
    Up,
    /// Extend toward later buffer rows.
    Down,
}

/// Selection drag geometry retained until the gesture or its pane ends.
#[derive(Debug, Clone)]
pub struct SelectionAutoscroll {
    /// Direction requested by the current drag.
    pub direction: SelectionAutoscrollDirection,
    /// Last observed mouse column.
    pub last_mouse_screen_col: u16,
    /// Last observed mouse row.
    pub last_mouse_screen_row: u16,
    /// Pane content rectangle at the last drag observation.
    pub inner_rect: maestro_workspace::layout::Rect,
}
