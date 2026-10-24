//! Incremental viewport snapshots consuming terminal dirty flags.

mod cell;
mod row;
#[cfg(test)]
mod tests;
pub use cell::RenderCell;
pub use row::RenderRow;

use crate::{
    Mode, Terminal,
    color::{Palette, Rgb},
    page_list::Pin,
    point::{Coordinate, Tag},
    screens::Key,
};
use row::RowData;

/// Renderer invalidation retained until the caller clears it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Dirty {
    /// No newly invalidated content.
    #[default]
    Clean,
    /// Some rows changed, without a global change.
    Partial,
    /// Global state or viewport geometry changed.
    Full,
}

/// Effective defaults, with reverse video already applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Colors {
    /// Default background.
    pub background: Rgb,
    /// Default foreground.
    pub foreground: Rgb,
    /// Explicit cursor colour, if configured.
    pub cursor: Option<Rgb>,
    /// Effective indexed palette.
    pub palette: Palette,
}

impl Default for Colors {
    fn default() -> Self {
        Self {
            background: Rgb::default(),
            foreground: Rgb {
                r: 255,
                g: 255,
                b: 255,
            },
            cursor: None,
            palette: crate::color::DEFAULT,
        }
    }
}

/// Cursor location relative to the visible rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorViewport {
    /// Column in the viewport.
    pub x: u16,
    /// Row in the viewport.
    pub y: u32,
    /// The preceding cell is the head of a wide character.
    pub wide_tail: bool,
}

/// Cursor state independent of row invalidation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cursor {
    /// Position in the editable area.
    pub active: Coordinate,
    /// Position in the viewport, absent when scrolled out of view.
    pub viewport: Option<CursorViewport>,
    /// Packed cell under the cursor.
    pub cell: crate::page::Cell,
    /// Current writing style.
    pub style: crate::Style,
    /// Cursor shape.
    pub visual_style: crate::cursor::Style,
    /// Visibility mode.
    pub visible: bool,
    /// Blink mode.
    pub blinking: bool,
}

/// Retained viewport contents with independently cleared row invalidation.
///
/// Updating requires exclusive terminal access because this is the sole dirty
/// consumer. Clear `dirty` and each handled row separately after rendering.
/// Rows and cells borrow this state; no terminal borrow survives an update.
pub struct RenderState {
    /// Viewport height.
    pub rows: u16,
    /// Viewport width.
    pub cols: u16,
    /// Effective terminal colours.
    pub colors: Colors,
    /// Current cursor state.
    pub cursor: Cursor,
    /// Overall renderer invalidation; row clearing does not change it.
    pub dirty: Dirty,
    /// Last selected screen.
    screen: Key,
    /// Last visible origin, compared without dereferencing expired handles.
    viewport_pin: Option<Pin>,
    /// Owned row contents, reused across updates.
    row_data: Vec<RowData>,
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            rows: 0,
            cols: 0,
            colors: Colors::default(),
            cursor: Cursor {
                visible: true,
                ..Cursor::default()
            },
            dirty: Dirty::Clean,
            screen: Key::Primary,
            viewport_pin: None,
            row_data: Vec::new(),
        }
    }
}

impl RenderState {
    /// Refresh the viewport and consume terminal, page and visible-row dirtiness.
    ///
    /// Unhandled snapshot dirty flags remain set across updates. A reverse-video
    /// transition or changed effective colour invalidates the whole viewport even
    /// without a terminal dirty bit.
    pub fn update(&mut self, terminal: &mut Terminal) {
        let screen = terminal.screens.active();
        let origin = screen.pages.get_top_left(Tag::Viewport);
        let redraw = self.redraw(terminal, origin);
        self.rows = screen.pages.rows();
        self.cols = screen.pages.cols();
        self.viewport_pin = Some(origin);
        self.update_cursor(terminal);
        let redraw = self.update_colors(terminal, redraw) || redraw;
        self.row_data
            .resize_with(usize::from(self.rows), RowData::default);
        let changed = self.update_rows(terminal, origin, redraw);
        if redraw {
            self.screen = terminal.screens.active_key();
            self.dirty = Dirty::Full;
        } else if changed && self.dirty == Dirty::Clean {
            self.dirty = Dirty::Partial;
        }
        terminal.flags.dirty = crate::terminal::Dirty::default();
    }

