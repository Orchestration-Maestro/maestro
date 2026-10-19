//! Borrowed cell contents and shared style colour resolution.

use crate::{
    Style,
    color::{Palette, Rgb},
    page::{Cell, CellPosition, Page, cell::Content},
    style::{BoldColor, Fg},
};

/// Cell data owned by a retained viewport row.
#[derive(Clone)]
pub(super) struct CellData {
    /// Packed source cell without borrowed managed memory.
    raw: Cell,
    /// Owned suffix scalars, excluding the first scalar in the packed cell.
    grapheme: Vec<char>,
    /// Resolved page-local style.
    style: Style,
}

impl CellData {
    /// Copy managed contents while the page is borrowed.
    pub fn new(page: &Page, pos: CellPosition, raw: Cell) -> Self {
        let style = match raw.content() {
            Content::BgColorPalette(index) => Style {
                bg_color: crate::style::Color::Palette(index),
                ..Style::default()
            },
            Content::BgColorRgb(rgb) => Style {
                bg_color: crate::style::Color::Rgb(rgb),
                ..Style::default()
            },
            _ if raw.style_id() != 0 => *page.styles.get(raw.style_id()),
            _ => Style::default(),
        };
        Self {
            raw,
            grapheme: page.lookup_grapheme(pos).unwrap_or_default().to_vec(),
            style,
        }
    }
}

/// A cell borrowing retained render data and its effective palette.
pub struct RenderCell<'a> {
    /// Copied cell contents.
    pub(super) data: &'a CellData,
    /// Snapshot palette.
    pub(super) palette: &'a Palette,
}

impl RenderCell<'_> {
    /// Packed cell data; use this view's methods for managed content.
    #[must_use]
    pub fn raw(&self) -> Cell {
        self.data.raw
    }

    /// Fully resolved style, default for an unstyled cell.
    #[must_use]
    pub fn style(&self) -> Style {
        self.data.style
    }

    /// Whether the packed cell carries styling.
    #[must_use]
    pub fn has_styling(&self) -> bool {
        self.data.raw.has_styling()
    }

    /// Resolve the explicit background, preferring background-only cell contents.
    #[must_use]
    pub fn background(&self) -> Option<Rgb> {
        self.data.style.bg(&self.data.raw, self.palette)
    }

    /// Resolve an explicit foreground, absent when the default is inherited.
    #[must_use]
    pub fn foreground(&self) -> Option<Rgb> {
        (self.data.style.fg_color != crate::style::Color::None).then(|| {
            self.data.style.fg(Fg {
                default: Rgb::default(),
                palette: self.palette,
                bold: None,
            })
        })
    }

    /// Resolve the foreground using the caller's default and bold treatment.
    #[must_use]
    pub fn foreground_with(&self, default: Rgb, bold: Option<BoldColor>) -> Rgb {
        self.data.style.fg(Fg {
            default,
            palette: self.palette,
            bold,
        })
    }

    /// Resolve an independently specified underline colour.
    #[must_use]
    pub fn underline_color(&self) -> Option<Rgb> {
        self.data.style.underline_color(self.palette)
    }

    /// Borrow suffix scalars after the packed cell's first scalar.
    #[must_use]
    pub fn grapheme(&self) -> &[char] {
        &self.data.grapheme
    }

    /// Iterate all text scalars, omitting empty cells and wide-character spacers.
    pub fn graphemes(&self) -> impl Iterator<Item = char> + '_ {
        self.data
            .raw
            .has_text()
            .then(|| self.data.raw.codepoint())
            .into_iter()
            .chain(self.data.grapheme.iter().copied())
    }

    /// Encode all cell text, without foreign buffer sizing or filling.
    #[must_use]
    pub fn graphemes_utf8(&self) -> String {
        self.graphemes().collect()
    }
}
