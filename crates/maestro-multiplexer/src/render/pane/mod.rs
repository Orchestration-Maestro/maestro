//! Pane terminal cells painted into frame areas or collected as dirty-row
//! patches.

mod dirty;
mod style;
#[cfg(test)]
mod tests;

pub use dirty::{TerminalDirtyPatch, TerminalDirtyPatchOutcome, collect_dirty_patch};

use maestro_app::daemon::{PaneRenderView, PaneTerminal};
use maestro_app::terminal::{Dirty, RenderRow, RenderState};
use ratatui::{Frame, layout::Rect};
use std::time::Instant;
use style::{CellStyler, paint};

/// Paint a pane's viewport into a frame area, resetting cells the viewport
/// does not cover, and place the host cursor at the pane's visible cursor
/// when `show_cursor` is set.
pub fn render(pane: &PaneTerminal, frame: &mut Frame<'_>, area: Rect, show_cursor: bool) {
    pane.with_render_view(Instant::now(), |mut view| {
        render_view(&mut view, frame, area, show_cursor);
    });
}

/// Paint one borrowed snapshot, then acknowledge the painted rows.
fn render_view(
    view: &mut PaneRenderView<'_>,
    frame: &mut Frame<'_>,
    area: Rect,
    show_cursor: bool,
) {
    let styler = CellStyler::new(view);
    let buffer = frame.buffer_mut();
    let mut rows = view.render_state.rows();
    let mut symbol = String::new();
    for y in 0..area.height {
        let row = rows.next();
        let mut cells = row.iter().flat_map(RenderRow::cells);
        for x in 0..area.width {
            let cell = cells.next();
            let position = (area.x.saturating_add(x), area.y.saturating_add(y));
            let Some(target) = buffer.cell_mut(position) else {
                continue;
            };
            match cell {
                Some(cell) => {
                    styler.symbol(&cell, &mut symbol);
                    paint(target, &symbol, styler.style(&cell));
                }
                None => paint(target, " ", styler.blank_style()),
            }
        }
    }
    drop(rows);
    clear_render_dirty(view.render_state, area.height);
    let cursor = view.cursor.filter(|cursor| {
        show_cursor && cursor.visible && cursor.x < area.width && cursor.y < area.height
    });
    if let Some(cursor) = cursor {
        frame.set_cursor_position((area.x + cursor.x, area.y + cursor.y));
    }
}

/// Acknowledge the rows an area shows and the snapshot's global invalidation.
fn clear_render_dirty(render_state: &mut RenderState, area_height: u16) {
    for mut row in render_state.rows().take(usize::from(area_height)) {
        row.clear_dirty();
    }
    render_state.dirty = Dirty::Clean;
}
