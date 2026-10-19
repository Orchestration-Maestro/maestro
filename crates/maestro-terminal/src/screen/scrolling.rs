//! Screen cursor scrolling and viewport operations over the retained page list.

use super::{Screen, cursor::LinkRestore};
use crate::{Coordinate, Point, page::StorageError, page_list::Scroll, style::Color};

impl Screen {
    /// Move the viewport within existing retained history.
    pub fn scroll(&mut self, behavior: Scroll) {
        self.pages.scroll(behavior);
    }

    /// Move nonempty active rows into history and reload the tracked cursor.
    pub fn scroll_clear(&mut self) {
        self.with_cursor_detached(LinkRestore::PageChange, |screen| {
            screen.pages.scroll_clear();
            screen.cursor_reload();
        });
    }

    /// Whether the viewport follows the active area.
    #[must_use]
    pub fn viewport_is_bottom(&self) -> bool {
        self.pages.viewport_is_active()
    }

    /// Physically remove history through an inclusive endpoint, or all history.
    pub fn erase_history(&mut self, bottom: Option<Point>) {
        self.with_cursor_detached(LinkRestore::PageChange, |screen| {
            screen.pages.erase_history(bottom);
            screen.cursor_reload();
        });
    }

    /// Physically remove active rows through an inclusive row index.
    pub fn erase_active(&mut self, y: u16) {
        self.with_cursor_detached(LinkRestore::PageChange, |screen| {
            screen.pages.erase_active(y);
            screen.cursor_reload();
        });
    }

    /// Scroll at the active bottom, retaining history unless disabled.
    ///
    /// # Errors
    /// Returns a managed row-copy refusal; preceding changes remain applied.
    ///
    /// # Panics
    /// Panics if the cursor is not on the active area's last row.
    pub fn cursor_down_scroll(&mut self) -> Result<(), StorageError> {
        self.with_cursor_detached(LinkRestore::PageChange, Self::cursor_down_scroll_detached)
    }

    /// Scroll with cursor references detached.
    fn cursor_down_scroll_detached(&mut self) -> Result<(), StorageError> {
        assert_eq!(self.cursor.y, self.pages.rows() - 1);
        let Ok(old) = self.cursor_pin() else {
            return Ok(());
        };
        if self.no_scrollback {
            if self.pages.rows() == 1 {
                self.clear_cursor_row();
                self.cursor_mark_dirty();
            } else {
                self.erase_top_preserving_cursor(old)?;
            }
        } else {
            self.pages.grow();
            let Ok(current) = self.cursor_pin() else {
                return Ok(());
            };
            let next = if old.node == current.node {
                current.down(&self.pages, 1)
            } else {
                Some(crate::page_list::Pin {
                    x: self.cursor.x,
                    ..current
                })
            };
            if let Some(next) = next {
                self.cursor_change_pin(next);
            }
            self.cursor_mark_dirty();
            if self.cursor.style.bg_color != Color::None {
                self.clear_cursor_row();
            }
        }
        if self.cursor.style.bg_cell().is_some() {
            self.clear_cursor_row();
        }
        Ok(())
    }

    /// Shift active rows while retaining the cursor's original bottom position.
    fn erase_top_preserving_cursor(
        &mut self,
        old: crate::page_list::Pin,
    ) -> Result<(), StorageError> {
        self.pages.erase_row(Point::Active(Coordinate::default()))?;
        if let Some(pin) = self.pages.tracked_pin_mut(self.cursor.page_pin) {
            *pin = old;
        }
        Ok(())
    }

    /// Apply background-only blanks to the full stored cursor row.
    fn clear_cursor_row(&mut self) {
        if let Ok(pin) = self.cursor_pin()
            && let Some(cols) = self.pages.page(pin.node).map(|page| page.size.cols)
        {
            self.clear_cells(pin.node, pin.y, 0..cols);
        }
    }
}

impl Screen {
    /// Scroll rows at and above the cursor into history, inserting a blank below.
    ///
    /// # Errors
    /// Returns a managed boundary-copy refusal; preceding rotations remain applied.
    pub fn cursor_scroll_above(&mut self) -> Result<(), StorageError> {
        self.with_cursor_detached(LinkRestore::PageChange, Self::cursor_scroll_above_detached)
    }

    /// Insert above with cursor references detached.
    fn cursor_scroll_above_detached(&mut self) -> Result<(), StorageError> {
        self.cursor_mark_dirty();
        if self.cursor.y == self.pages.rows() - 1 {
            return self.cursor_down_scroll();
        }
        self.pages.grow();
        let Ok(current) = self.cursor_pin() else {
            return Ok(());
        };
        let Some(next) = current.down(&self.pages, 1) else {
            return Ok(());
        };
        self.cursor_change_pin(next);
        let Ok(pin) = self.cursor_pin() else {
            return Ok(());
        };
        self.pages.rotate_rows_down_from(pin)?;
        self.clear_cursor_row();
        Ok(())
    }

    /// Discard the top of a full-width region ending at the cursor.
    ///
    /// The cursor stays at the blank bottom row. `limit` counts rows above it.
    ///
    /// # Errors
    /// Returns a managed boundary-copy refusal; preceding changes remain applied.
    ///
    /// # Panics
    /// Panics when `limit` is zero or extends above the active area.
    pub fn cursor_scroll_region_up(&mut self, limit: usize) -> Result<(), StorageError> {
        self.with_cursor_detached(LinkRestore::PageChange, |screen| {
            screen.cursor_scroll_region_up_detached(limit)
        })
    }

    /// Rotate the region with cursor references detached.
    fn cursor_scroll_region_up_detached(&mut self, limit: usize) -> Result<(), StorageError> {
        assert!(limit >= 1 && limit <= usize::from(self.cursor.y));
        let Ok(old) = self.cursor_pin() else {
            return Ok(());
        };
        let y = usize::from(self.cursor.y) - limit;
        self.pages.erase_row_bounded(
            Point::Active(Coordinate {
                x: 0,
                y: u32::try_from(y).unwrap_or_default(),
            }),
            limit,
        )?;
        if let Some(pin) = self.pages.tracked_pin_mut(self.cursor.page_pin) {
            *pin = old;
        }
        self.clear_cursor_row();
        Ok(())
    }
}