    /// Borrow viewport rows in order, allowing each handled row to be cleared.
    pub fn rows(&mut self) -> impl ExactSizeIterator<Item = RenderRow<'_>> {
        let palette = &self.colors.palette;
        self.row_data
            .iter_mut()
            .map(move |data| RenderRow { data, palette })
    }

    /// Whether any global change requires rebuilding every visible row.
    fn redraw(&self, terminal: &Terminal, origin: Pin) -> bool {
        let pages = &terminal.screens.active().pages;
        self.screen != terminal.screens.active_key()
            || terminal.flags.dirty != crate::terminal::Dirty::default()
            || self.rows != pages.rows()
            || self.cols != pages.cols()
            || self.viewport_pin.is_some_and(|old| !old.eql(origin))
    }

    /// Capture cursor properties even when no row is dirty.
    fn update_cursor(&mut self, terminal: &Terminal) {
        let screen = terminal.screens.active();
        let pin = screen.pages.tracked_pin(screen.cursor.page_pin).copied();
        self.cursor.active = Coordinate {
            x: screen.cursor.x,
            y: u32::from(screen.cursor.y),
        };
        self.cursor.cell = pin
            .and_then(|p| p.row_and_cell(&screen.pages))
            .map_or_else(crate::page::Cell::default, |(_, cell)| *cell);
        self.cursor.style = screen.cursor.style;
        self.cursor.visual_style = screen.cursor.cursor_style;
        self.cursor.visible = terminal.modes.get(Mode::CursorVisible);
        self.cursor.blinking = terminal.modes.get(Mode::CursorBlinking);
        self.cursor.viewport = None;
    }

    /// Refresh cheap defaults, copying the palette only when globally invalidated.
    fn update_colors(&mut self, terminal: &Terminal, redraw: bool) -> bool {
        let previous = (
            self.colors.background,
            self.colors.foreground,
            self.colors.cursor,
        );
        self.colors.cursor = terminal.colors.cursor.get();
        if redraw {
            self.colors.palette = terminal.colors.palette.current;
        }
        if let (Some(bg), Some(fg)) = (
            terminal.colors.background.get(),
            terminal.colors.foreground.get(),
        ) {
            (self.colors.background, self.colors.foreground) =
                if terminal.modes.get(Mode::ReverseColors) {
                    (fg, bg)
                } else {
                    (bg, fg)
                };
        }
        previous
            != (
                self.colors.background,
                self.colors.foreground,
                self.colors.cursor,
            )
    }

    /// Rebuild dirty rows in contiguous page chunks, consuming each page flag once.
    fn update_rows(&mut self, terminal: &mut Terminal, origin: Pin, redraw: bool) -> bool {
        let screen = terminal.screens.active_mut();
        let cursor_pin = screen.pages.tracked_pin(screen.cursor.page_pin).copied();
        let pages = &mut screen.pages;
        let mut chunks = origin.page_iterator(crate::page_list::Direction::RightDown, None);
        let mut y = 0;
        let mut changed = false;
        while y < self.row_data.len() {
            let Some(chunk) = chunks.next(pages) else {
                break;
            };
            let Some(page) = pages.page_mut(chunk.node) else {
                break;
            };
            let page_dirty = page.dirty;
            page.dirty = false;
            let take = usize::from(chunk.end - chunk.start).min(self.row_data.len() - y);
            if let Some(pin) = cursor_pin.filter(|pin| {
                pin.node == chunk.node
                    && pin.y >= chunk.start
                    && usize::from(pin.y - chunk.start) < take
            }) {
                self.cursor.viewport = Some(CursorViewport {
                    x: pin.x,
                    y: u32::try_from(y).unwrap_or_default() + u32::from(pin.y - chunk.start),
                    wide_tail: pin.x > 0
                        && page.get_cells(pin.y)[usize::from(pin.x - 1)].wide()
                            == crate::page::cell::Wide::Wide,
                });
            }
            for (offset, py) in (chunk.start..chunk.end).take(take).enumerate() {
                let data = &mut self.row_data[y + offset];
                changed |= data.update(page, py, redraw || page_dirty);
            }
            y += take;
        }
        changed
    }
}
