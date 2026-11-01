//! Dirty viewport rows collected as frame-record patches.

use super::{clear_render_dirty, style::CellStyler};
use crate::render::frame::color_to_u32;
use maestro_app::daemon::{PaneRenderView, PaneTerminal, render_prof};
use maestro_app::terminal::Dirty;
use maestro_protocol::frames::CellData;
use ratatui::style::{Color, Style};
use std::time::Instant;

/// Changed viewport rows as frame-record cells, padded to the area width.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalDirtyPatch {
    /// Row index and cells of each changed row, top to bottom.
    pub rows: Vec<(u16, Vec<CellData>)>,
}

/// What collecting a pane's dirty rows found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalDirtyPatchOutcome {
    /// Nothing changed since the last acknowledgement.
    Clean,
    /// Only these rows changed.
    Patch(TerminalDirtyPatch),
    /// Rows cannot be patched, so a full frame is needed.
    Fallback,
}

/// Collect the rows of a pane's viewport that changed since they were last
/// painted or collected, acknowledging them; a global change or a hyperlink
/// in a changed row asks for a full frame instead.
#[must_use]
pub fn collect_dirty_patch(
    pane: &PaneTerminal,
    area_width: u16,
    area_height: u16,
) -> TerminalDirtyPatchOutcome {
    pane.with_render_view(Instant::now(), |mut view| {
        let outcome = patch(&mut view, area_width, area_height).unwrap_or_else(|reason| {
            render_prof::event(reason);
            TerminalDirtyPatchOutcome::Fallback
        });
        if let Some(started) = view.prof_started {
            record(&outcome, started);
        }
        outcome
    })
    .unwrap_or(TerminalDirtyPatchOutcome::Fallback)
}

/// Build the patch, or name the profile event explaining why rows cannot be
/// patched.
fn patch(
    view: &mut PaneRenderView<'_>,
    area_width: u16,
    area_height: u16,
) -> Result<TerminalDirtyPatchOutcome, &'static str> {
    match view.render_state.dirty {
        Dirty::Clean => return Ok(TerminalDirtyPatchOutcome::Clean),
        Dirty::Full => return Err("dirty_fallback.dirty_full"),
        Dirty::Partial => {}
    }
    let styler = CellStyler::new(view);
    let width = usize::from(area_width);
    let mut rows = Vec::new();
    let mut symbol = String::new();
    for (y, row) in (0..area_height).zip(view.render_state.rows()) {
        if !row.dirty() {
            continue;
        }
        let mut cells = Vec::with_capacity(width);
        for cell in row.cells().take(width) {
            if cell.raw().hyperlink() {
                return Err("dirty_fallback.hyperlink_present");
            }
            styler.symbol(&cell, &mut symbol);
            cells.push(cell_data(std::mem::take(&mut symbol), styler.style(&cell)));
        }
        cells.resize_with(width, || cell_data(" ".to_owned(), styler.blank_style()));
        rows.push((y, cells));
    }
    clear_render_dirty(view.render_state, area_height);
    Ok(TerminalDirtyPatchOutcome::Patch(TerminalDirtyPatch {
        rows,
    }))
}

/// A frame-record cell; absent colours are the terminal default.
fn cell_data(symbol: String, style: Style) -> CellData {
    CellData {
        symbol,
        fg: color_to_u32(style.fg.unwrap_or(Color::Reset)),
        bg: color_to_u32(style.bg.unwrap_or(Color::Reset)),
        modifier: style.add_modifier.bits(),
        skip: false,
        hyperlink: None,
    }
}

/// Report a collection's outcome to the opt-in render profiler.
fn record(outcome: &TerminalDirtyPatchOutcome, started: Instant) {
    render_prof::duration("dirty_collect.total", started.elapsed());
    match outcome {
        TerminalDirtyPatchOutcome::Clean => render_prof::event("dirty_collect.clean"),
        TerminalDirtyPatchOutcome::Fallback => render_prof::event("dirty_collect.fallback"),
        TerminalDirtyPatchOutcome::Patch(patch) => {
            render_prof::event("dirty_collect.patch");
            render_prof::counter("dirty_collect.rows", patch.rows.len() as u64);
            let cells = patch.rows.iter().map(|(_, cells)| cells.len() as u64).sum();
            render_prof::counter("dirty_collect.cells", cells);
        }
    }
}
