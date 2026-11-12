//! Cluster suffix admission on the cursor row across page replacement.

use super::Screen;
use crate::{
    page::CellPosition,
    page_list::{IncreaseCapacity, IncreaseCapacityError as Error},
};

impl Screen {
    /// Append to a textual cell on the cursor row, growing suffix storage once if needed.
    ///
    /// Cursor style and hyperlink references are retained across page replacement.
    ///
    /// # Errors
    /// Returns a capacity error if growth or the retried append cannot fit.
    ///
    /// # Panics
    /// Panics if the column is outside the active width.
    pub fn append_grapheme(&mut self, x: u16, cp: char) -> Result<(), Error> {
        assert!(x < self.pages.cols());
        let pin = self.cursor_pin()?;
        let position = CellPosition { x, y: pin.y };
        let page = self.pages.page_mut(pin.node).ok_or(Error::InvalidNode)?;
        if page.append_grapheme(position, cp).is_ok() {
            return Ok(());
        }
        self.increase_capacity(pin.node, Some(IncreaseCapacity::GraphemeBytes))?;
        let pin = self.cursor_pin()?;
        self.pages
            .page_mut(pin.node)
            .ok_or(Error::InvalidNode)?
            .append_grapheme(CellPosition { x, y: pin.y }, cp)
            .map_err(|_| Error::OutOfSpace)
    }
}

#[cfg(test)]
mod tests;
