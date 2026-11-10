//! Absolute-row text selections within a pane.

use crate::panes::display::ScrollMetrics;
use maestro_workspace::tiling::{PaneId, Rect};

/// Current phase of a selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Mouse is down but hasn't moved yet. If released without
    /// moving, this was just a click — no selection created.
    Anchored,
    /// Mouse has moved from the anchor point. Cells are being highlighted.
    Dragging,
    /// Mouse released after dragging. Selection is visible and the gesture is
    /// complete. Clipboard policy is owned by the caller.
    Done,
}

/// A text selection within a terminal pane.
#[derive(Debug, Clone)]
pub struct Selection {
    /// Which pane the selection belongs to.
    pub pane_id: PaneId,
    /// Anchor position in screen-buffer coordinates (row, col).
    anchor: (u32, u16),
    /// Current/final position in screen-buffer coordinates (row, col).
    cursor: (u32, u16),
    /// Selection phase.
    phase: Phase,
}

impl Selection {
    /// Start a potential selection. This records the anchor but doesn't
    /// make anything visible yet — the user might just be clicking.
    #[must_use]
    pub fn anchor(
        pane_id: PaneId,
        viewport_row: u16,
        col: u16,
        metrics: Option<ScrollMetrics>,
    ) -> Self {
        let anchor = (absolute_row_for_viewport_row(viewport_row, metrics), col);
        Self {
            pane_id,
            anchor,
            cursor: anchor,
            phase: Phase::Anchored,
        }
    }

    /// Create an active selection from an explicit viewport-row range.
    #[must_use]
    pub fn range(
        pane_id: PaneId,
        viewport_row: u16,
        start_col: u16,
        end_col: u16,
        metrics: Option<ScrollMetrics>,
    ) -> Self {
        let row = absolute_row_for_viewport_row(viewport_row, metrics);
        Self {
            pane_id,
            anchor: (row, start_col),
            cursor: (row, end_col),
            phase: Phase::Dragging,
        }
    }

    /// Select whole absolute rows in either drag direction.
    #[must_use]
    pub fn line_range(pane_id: PaneId, anchor_row: u32, cursor_row: u32, end_col: u16) -> Self {
        let (anchor_col, cursor_col) = if anchor_row <= cursor_row {
            (0, end_col)
        } else {
            (end_col, 0)
        };
        Self {
            pane_id,
            anchor: (anchor_row, anchor_col),
            cursor: (cursor_row, cursor_col),
            phase: Phase::Dragging,
        }
    }

    /// Convert a viewport row to its retained buffer row.
    #[must_use]
    pub fn absolute_row_for_viewport(viewport_row: u16, metrics: Option<ScrollMetrics>) -> u32 {
        absolute_row_for_viewport_row(viewport_row, metrics)
    }

    /// Convert the anchor's absolute row and pane-relative column back to
    /// screen coordinates. Adds the pane origin before clamping so the
    /// returned (`screen_row`, `screen_col`) can be compared directly against
    /// mouse screen positions.
    #[must_use]
    pub fn anchor_screen_pos(
        &self,
        pane_inner: Rect,
        metrics: Option<ScrollMetrics>,
    ) -> (u16, u16) {
        let viewport_row = viewport_row_for_absolute_row(self.anchor.0, metrics);
        // Convert pane-relative to screen coordinates, then clamp.
        let row = (viewport_row.saturating_add(pane_inner.y)).clamp(
            pane_inner.y,
            pane_inner
                .y
                .saturating_add(pane_inner.height.saturating_sub(1)),
        );
        let col = (self.anchor.1.saturating_add(pane_inner.x)).clamp(
            pane_inner.x,
            pane_inner
                .x
                .saturating_add(pane_inner.width.saturating_sub(1)),
        );
        (row, col)
    }

    /// Extend the selection as the mouse drags. Activates highlighting
    /// once the cursor moves to a different cell than the anchor.
    /// Screen coordinates are clamped to the pane boundary.
    pub fn drag(
        &mut self,
        screen_col: u16,
        screen_row: u16,
        pane_inner: Rect,
        metrics: Option<ScrollMetrics>,
    ) {
        let (viewport_row, col) = clamp_to_pane(screen_col, screen_row, pane_inner);
        self.cursor = (absolute_row_for_viewport_row(viewport_row, metrics), col);
        if self.cursor != self.anchor {
            self.phase = Phase::Dragging;
        }
    }

