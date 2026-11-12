//! Terminal geometry and screen-specific resize policies.

use super::{ScrollingRegion, SizeError, TABSTOP_INTERVAL, Terminal};
use crate::{
    Mode, osc::semantic_prompt::Redraw, page::StorageError, screen, screens::Key,
    tabstops::Tabstops,
};
use std::num::NonZeroU16;

/// Refusal to resize terminal geometry or retained content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizeError {
    /// A requested grid dimension was zero.
    Size(SizeError),
    /// Retained managed content could not be copied.
    Storage(StorageError),
}

impl std::fmt::Display for ResizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Size(error) => error.fmt(f),
            Self::Storage(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ResizeError {}

impl Terminal {
    /// Resize the grid and effective host pixels, ending synchronized output on success.
    ///
    /// Screen pixels and mode 2048 reports clamp cell dimensions to at least one.
    /// CSI 14/16/18 queries retain the raw caller dimensions and emit nothing when
    /// either cell dimension is zero. Each successful resize queues an in-band
    /// report if mode 2048 is enabled, including resizes to unchanged dimensions.
    ///
    /// # Errors
    /// Rejects zero grid dimensions before mutation. A storage refusal may leave earlier
    /// screen changes applied; reset before using content after that refusal.
    pub fn resize(
        &mut self,
        cols: u16,
        rows: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    ) -> Result<(), ResizeError> {
        self.resize_grid(cols, rows)?;
        self.width_px = u32::from(cols).saturating_mul(cell_width_px.max(1));
        self.height_px = u32::from(rows).saturating_mul(cell_height_px.max(1));
        self.modes.set(Mode::SynchronizedOutput, false);
        self.size_report = crate::size_report::Size {
            columns: cols,
            rows,
            cell_width: cell_width_px,
            cell_height: cell_height_px,
        };
        if self.modes.get(Mode::InBandSizeReports) {
            let size = crate::size_report::Size {
                cell_width: cell_width_px.max(1),
                cell_height: cell_height_px.max(1),
                ..self.size_report
            };
            let _ = crate::size_report::encode(
                &mut self.effects.pty_reply,
                crate::size_report::Style::Mode2048,
                size,
            );
        }
        Ok(())
    }

    /// Resize retained screens without altering independently supplied pixel geometry.
    pub(super) fn resize_grid(&mut self, cols: u16, rows: u16) -> Result<(), ResizeError> {
        let width = NonZeroU16::new(cols).ok_or(ResizeError::Size(SizeError))?;
        let height = NonZeroU16::new(rows).ok_or(ResizeError::Size(SizeError))?;
        if (self.cols, self.rows) == (cols, rows) {
            return Ok(());
        }
        if self.cols != cols {
            self.tabstops = Tabstops::init(usize::from(cols), TABSTOP_INTERVAL);
        }
        // Primary storage is always resident, regardless of the selected screen.
        if let Some(primary) = self.screens.get_mut(Key::Primary) {
            primary
                .resize(screen::Resize {
                    cols: width,
                    rows: height,
                    reflow: self.modes.get(Mode::Wraparound),
                    prompt_redraw: self.flags.shell_redraws_prompt,
                })
                .map_err(ResizeError::Storage)?;
        }
        if let Some(alternate) = self.screens.get_mut(Key::Alternate) {
            alternate
                .resize(screen::Resize {
                    cols: width,
                    rows: height,
                    reflow: false,
                    prompt_redraw: Redraw::False,
                })
                .map_err(ResizeError::Storage)?;
        }
        self.flags.dirty.clear = true;
        self.cols = cols;
        self.rows = rows;
        self.scrolling_region = ScrollingRegion {
            top: 0,
            bottom: rows - 1,
            left: 0,
            right: cols - 1,
        };
        Ok(())
    }
}
