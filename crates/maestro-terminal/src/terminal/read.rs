//! Text and cell reads over the selected screen's existing storage.

use super::Terminal;
use crate::{
    Coordinate, Pin, Point,
    formatter::{Content, Format, Options, ScreenExtra, TerminalExtra, TerminalFormatter},
    page::{CellPosition, cell::Wide},
    selection::Selection,
};

/// Geometry of a text range before its coordinates are resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextShape {
    /// Follow text from the first endpoint through the last.
    Linear,
    /// Include the same column interval on every selected row.
    Rectangle,
}

/// Inclusive coordinates relative to a read method's screen or viewport origin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextRange {
    /// First endpoint, in zero-based column and row coordinates.
    pub start: (u16, u32),
    /// Last endpoint; endpoints may be supplied in either order.
    pub end: (u16, u32),
    /// Linear text or a rectangular column interval.
    pub shape: TextShape,
}

/// A physical screen cell's width and Unicode scalars.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreenTextCell {
    /// Position within a single-width or wide character.
    pub wide: Wide,
    /// First scalar and combining suffix; empty for blank cells and spacers.
    pub graphemes: Vec<char>,
}

/// A physical retained row, including soft-wrap metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreenTextRow {
    /// Addressable cells across the terminal's current width.
    pub cells: Vec<ScreenTextCell>,
    /// Whether text continues onto the next physical row.
    pub soft_wrapped: bool,
    /// Whether this row continues the preceding physical row.
    pub wrap_continuation: bool,
}

impl Terminal {
    /// Replace the viewport row anchor, returning the old anchor's retained row.
    ///
    /// The first call or a removed anchor returns `None`. An invalid new row
    /// clears tracking but still returns the previous anchor when it survives.
    pub fn track_row(&mut self, y: u32) -> Option<usize> {
        let previous = self
            .tracked_row
            .take()
            .and_then(|(key, generation, tracked)| {
                if self.screens.generation(key) != generation {
                    return None;
                }
                let list = &mut self.screens.get_mut(key)?.pages;
                let row = list
                    .tracked_pin(tracked)
                    .filter(|pin| !pin.garbage)
                    .and_then(|pin| list.point_from_pin(crate::point::Tag::Screen, *pin))
                    .and_then(|point| usize::try_from(point.coord().y).ok());
                list.untrack_pin(tracked);
                row
            });
        let key = self.screens.active_key();
        let generation = self.screens.generation(key);
        let list = &mut self.screens.active_mut().pages;
        self.tracked_row = list
            .pin(Point::Viewport(Coordinate { x: 0, y }))
            .and_then(|pin| list.track_pin(pin))
            .map(|tracked| (key, generation, tracked));
        previous
    }

    /// Read a retained cell, or `None` for an out-of-range coordinate.
    #[must_use]
    pub fn screen_cell(&self, x: u16, y: u32) -> Option<(Wide, Vec<char>)> {
        let pin = self
            .screens
            .active()
            .pages
            .pin(Point::Screen(Coordinate { x, y }))?;
        let cell = self.text_cell(pin)?;
        Some((cell.wide, cell.graphemes))
    }

    /// Copy all physical rows, retaining wrap flags and Unicode cell boundaries.
    ///
    /// Returns `None` if a retained row or cell cannot be resolved.
    #[must_use]
    pub fn screen_text_rows(&self) -> Option<Vec<ScreenTextRow>> {
        self.screen_text_rows_range(0, usize::MAX)
    }

    /// Copy a clamped, half-open interval of retained physical rows.
    ///
    /// Returns `None` if a retained row or cell cannot be resolved.
    #[must_use]
    pub fn screen_text_rows_range(
        &self,
        start_row: usize,
        end_row_exclusive: usize,
    ) -> Option<Vec<ScreenTextRow>> {
        let list = &self.screens.active().pages;
        let start = start_row.min(list.total_rows());
        let end = end_row_exclusive.min(list.total_rows()).max(start);
        (start..end)
            .map(|y| {
                let pin = list.pin(Point::Screen(Coordinate {
                    x: 0,
                    y: u32::try_from(y).ok()?,
                }))?;
                let page = list.page(pin.node)?;
                let row = page.get_row(pin.y);
                let cells = (0..self.cols())
                    .map(|x| self.text_cell(Pin { x, ..pin }))
                    .collect::<Option<Vec<_>>>()?;
                Some(ScreenTextRow {
                    cells,
                    soft_wrapped: row.wrap(),
                    wrap_continuation: row.wrap_continuation(),
                })
            })
            .collect()
    }

