//! Text formatting over terminal-owned storage.

mod plain;
mod vt;

pub use vt::{ScreenExtra, TerminalExtra};

use crate::Page;

/// Replacement for a matching Unicode scalar.
#[derive(Clone, Copy, Debug)]
pub enum Replacement<'a> {
    /// Emit one scalar.
    Codepoint(char),
    /// Emit a UTF-8 string.
    String(&'a str),
}

/// An inclusive Unicode range and its replacement.
#[derive(Clone, Debug)]
pub struct CodepointMap<'a> {
    /// Inclusive scalar interval.
    pub range: std::ops::RangeInclusive<char>,
    /// Text emitted instead of each matching scalar.
    pub replacement: Replacement<'a>,
}

/// Output encoding for retained terminal content.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Format {
    /// Unicode text without control sequences.
    #[default]
    Plain,
    /// Replayable text with SGR styling and carriage-return line feeds.
    Vt,
}

/// Common text encoding options.
#[derive(Clone, Copy, Debug)]
pub struct Options<'a> {
    /// Output encoding.
    pub emit: Format,
    /// Default background emitted before styled content.
    pub background: Option<crate::color::Rgb>,
    /// Default foreground emitted before styled content.
    pub foreground: Option<crate::color::Rgb>,
    /// Resolve indexed style colours when supplied.
    pub palette: Option<&'a crate::color::Palette>,
    /// Join soft-wrapped rows instead of emitting physical line breaks.
    pub unwrap: bool,
    /// Omit trailing space characters on nonempty rows.
    pub trim: bool,
    /// Ordered Unicode substitutions; the last matching range wins.
    pub codepoint_map: &'a [CodepointMap<'a>],
}

impl Default for Options<'_> {
    fn default() -> Self {
        Self {
            emit: Format::Plain,
            background: None,
            foreground: None,
            palette: None,
            unwrap: false,
            trim: true,
            codepoint_map: &[],
        }
    }
}

/// Deferred blanks carried between consecutive pages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TrailingState {
    /// Pending line breaks, omitted if no later text appears.
    pub rows: usize,
    /// Pending spaces, retained only across a soft-wrap continuation.
    pub cells: usize,
}

/// Format an inclusive range within one page.
///
/// A start on a wide character's tail includes its leading cell.
/// Trailing blank rows are always omitted.
pub struct PageFormatter<'a> {
    /// Borrowed page containing cells and graphemes.
    pub page: &'a Page,
    /// Text encoding options.
    pub opts: Options<'a>,
    /// Starting column on the first row, or every rectangle row.
    pub start_x: u16,
    /// Inclusive starting page-local row.
    pub start_y: u16,
    /// Final column on the last row, or every rectangle row.
    pub end_x: Option<u16>,
    /// Inclusive final page-local row; absent means the page bottom.
    pub end_y: Option<u16>,
    /// Apply the column interval to every included row.
    pub rectangle: bool,
    /// Deferred blanks from the preceding page.
    pub trailing_state: Option<TrailingState>,
}

impl<'a> PageFormatter<'a> {
    /// Format the whole page with supplied encoding options.
    #[must_use]
    pub fn init(page: &'a Page, opts: Options<'a>) -> Self {
        Self {
            page,
            opts,
            start_x: 0,
            start_y: 0,
            end_x: None,
            end_y: None,
            rectangle: false,
            trailing_state: None,
        }
    }

    /// Write text, discarding deferred trailing blanks.
    ///
    /// # Errors
    /// Returns the destination's formatting error.
    pub fn format(&self, writer: &mut impl std::fmt::Write) -> std::fmt::Result {
        self.format_with_state(writer).map(|_| ())
    }
}

/// Screen content included in formatted output.
#[derive(Clone, Copy, Debug)]
pub enum Content {
    /// Emit no cells.
    None,
    /// Emit the whole screen, or an inclusive selected range.
    Selection(Option<crate::selection::Selection>),
}

/// Format the active screen of a terminal.
pub struct TerminalFormatter<'a> {
    /// Optional keyboard state emitted after content.
    pub extra: TerminalExtra,
    /// Terminal supplying its active screen.
    pub terminal: &'a crate::Terminal,
    /// Text encoding options.
    pub opts: Options<'a>,
    /// Included screen range.
    pub content: Content,
}

impl<'a> TerminalFormatter<'a> {
    /// Include all retained content from the active screen.
    #[must_use]
    pub fn init(terminal: &'a crate::Terminal, opts: Options<'a>) -> Self {
        Self {
            extra: TerminalExtra::default(),
            terminal,
            opts,
            content: Content::Selection(None),
        }
    }

