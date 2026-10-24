//! Cursor movement and migration of page-local managed references.

use super::{Cursor, Screen};
use crate::{
    page::{Cell, row::Row},
    page_list::{IncreaseCapacityError, Pin},
    point::Tag,
    style::Style,
};

/// Source-owned hyperlink identity policy while restoring page-local references.
#[derive(Clone, Copy)]
pub(super) enum LinkRestore {
    /// Capacity replacement retains the authored or generated identity.
    Preserve,
    /// Explicit cursor moves regenerate implicit identities across pages.
    PageChange,
    /// Resize regenerates an implicit identity even within the same page.
    Reidentify,
}

impl Screen {
    /// Borrow a cell to the right without moving the cursor.
    #[must_use]
    pub fn cursor_cell_right(&mut self, n: u16) -> Option<&mut Cell> {
        let pin = self.cursor_pin().ok()?.right(&self.pages, usize::from(n))?;
        Some(pin.row_and_cell_mut(&mut self.pages)?.1)
    }

    /// Borrow a cell to the left without moving the cursor.
    #[must_use]
    pub fn cursor_cell_left(&mut self, n: u16) -> Option<&mut Cell> {
        let pin = self.cursor_pin().ok()?;
        if n > pin.x {
            return None;
        }
        Some(
            pin.left(usize::from(n))
                .row_and_cell_mut(&mut self.pages)?
                .1,
        )
    }

    /// Borrow the final cell of the preceding row using that page's stored width.
    #[must_use]
    pub fn cursor_cell_end_of_prev(&mut self) -> Option<&mut Cell> {
        if self.cursor.y == 0 {
            return None;
        }
        let mut pin = self.cursor_pin().ok()?.up(&self.pages, 1)?;
        pin.x = self.pages.page(pin.node)?.size.cols - 1;
        Some(pin.row_and_cell_mut(&mut self.pages)?.1)
    }

    /// Borrow a preceding cursor row without moving the cursor.
    #[must_use]
    pub fn cursor_row_up(&mut self, n: u16) -> Option<&mut Row> {
        if n > self.cursor.y {
            return None;
        }
        let pin = self.cursor_pin().ok()?.up(&self.pages, usize::from(n))?;
        Some(pin.row_and_cell_mut(&mut self.pages)?.0)
    }

    /// Move right without wrapping; the caller supplies an in-row distance.
    ///
    /// # Panics
    /// Panics if the destination is outside the active width.
    pub fn cursor_right(&mut self, n: u16) {
        assert!(u32::from(self.cursor.x) + u32::from(n) < u32::from(self.pages.cols()));
        self.cursor_horizontal_absolute(self.cursor.x + n);
    }

    /// Move left without wrapping.
    ///
    /// # Panics
    /// Panics if the distance exceeds the cursor column.
    pub fn cursor_left(&mut self, n: u16) {
        assert!(n <= self.cursor.x);
        self.cursor_horizontal_absolute(self.cursor.x - n);
    }

    /// Move upward within the active area.
    ///
    /// # Panics
    /// Panics if the destination precedes the active area.
    pub fn cursor_up(&mut self, n: u16) {
        assert!(n <= self.cursor.y);
        self.cursor_absolute(self.cursor.x, self.cursor.y - n);
    }

    /// Move downward within the active area.
    ///
    /// # Panics
    /// Panics if the destination follows the active area.
    pub fn cursor_down(&mut self, n: u16) {
        assert!(u32::from(self.cursor.y) + u32::from(n) < u32::from(self.pages.rows()));
        self.cursor_absolute(self.cursor.x, self.cursor.y + n);
    }

    /// Move to an active column without changing rows or dirty state.
    ///
    /// # Panics
    /// Panics if the column is outside the active width.
    pub fn cursor_horizontal_absolute(&mut self, x: u16) {
        assert!(x < self.pages.cols());
        if let Some(pin) = self.pages.tracked_pin_mut(self.cursor.page_pin) {
            pin.x = x;
            self.cursor.x = x;
        }
    }

    /// Move to active coordinates, migrating style and hyperlink owners as needed.
    ///
    /// # Panics
    /// Panics if the destination is outside the active dimensions.
    pub fn cursor_absolute(&mut self, x: u16, y: u16) {
        assert!(x < self.pages.cols() && y < self.pages.rows());
        let Ok(old) = self.cursor_pin() else {
            return;
        };
        let pin = if y < self.cursor.y {
            old.up(&self.pages, usize::from(self.cursor.y - y))
        } else {
            old.down(&self.pages, usize::from(y - self.cursor.y))
        };
        if let Some(mut pin) = pin {
            pin.x = x;
            self.cursor.x = x;
            self.cursor.y = y;
            self.cursor_change_pin(pin);
        }
    }