    /// Read a viewport cell's hyperlink; an invalid coordinate returns `None`.
    ///
    /// Valid unlinked cells return `Some(None)`. Invalid UTF-8 is replaced.
    #[must_use]
    pub fn viewport_hyperlink_uri(&self, x: u16, y: u32) -> Option<Option<String>> {
        let list = &self.screens.active().pages;
        let pin = list.pin(Point::Viewport(Coordinate { x, y }))?;
        Some(
            pin.hyperlink_uri(list)
                .map(|uri| String::from_utf8_lossy(uri).into_owned()),
        )
    }

    /// Read retained screen text, joining soft wraps and trimming trailing blanks.
    ///
    /// Returns `None` if either inclusive endpoint is outside the retained screen.
    #[must_use]
    pub fn read_text_screen(&self, range: TextRange) -> Option<String> {
        self.read_formatted_selection(
            range,
            Point::Screen,
            Options {
                unwrap: true,
                ..Options::default()
            },
        )
    }

    /// Read styled viewport text, preserving soft wraps as physical line breaks.
    ///
    /// Trailing empty cells are omitted; authored spaces remain.
    /// Invalid viewport endpoints return `None`.
    #[must_use]
    pub fn read_ansi_viewport(&self, range: TextRange) -> Option<String> {
        self.read_formatted_selection(
            range,
            Point::Viewport,
            Options {
                emit: Format::Vt,
                ..Options::default()
            },
        )
    }

    /// Read styled retained screen text with optional soft-wrap joining.
    ///
    /// Trailing empty cells are omitted; authored spaces remain.
    /// Invalid retained endpoints return `None`.
    #[must_use]
    pub fn read_ansi_screen(&self, range: TextRange, unwrap: bool) -> Option<String> {
        self.read_formatted_selection(
            range,
            Point::Screen,
            Options {
                emit: Format::Vt,
                unwrap,
                ..Options::default()
            },
        )
    }

    /// Emit retained keyboard protocol state as replayable text, without screen content.
    ///
    /// # Errors
    /// Returns a formatting error if the formatter cannot write its output.
    pub fn kitty_keyboard_state_ansi(&self) -> Result<String, std::fmt::Error> {
        let mut formatter = TerminalFormatter::init(
            self,
            Options {
                emit: Format::Vt,
                unwrap: false,
                trim: false,
                ..Options::default()
            },
        );
        formatter.content = Content::None;
        formatter.extra = TerminalExtra {
            keyboard: true,
            screen: ScreenExtra {
                kitty_keyboard: true,
            },
        };
        let mut output = String::new();
        formatter.format(&mut output)?;
        Ok(output)
    }

    /// Resolve a coordinate range once and delegate all text emission to its owner.
    fn read_formatted_selection(
        &self,
        range: TextRange,
        point: fn(Coordinate) -> Point,
        opts: Options<'_>,
    ) -> Option<String> {
        let list = &self.screens.active().pages;
        let selection = Selection::init(
            list.pin(point(Coordinate {
                x: range.start.0,
                y: range.start.1,
            }))?,
            list.pin(point(Coordinate {
                x: range.end.0,
                y: range.end.1,
            }))?,
            range.shape == TextShape::Rectangle,
        );
        let mut formatter = TerminalFormatter::init(self, opts);
        formatter.content = Content::Selection(Some(selection));
        let mut output = String::new();
        formatter.format(&mut output).ok()?;
        Some(output)
    }

    /// Copy text from a checked cell without copying styles or page storage.
    fn text_cell(&self, pin: Pin) -> Option<ScreenTextCell> {
        let list = &self.screens.active().pages;
        let (_, cell) = pin.row_and_cell(list)?;
        let mut graphemes = Vec::new();
        if cell.has_text() {
            graphemes.push(cell.codepoint());
            if cell.has_grapheme() {
                graphemes.extend_from_slice(
                    pin.grapheme(list, CellPosition { x: pin.x, y: pin.y })
                        .unwrap_or_default(),
                );
            }
        }
        Some(ScreenTextCell {
            wide: cell.wide(),
            graphemes,
        })
    }
}