    /// Write the selected content to the destination.
    ///
    /// # Errors
    /// Returns the destination's formatting error.
    pub fn format(&self, writer: &mut impl std::fmt::Write) -> std::fmt::Result {
        ScreenFormatter {
            screen: self.terminal.screens.active(),
            opts: self.opts,
            content: self.content,
            extra: self.extra.screen,
        }
        .format(writer)?;
        self.format_keyboard(writer)
    }
}

/// Format a screen's retained content without copying its cells.
pub struct ScreenFormatter<'a> {
    /// Optional keyboard protocol state emitted after content.
    pub extra: ScreenExtra,
    /// Borrowed screen.
    pub screen: &'a crate::Screen,
    /// Text encoding options.
    pub opts: Options<'a>,
    /// Included screen range.
    pub content: Content,
}

impl<'a> ScreenFormatter<'a> {
    /// Include the whole retained screen.
    #[must_use]
    pub fn init(screen: &'a crate::Screen, opts: Options<'a>) -> Self {
        Self {
            extra: ScreenExtra::default(),
            screen,
            opts,
            content: Content::Selection(None),
        }
    }

    /// Write the selected content; expired selections emit no cells.
    ///
    /// # Errors
    /// Returns the destination's formatting error.
    pub fn format(&self, writer: &mut impl std::fmt::Write) -> std::fmt::Result {
        if let Content::Selection(selection) = self.content {
            self.format_content(writer, selection)?;
        }
        self.format_keyboard(writer)
    }

    /// Write the selected cells without optional screen state.
    fn format_content(
        &self,
        writer: &mut impl std::fmt::Write,
        selection: Option<crate::selection::Selection>,
    ) -> std::fmt::Result {
        let mut formatter = PageListFormatter::init(&self.screen.pages, self.opts);
        if let Some(selection) = selection {
            let (Some(tl), Some(br)) = (
                selection.top_left(self.screen),
                selection.bottom_right(self.screen),
            ) else {
                return Ok(());
            };
            formatter.top_left = Some(tl);
            formatter.bottom_right = Some(br);
            formatter.rectangle = selection.rectangle;
        }
        formatter.format(writer)
    }
}

/// Format consecutive page chunks, carrying deferred blanks between pages.
pub struct PageListFormatter<'a> {
    /// Borrowed list owning the selected pins.
    pub list: &'a crate::PageList,
    /// Text encoding options.
    pub opts: Options<'a>,
    /// Inclusive starting pin; absent means the oldest retained cell.
    pub top_left: Option<crate::Pin>,
    /// Inclusive ending pin; absent means the last retained cell.
    pub bottom_right: Option<crate::Pin>,
    /// Apply the column bounds to every row.
    pub rectangle: bool,
}

impl<'a> PageListFormatter<'a> {
    /// Include all retained pages.
    #[must_use]
    pub fn init(list: &'a crate::PageList, opts: Options<'a>) -> Self {
        Self {
            list,
            opts,
            top_left: None,
            bottom_right: None,
            rectangle: false,
        }
    }

    /// Write ordered inclusive bounds, or nothing for expired or inverted pins.
    ///
    /// # Errors
    /// Returns the destination's formatting error.
    pub fn format(&self, writer: &mut impl std::fmt::Write) -> std::fmt::Result {
        use crate::{page_list::Direction, point::Tag};
        let tl = self
            .top_left
            .unwrap_or_else(|| self.list.get_top_left(Tag::Screen));
        let Some(br) = self
            .bottom_right
            .or_else(|| self.list.get_bottom_right(Tag::Screen))
        else {
            return Ok(());
        };
        let (Some(start), Some(end)) = (
            self.list.point_from_pin(Tag::Screen, tl),
            self.list.point_from_pin(Tag::Screen, br),
        ) else {
            return Ok(());
        };
        if (start.coord().y, start.coord().x) > (end.coord().y, end.coord().x) {
            return Ok(());
        }
        let mut iter = tl.page_iterator(Direction::RightDown, Some(br));
        let mut state = None;
        while let Some(chunk) = iter.next(self.list) {
            let Some(page) = self.list.page(chunk.node) else {
                break;
            };
            let mut formatter = PageFormatter::init(page, self.opts);
            formatter.start_y = chunk.start;
            formatter.end_y = Some(chunk.end - 1);
            formatter.rectangle = self.rectangle;
            formatter.trailing_state = state;
            if self.rectangle || chunk.node == tl.node {
                formatter.start_x = tl.x;
            }
            if self.rectangle || chunk.node == br.node {
                formatter.end_x = Some(br.x);
            }
            state = Some(formatter.format_with_state(writer)?);
        }
        Ok(())
    }
}
