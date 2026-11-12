//! Packed terminal cell contents and attributes.

use crate::color::Rgb;

/// One terminal grid cell.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cell(u64);

/// Text or a background-only cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Content {
    /// A single Unicode scalar.
    Codepoint(char),
    /// The first scalar of a page-owned cluster.
    CodepointGrapheme(char),
    /// An indexed background colour.
    BgColorPalette(u8),
    /// A direct background colour.
    BgColorRgb(Rgb),
}

/// The active content representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ContentTag {
    /// A single scalar.
    Codepoint = 0,
    /// A scalar with a cluster suffix.
    CodepointGrapheme = 1,
    /// An indexed background.
    BgColorPalette = 2,
    /// A direct background.
    BgColorRgb = 3,
}

/// Wide-character position within the grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Wide {
    /// An ordinary single-width cell.
    Narrow = 0,
    /// The leading cell of a wide character.
    Wide = 1,
    /// The trailing spacer of a wide character.
    SpacerTail = 2,
    /// A spacer before a wrapped wide character.
    SpacerHead = 3,
}

/// Shell-defined semantic role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SemanticContent {
    /// Program output.
    Output = 0,
    /// User input.
    Input = 1,
    /// A shell prompt.
    Prompt = 2,
}

impl Cell {
    /// Construct an unstyled scalar cell.
    #[must_use]
    pub fn init(cp: char) -> Self {
        Self(u64::from(cp) << 2)
    }

    /// Whether every packed bit is zero.
    #[must_use]
    pub fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Whether the cell carries non-NUL text.
    #[must_use]
    pub fn has_text(self) -> bool {
        self.codepoint() != '\0'
    }

    /// Read the scalar, or NUL for background-only cells.
    #[must_use]
    pub fn codepoint(self) -> char {
        match self.content() {
            Content::Codepoint(cp) | Content::CodepointGrapheme(cp) => cp,
            Content::BgColorPalette(_) | Content::BgColorRgb(_) => '\0',
        }
    }

    /// Number of occupied grid columns, including spacers.
    #[must_use]
    pub fn grid_width(self) -> u8 {
        if self.wide() == Wide::Wide { 2 } else { 1 }
    }

    /// Whether this cell references a nondefault style.
    #[must_use]
    pub fn has_styling(self) -> bool {
        self.style_id() != 0
    }

    /// Whether the cell is narrow text with no scalar.
    #[must_use]
    pub fn is_empty(self) -> bool {
        matches!(
            self.content(),
            Content::Codepoint('\0') | Content::CodepointGrapheme('\0')
        ) && self.wide() == Wide::Narrow
    }

    /// Whether the cell refers to a cluster suffix.
    #[must_use]
    pub fn has_grapheme(self) -> bool {
        self.content_tag() == ContentTag::CodepointGrapheme
    }

    /// Whether any supplied cell carries non-NUL text.
    #[must_use]
    pub fn has_text_any(cells: &[Self]) -> bool {
        cells.iter().any(|cell| cell.has_text())
    }

    /// Read the active content value.
    #[must_use]
    pub fn content(self) -> Content {
        let value = self.field(2, 24);
        match self.content_tag() {
            ContentTag::Codepoint => Content::Codepoint(char::from_u32(value).unwrap_or('\0')),
            ContentTag::CodepointGrapheme => {
                Content::CodepointGrapheme(char::from_u32(value).unwrap_or('\0'))
            }
            ContentTag::BgColorPalette => Content::BgColorPalette(value.to_le_bytes()[0]),
            ContentTag::BgColorRgb => {
                let [r, g, b, _] = value.to_le_bytes();
                Content::BgColorRgb(Rgb { r, g, b })
            }
        }
    }

    /// Replace only the content tag and payload.
    pub fn set_content(&mut self, value: Content) {
        let (tag, payload) = match value {
            Content::Codepoint(cp) => (ContentTag::Codepoint, u32::from(cp)),
            Content::CodepointGrapheme(cp) => (ContentTag::CodepointGrapheme, u32::from(cp)),
            Content::BgColorPalette(index) => (ContentTag::BgColorPalette, u32::from(index)),
            Content::BgColorRgb(Rgb { r, g, b }) => {
                (ContentTag::BgColorRgb, u32::from_le_bytes([r, g, b, 0]))
            }
        };
        self.set_field(0, 2, u32::from(tag as u8));
        self.set_field(2, 24, payload);
    }

    /// Read the content discriminator.
    #[must_use]
    pub fn content_tag(self) -> ContentTag {
        match self.field(0, 2) {
            0 => ContentTag::Codepoint,
            1 => ContentTag::CodepointGrapheme,
            2 => ContentTag::BgColorPalette,
            _ => ContentTag::BgColorRgb,
        }
    }

    /// Read the page-local style identifier.
    #[must_use]
    pub fn style_id(self) -> crate::style::Id {
        let bytes = (self.0 >> 26).to_le_bytes();
        u16::from_le_bytes([bytes[0], bytes[1]])
    }

    /// Set the page-local style identifier without altering content.
    pub fn set_style_id(&mut self, value: crate::style::Id) {
        self.set_field(26, 16, u32::from(value));
    }

    /// Read the wide-character role.
    #[must_use]
    pub fn wide(self) -> Wide {
        match self.field(42, 2) {
            0 => Wide::Narrow,
            1 => Wide::Wide,
            2 => Wide::SpacerTail,
            _ => Wide::SpacerHead,
        }
    }

    /// Set the wide-character role.
    pub fn set_wide(&mut self, value: Wide) {
        self.set_field(42, 2, u32::from(value as u8));
    }

    /// Read the shell semantic role.
    #[must_use]
    pub fn semantic_content(self) -> SemanticContent {
        match self.field(46, 2) {
            1 => SemanticContent::Input,
            2 => SemanticContent::Prompt,
            _ => SemanticContent::Output,
        }
    }

    /// Set the shell semantic role.
    pub fn set_semantic_content(&mut self, value: SemanticContent) {
        self.set_field(46, 2, u32::from(value as u8));
    }

    /// Read the protected-write flag.
    #[must_use]
    pub fn protected(self) -> bool {
        self.field(44, 1) != 0
    }

    /// Set the protected-write flag.
    pub fn set_protected(&mut self, value: bool) {
        self.set_field(44, 1, u32::from(value));
    }

    /// Read the page-owned link flag.
    #[must_use]
    pub fn hyperlink(self) -> bool {
        self.field(45, 1) != 0
    }

    /// Set the page-owned link flag.
    pub fn set_hyperlink(&mut self, value: bool) {
        self.set_field(45, 1, u32::from(value));
    }

    /// Read a bounded packed field.
    fn field(self, shift: u32, width: u32) -> u32 {
        let bytes = ((self.0 >> shift) & ((1 << width) - 1)).to_le_bytes();
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }

    /// Replace a bounded packed field.
    fn set_field(&mut self, shift: u32, width: u32, value: u32) {
        let mask = ((1_u64 << width) - 1) << shift;
        self.0 = (self.0 & !mask) | (u64::from(value) << shift);
    }
}
