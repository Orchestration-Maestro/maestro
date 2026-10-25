//! Screen resize policy around the retained page-list resize owner.

use super::{Screen, cursor::LinkRestore};
use crate::{
    Point,
    osc::semantic_prompt::Redraw,
    page::{StorageError, cell::SemanticContent},
    page_list::{Direction, TrackedPin},
    point::Tag,
};
use std::num::NonZeroU16;

/// New screen dimensions, soft-wrap policy and explicit prompt redraw policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resize {
    /// Positive column count.
    pub cols: NonZeroU16,
    /// Positive active height.
    pub rows: NonZeroU16,
    /// Rewrap soft lines when columns change.
    pub reflow: bool,
    /// Clear prompt content only when the caller supports redrawing it.
    pub prompt_redraw: Redraw,
}

impl Screen {
    /// Resize retained content while preserving cursor style and hyperlink values.
    ///
    /// # Errors
    /// Returns a managed-copy refusal. Earlier storage changes remain applied;
    /// reset the screen before using content after a failed resize.
    pub fn resize(&mut self, opts: Resize) -> Result<(), StorageError> {
        self.kitty_images.dirty = true;
        self.with_cursor_detached(LinkRestore::Reidentify, |screen| {
            let style = std::mem::take(&mut screen.cursor.style);
            let result = screen.resize_detached(opts);
            screen.cursor.style = style;
            result
        })
    }

    /// Resize with no page-local cursor references retained in mutable storage.
    fn resize_detached(&mut self, opts: Resize) -> Result<(), StorageError> {
        let saved = self
            .saved_cursor
            .and_then(|cursor| {
                self.pages.pin(Point::Active(crate::Coordinate {
                    x: cursor.x,
                    y: u32::from(cursor.y),
                }))
            })
            .and_then(|pin| self.pages.track_pin(pin));
        let result = self.resize_content(opts, saved);
        if let Some(saved) = saved {
            self.pages.untrack_pin(saved);
        }
        result
    }

    /// Apply prompt clearing, delegated reflow and successful cursor fixups.
    fn resize_content(
        &mut self,
        opts: Resize,
        saved: Option<TrackedPin>,
    ) -> Result<(), StorageError> {
        self.clear_resize_prompt(opts.prompt_redraw);
        self.pages.resize(crate::page_list::Resize {
            cols: Some(opts.cols),
            rows: Some(opts.rows),
            reflow: opts.reflow,
            cursor: Some(crate::page_list::Cursor {
                x: self.cursor.x,
                y: self.cursor.y,
                pin: Some(self.cursor.page_pin),
            }),
        })?;
        if self.no_scrollback {
            self.pages.erase_history(None);
        }
        self.cursor_reload();
        self.resize_saved_cursor(saved, opts.cols.get());
        Ok(())
    }

    /// Follow the saved location or reset it when it leaves the active area.
    fn resize_saved_cursor(&mut self, saved: Option<TrackedPin>, cols: u16) {
        let Some(saved) = saved else { return };
        let point = self
            .pages
            .tracked_pin(saved)
            .and_then(|pin| self.pages.point_from_pin(Tag::Active, *pin));
        let Some(cursor) = self.saved_cursor.as_mut() else {
            return;
        };
        if let Some(point) = point {
            let coordinate = point.coord();
            cursor.x = coordinate.x;
            cursor.y = u16::try_from(coordinate.y).unwrap_or_default();
            if cursor.pending_wrap && cursor.x != cols - 1 {
                cursor.pending_wrap = false;
                cursor.x += 1;
            }
        } else {
            cursor.x = 0;
            cursor.y = 0;
            cursor.pending_wrap = false;
        }
    }

    /// Clear the requested prompt rows without physically deleting screen space.
    fn clear_resize_prompt(&mut self, redraw: Redraw) {
        if redraw == Redraw::False || self.cursor.semantic_content == SemanticContent::Output {
            return;
        }
        let Ok(cursor) = self.cursor_pin() else {
            return;
        };
        if redraw == Redraw::Last {
            self.clear_prompt_row(cursor);
            return;
        }
        let Some(start) = cursor
            .prompt_iterator(Direction::LeftUp, None)
            .next(&self.pages)
        else {
            return;
        };
        let mut rows = start.row_iterator(Direction::RightDown, None);
        while let Some(pin) = rows.next(&self.pages) {
            self.clear_prompt_row(pin);
        }
    }

    /// Clear the stored width of one prompt row with the current blank cell.
    fn clear_prompt_row(&mut self, pin: crate::page_list::Pin) {
        if let Some(cols) = self.pages.page(pin.node).map(|page| page.size.cols) {
            self.clear_cells(pin.node, pin.y, 0..cols);
        }
    }
}
