//! Screen-owned terminal storage, cursor and protocol state.

mod attributes;
mod capacity;
mod clearing;
mod cursor;
mod grapheme;
mod hyperlink;
mod prompt;
mod resize;
pub use prompt::{InputTermination, SemanticContent};
mod scrolling;
pub use resize::Resize;

use std::num::NonZeroU16;

use crate::{
    PageList,
    ansi::ProtectedMode,
    charsets::{Charset, Slots},
    kitty::KeyFlagStack,
    links::Hyperlink,
    page::cell::SemanticContent as CellContent,
    page_list::TrackedPin,
    style::{self, Style},
};

pub use crate::cursor::Style as CursorStyle;
pub use crate::page_list::Scroll;

/// Page storage and protocol state for one terminal screen.
pub struct Screen {
    /// Screen-local image payloads, placements and transmission state.
    pub kitty_images: crate::kitty::graphics::images::ImageStorage,
    /// Retained content and tracked positions.
    pub pages: PageList,
    /// Disable history even when the active-area storage budget has spare room.
    pub no_scrollback: bool,
    /// Current writing position and attributes.
    pub cursor: Cursor,
    /// State saved by the terminal's cursor-save operation.
    pub saved_cursor: Option<SavedCursor>,
    /// Character-set designation and invocation state.
    pub charset: CharsetState,
    /// Most recently selected protection protocol.
    pub protected_mode: ProtectedMode,
    /// Per-screen enhanced keyboard protocol history.
    pub kitty_keyboard: KeyFlagStack,
    /// Whether shell prompt semantics have been observed.
    pub semantic_prompt: SemanticPrompt,
}

/// Semantic-content observation for a screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SemanticPrompt {
    /// Whether any prompt content has been observed.
    pub seen: bool,
}

/// Writing position and attributes, with a list-owned retained cell position.
#[derive(Clone)]
pub struct Cursor {
    /// Column in the active area.
    pub x: u16,
    /// Row in the active area.
    pub y: u16,
    /// Visual cursor shape.
    pub cursor_style: CursorStyle,
    /// Whether the next character must soft-wrap.
    pub pending_wrap: bool,
    /// Whether newly written cells are protected.
    pub protected: bool,
    /// Concrete active text style.
    pub style: Style,
    /// Active style identity in the cursor's page.
    pub style_id: style::Id,
    /// Active hyperlink identity in the cursor's page.
    pub hyperlink_id: crate::links::Id,
    /// Wrapping counter for hyperlinks without explicit identities.
    pub hyperlink_implicit_id: u32,
    /// Owned identity retained across page replacement and cursor copying.
    pub hyperlink: Option<Box<Hyperlink>>,
    /// Semantic role applied to newly written cells.
    pub semantic_content: CellContent,
    /// Whether semantic content extends through the end of the line.
    pub semantic_content_clear_eol: bool,
    /// Retained cell location resolved through the owning page list.
    pub page_pin: TrackedPin,
}

impl Cursor {
    /// Construct default writing state around an existing retained position.
    fn new(page_pin: TrackedPin) -> Self {
        Self {
            x: 0,
            y: 0,
            cursor_style: CursorStyle::Block,
            pending_wrap: false,
            protected: false,
            style: Style::default(),
            style_id: style::DEFAULT_ID,
            hyperlink_id: 0,
            hyperlink_implicit_id: 0,
            hyperlink: None,
            semantic_content: CellContent::Output,
            semantic_content_clear_eol: false,
            page_pin,
        }
    }
}

/// Cursor state retained by terminal save/restore operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavedCursor {
    /// Saved active column.
    pub x: u16,
    /// Saved active row.
    pub y: u16,
    /// Saved text style.
    pub style: Style,
    /// Saved character-protection state.
    pub protected: bool,
    /// Saved pending soft-wrap state.
    pub pending_wrap: bool,
    /// Saved origin mode.
    pub origin: bool,
    /// Saved character-set state.
    pub charset: CharsetState,
}

/// Character-set designations and independent invocation slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharsetState {
    /// Mapping from each designation slot to a character set.
    pub charsets: CharsetArray,
    /// Slot used for seven-bit printable input.
    pub gl: Slots,
    /// Slot used for eight-bit printable input.
    pub gr: Slots,
    /// Temporary slot used for exactly one character.
    pub single_shift: Option<Slots>,
}

impl Default for CharsetState {
    fn default() -> Self {
        Self {
            charsets: CharsetArray([Charset::Utf8; 4]),
            gl: Slots::G0,
            gr: Slots::G2,
            single_shift: None,
        }
    }
}

/// Four character-set designations indexed by protocol slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharsetArray([Charset; 4]);

impl CharsetArray {
    /// Read the designation for a slot.
    #[must_use]
    pub fn get(&self, slot: Slots) -> Charset {
        self.0[Self::index(slot)]
    }

    /// Replace the designation for one slot.
    pub fn set(&mut self, slot: Slots, charset: Charset) {
        self.0[Self::index(slot)] = charset;
    }

    /// Map the protocol slot to its array position.
    fn index(slot: Slots) -> usize {
        match slot {
            Slots::G0 => 0,
            Slots::G1 => 1,
            Slots::G2 => 2,
            Slots::G3 => 3,
        }
    }
}

/// Initial active dimensions and history budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    /// Positive active column count.
    pub cols: NonZeroU16,
    /// Positive active row count.
    pub rows: NonZeroU16,
    /// History byte budget; zero disables history.
    pub max_scrollback: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            cols: NonZeroU16::new(80).unwrap_or(NonZeroU16::MIN),
            rows: NonZeroU16::new(24).unwrap_or(NonZeroU16::MIN),
            max_scrollback: 0,
        }
    }
}

impl Screen {
    /// Create blank storage with a tracked cursor at the origin.
    #[must_use]
    pub fn init(opts: Options) -> Self {
        let mut pages = PageList::init(opts.cols, opts.rows, Some(opts.max_scrollback));
        let page_pin = pages.track_origin();
        let mut kitty_images = crate::kitty::graphics::images::ImageStorage::default();
        kitty_images.total_limit = 0;
        Self {
            kitty_images,
            pages,
            no_scrollback: opts.max_scrollback == 0,
            cursor: Cursor::new(page_pin),
            saved_cursor: None,
            charset: CharsetState::default(),
            protected_mode: ProtectedMode::Off,
            kitty_keyboard: KeyFlagStack::default(),
            semantic_prompt: SemanticPrompt::default(),
        }
    }

    /// Clear content and protocol state, preserving dimensions and cursor handle.
    pub fn reset(&mut self) {
        let limit = self.kitty_images.total_limit;
        self.kitty_images.clear(&mut self.pages);
        self.kitty_images.total_limit = limit;
        self.detach_cursor();
        self.pages.reset();
        self.cursor = Cursor::new(self.cursor.page_pin);
        self.saved_cursor = None;
        self.charset = CharsetState::default();
        self.kitty_keyboard = KeyFlagStack::default();
        self.protected_mode = ProtectedMode::Off;
        self.semantic_prompt = SemanticPrompt::default();
    }
}

#[cfg(test)]
mod tests;