    /// Finalize the selection, returning true for a drag and false for a plain click.
    pub fn finish(&mut self) -> bool {
        if self.phase == Phase::Dragging {
            self.phase = Phase::Done;
            true
        } else {
            false
        }
    }

    /// Whether this selection should be rendered (highlight visible).
    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.phase == Phase::Dragging || self.phase == Phase::Done
    }

    /// Whether this selection was already finalized.
    #[must_use]
    pub fn is_finalized(&self) -> bool {
        self.phase == Phase::Done
    }

    /// Whether the user just clicked without dragging (not a selection).
    #[must_use]
    pub fn was_just_click(&self) -> bool {
        self.is_just_click()
    }

    /// Whether the user just clicked without dragging (not a selection).
    #[must_use]
    pub fn is_just_click(&self) -> bool {
        self.phase == Phase::Anchored
    }

    /// Force the selection into Dragging phase, used when the mouse
    /// has moved off the anchor cell but `drag()` couldn't transition
    /// because the cursor was clamped to the same cell as the anchor.
    pub fn force_dragging(&mut self) {
        if self.phase == Phase::Anchored {
            self.phase = Phase::Dragging;
        }
    }

    /// Whether the pointer is still down and the selection can keep extending.
    #[must_use]
    pub fn is_in_progress(&self) -> bool {
        matches!(self.phase, Phase::Anchored | Phase::Dragging)
    }

    /// Whether the user is actively dragging (cursor moved from anchor).
    #[must_use]
    pub fn is_dragging(&self) -> bool {
        self.phase == Phase::Dragging
    }

    /// Return the inclusive endpoints in reading order.
    #[must_use]
    pub fn ordered_cells(&self) -> ((u32, u16), (u32, u16)) {
        let (ar, ac) = self.anchor;
        let (cr, cc) = self.cursor;
        if ar < cr || (ar == cr && ac <= cc) {
            ((ar, ac), (cr, cc))
        } else {
            ((cr, cc), (ar, ac))
        }
    }

    /// Check whether a pane-relative cell (row, col) is inside the selection.
    #[must_use]
    pub fn contains(&self, viewport_row: u16, col: u16, metrics: Option<ScrollMetrics>) -> bool {
        if !self.is_visible() {
            return false;
        }
        let row = absolute_row_for_viewport_row(viewport_row, metrics);
        let ((sr, sc), (er, ec)) = self.ordered_cells();
        if row < sr || row > er {
            return false;
        }
        if sr == er {
            col >= sc && col <= ec
        } else if row == sr {
            col >= sc
        } else if row == er {
            col <= ec
        } else {
            true
        }
    }
}

/// First retained row currently shown.
fn viewport_top_row(metrics: Option<ScrollMetrics>) -> u32 {
    let top = metrics.map_or(0, |metrics| {
        metrics
            .max_offset_from_bottom
            .saturating_sub(metrics.offset_from_bottom)
    });
    u32::try_from(top).unwrap_or(u32::MAX)
}

/// Add the retained viewport origin.
fn absolute_row_for_viewport_row(viewport_row: u16, metrics: Option<ScrollMetrics>) -> u32 {
    viewport_top_row(metrics).saturating_add(u32::from(viewport_row))
}

/// Subtract the retained viewport origin.
fn viewport_row_for_absolute_row(absolute_row: u32, metrics: Option<ScrollMetrics>) -> u16 {
    absolute_row
        .saturating_sub(viewport_top_row(metrics))
        .try_into()
        .unwrap_or(0)
}

/// Clamp screen coordinates and return pane-relative row and column.
fn clamp_to_pane(screen_col: u16, screen_row: u16, pane_inner: Rect) -> (u16, u16) {
    let clamped_col = screen_col.clamp(
        pane_inner.x,
        pane_inner
            .x
            .saturating_add(pane_inner.width.saturating_sub(1)),
    );
    let clamped_row = screen_row.clamp(
        pane_inner.y,
        pane_inner
            .y
            .saturating_add(pane_inner.height.saturating_sub(1)),
    );
    (clamped_row - pane_inner.y, clamped_col - pane_inner.x)
}

#[cfg(test)]
mod tests;
