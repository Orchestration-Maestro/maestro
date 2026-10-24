//! Cursor position settling.

use super::display::TerminalCursorState;
use std::time::{Duration, Instant};

/// Quiet interval before presenting a pending cursor position.
pub const CURSOR_POSITION_SETTLE: Duration = Duration::from_millis(20);
/// Longest burst over which the settled position is held.
const CURSOR_POSITION_MAX_HOLD: Duration = Duration::from_millis(100);

/// Retains a stable cursor position across short output bursts.
#[derive(Debug, Default)]
pub struct CursorPositionSettleState {
    /// Last stable or hidden cursor.
    settled: Option<TerminalCursorState>,
    /// Latest pending position.
    candidate: Option<TerminalCursorState>,
    /// First movement in the current burst.
    pending_since: Option<Instant>,
}

impl CursorPositionSettleState {
    /// Observe cursor state using the supplied monotonic timestamp.
    pub fn observe(&mut self, current: Option<TerminalCursorState>, now: Instant) {
        let Some(current) = current else {
            self.settle(None);
            return;
        };
        if !current.visible {
            self.settle(Some(current));
            return;
        }
        let Some(settled) = self.settled else {
            self.settle(Some(current));
            return;
        };
        if same_cursor_position(settled, current) && settled.visible {
            self.settle(Some(current));
            return;
        }

        let Some(candidate) = self.candidate else {
            self.candidate = Some(current);
            self.pending_since = Some(now);
            return;
        };

        let pending_since = self.pending_since.unwrap_or(now);
        if now.duration_since(pending_since) >= CURSOR_POSITION_MAX_HOLD {
            self.settle(Some(current));
        } else if same_cursor_position(candidate, current) {
            if now.duration_since(pending_since) >= CURSOR_POSITION_SETTLE {
                self.settle(Some(current));
            } else {
                self.candidate = Some(current);
            }
        } else {
            self.candidate = Some(current);
        }
    }

    /// Commit a position and end its pending burst.
    fn settle(&mut self, current: Option<TerminalCursorState>) {
        self.settled = current;
        self.candidate = None;
        self.pending_since = None;
    }

    /// Read the presented cursor without advancing the settle state.
    #[must_use]
    pub fn reported_cursor(
        &self,
        current: Option<TerminalCursorState>,
        now: Instant,
    ) -> Option<TerminalCursorState> {
        let current = current?;
        let Some(candidate) = self.candidate else {
            return Some(current);
        };
        let pending_since = self.pending_since.unwrap_or(now);
        if now.duration_since(pending_since) >= CURSOR_POSITION_SETTLE {
            return Some(TerminalCursorState {
                visible: current.visible && candidate.visible,
                shape: current.shape,
                ..candidate
            });
        }
        self.settled
            .map(|settled| TerminalCursorState {
                visible: current.visible && settled.visible,
                shape: current.shape,
                ..settled
            })
            .or(Some(TerminalCursorState {
                visible: false,
                shape: current.shape,
                ..candidate
            }))
    }

    /// Whether a position is waiting to settle.
    #[must_use]
    pub fn pending(&self) -> bool {
        self.candidate.is_some()
    }
}

/// Compare position independently of shape and visibility.
fn same_cursor_position(left: TerminalCursorState, right: TerminalCursorState) -> bool {
    left.x == right.x && left.y == right.y
}

#[cfg(test)]
mod tests;