    /// Recompute active coordinates after tracked storage moves, restoring style ownership.
    pub fn cursor_reload(&mut self) {
        let Ok(mut pin) = self.cursor_pin() else {
            return;
        };
        let coordinate = if let Some(point) = self.pages.point_from_pin(Tag::Active, pin) {
            point.coord()
        } else {
            pin = self.pages.get_top_left(Tag::Active);
            if let Some(current) = self.pages.tracked_pin_mut(self.cursor.page_pin) {
                *current = pin;
            }
            crate::point::Coordinate::default()
        };
        self.cursor.x = coordinate.x;
        self.cursor.y = u16::try_from(coordinate.y).unwrap_or(0);
        if self.cursor.style_id != 0 && self.manual_style_update().is_err() {
            self.cursor.style = Style::default();
            self.cursor.style_id = 0;
        }
    }

    /// Copy cursor state into this screen, optionally copying its owned hyperlink.
    ///
    /// # Errors
    /// Returns a capacity error if the incoming style cannot be admitted.
    ///
    /// # Panics
    /// Panics if the incoming position is outside this screen.
    pub fn cursor_copy(
        &mut self,
        other: &Cursor,
        hyperlink: bool,
    ) -> Result<(), IncreaseCapacityError> {
        assert!(other.x < self.pages.cols() && other.y < self.pages.rows());
        self.end_hyperlink();
        let cursor = Cursor {
            x: self.cursor.x,
            y: self.cursor.y,
            page_pin: self.cursor.page_pin,
            style_id: self.cursor.style_id,
            hyperlink: None,
            hyperlink_id: 0,
            cursor_style: other.cursor_style,
            pending_wrap: other.pending_wrap,
            protected: other.protected,
            style: other.style,
            hyperlink_implicit_id: other.hyperlink_implicit_id,
            semantic_content: other.semantic_content,
            semantic_content_clear_eol: other.semantic_content_clear_eol,
        };
        let old = std::mem::replace(&mut self.cursor, cursor);
        if let Err(error) = self.manual_style_update() {
            self.cursor = old;
            return Err(error);
        }
        self.cursor_absolute(other.x, other.y);
        if hyperlink && let Some(link) = &other.hyperlink {
            let id = match &link.id {
                crate::links::HyperlinkId::Explicit(id) => Some(id.as_slice()),
                crate::links::HyperlinkId::Implicit(_) => None,
            };
            let _ = self.start_hyperlink(&link.uri, id);
        }
        Ok(())
    }

    /// Move a tracked cursor while releasing and reinstalling page-local owners.
    pub(super) fn cursor_change_pin(&mut self, new: Pin) {
        let Ok(old) = self.cursor_pin() else {
            return;
        };
        if !old.eql(new) {
            self.cursor_mark_dirty();
            new.mark_dirty(&mut self.pages);
        }
        if old.node == new.node {
            if let Some(pin) = self.pages.tracked_pin_mut(self.cursor.page_pin) {
                *pin = new;
            }
            return;
        }
        self.with_cursor_detached(LinkRestore::PageChange, |screen| {
            if let Some(pin) = screen.pages.tracked_pin_mut(screen.cursor.page_pin) {
                *pin = new;
            }
        });
    }

    /// Release page-local cursor owners before storage can move or disappear.
    pub(super) fn detach_cursor(&mut self) -> (bool, Option<Box<crate::links::Hyperlink>>) {
        let styled = self.cursor.style_id != 0;
        if let Ok(pin) = self.cursor_pin()
            && let Some(page) = self.pages.page_mut(pin.node)
        {
            if styled {
                page.styles.release_multiple(self.cursor.style_id, 1);
            }
            if self.cursor.hyperlink_id != 0 {
                page.release_hyperlink(self.cursor.hyperlink_id);
            }
        }
        self.cursor.style_id = 0;
        self.cursor.hyperlink_id = 0;
        (styled, self.cursor.hyperlink.take())
    }

    /// Restore detached owners once, including after a partially applied edit fails.
    pub(super) fn with_cursor_detached<R>(
        &mut self,
        restore: LinkRestore,
        edit: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let old = self.cursor_pin().ok();
        let (styled, link) = self.detach_cursor();
        let result = edit(self);
        if styled && self.manual_style_update().is_err() {
            self.cursor.style = Style::default();
        }
        if let Some(link) = link {
            let regenerate = match restore {
                LinkRestore::Preserve => false,
                LinkRestore::Reidentify => true,
                LinkRestore::PageChange => old
                    .zip(self.cursor_pin().ok())
                    .is_some_and(|(a, b)| a.node != b.node),
            };
            if regenerate {
                let id = match &link.id {
                    crate::links::HyperlinkId::Explicit(id) => Some(id.as_slice()),
                    crate::links::HyperlinkId::Implicit(_) => None,
                };
                let _ = self.start_hyperlink(&link.uri, id);
            } else {
                let _ = self.admit_hyperlink(&link);
            }
        }
        result
    }

    /// Mark the current row dirty without setting the whole-page dirty flag.
    pub fn cursor_mark_dirty(&mut self) {
        if let Ok(pin) = self.cursor_pin() {
            pin.mark_dirty(&mut self.pages);
        }
    }
}
