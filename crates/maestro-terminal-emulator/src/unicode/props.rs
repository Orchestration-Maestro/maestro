//! Unicode properties tailored for terminal cells.

use icu_properties::props::{
    DefaultIgnorableCodePoint, EastAsianWidth, EmojiModifier, EmojiModifierBase,
    ExtendedPictographic, GeneralCategory, GraphemeClusterBreak, IndicConjunctBreak,
};
use icu_properties::{CodePointMapData, CodePointSetData};

/// The terminal cell properties of one Unicode scalar.
pub(crate) struct Properties {
    /// Cell width, limited to zero, one or two.
    pub(crate) width: u8,
    /// Whether this scalar contributes no width inside a cluster.
    pub(crate) width_zero_in_grapheme: bool,
    /// Tailored boundary property with controls mapped to ordinary scalars.
    pub(crate) grapheme_break: GraphemeBreak,
    /// Whether emoji variation selectors can modify this scalar.
    pub(crate) emoji_vs_base: bool,
}

impl Properties {
    /// Derive terminal properties from the compiled Unicode data.
    pub(crate) fn get(c: char) -> Self {
        let category = CodePointMapData::<GeneralCategory>::new().get(c);
        let grapheme = CodePointMapData::<GraphemeClusterBreak>::new().get(c);
        let east_asian = CodePointMapData::<EastAsianWidth>::new().get(c);
        let modifier = CodePointSetData::new::<EmojiModifier>().contains(c);
        let invisible = matches!(
            category,
            GeneralCategory::Control
                | GeneralCategory::LineSeparator
                | GeneralCategory::ParagraphSeparator
        ) || (c != '\u{AD}'
            && CodePointSetData::new::<DefaultIgnorableCodePoint>().contains(c));
        let standalone = if invisible {
            0
        } else if matches!(c, '\u{2E3A}' | '\u{2E3B}' | '\u{20E3}')
            || matches!(east_asian, EastAsianWidth::Wide | EastAsianWidth::Fullwidth)
            || grapheme == GraphemeClusterBreak::RegionalIndicator
        {
            2
        } else {
            1
        };
        let zero_in_grapheme = invisible
            || modifier
            || matches!(
                category,
                GeneralCategory::NonspacingMark | GeneralCategory::EnclosingMark
            )
            || matches!(
                grapheme,
                GraphemeClusterBreak::V | GraphemeClusterBreak::T | GraphemeClusterBreak::Prepend
            );
        Self {
            width_zero_in_grapheme: zero_in_grapheme,
            grapheme_break: GraphemeBreak::get(c, grapheme),
            emoji_vs_base: super::variation::is_base(c),
            width: if zero_in_grapheme && !modifier && grapheme != GraphemeClusterBreak::Prepend {
                0
            } else {
                standalone
            },
        }
    }
}

/// Tailored non-control grapheme properties used by terminal streaming.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GraphemeBreak {
    /// Other.
    Other,
    /// Prepend.
    Prepend,
    /// Regional indicator.
    RegionalIndicator,
    /// Spacing mark.
    SpacingMark,
    /// L.
    L,
    /// V.
    V,
    /// T.
    T,
    /// Lv.
    Lv,
    /// Lvt.
    Lvt,
    /// Zwj.
    Zwj,
    /// Zwnj.
    Zwnj,
    /// Extended pictographic.
    ExtendedPictographic,
    /// Emoji modifier base.
    EmojiModifierBase,
    /// Emoji modifier.
    EmojiModifier,
    /// Indic conjunct break extend.
    IndicConjunctBreakExtend,
    /// Indic conjunct break linker.
    IndicConjunctBreakLinker,
    /// Indic conjunct break consonant.
    IndicConjunctBreakConsonant,
}

impl GraphemeBreak {
    /// Map compiled Unicode properties to the terminal's tailored vocabulary.
    fn get(c: char, grapheme: GraphemeClusterBreak) -> Self {
        if CodePointSetData::new::<EmojiModifier>().contains(c) {
            return Self::EmojiModifier;
        }
        if CodePointSetData::new::<EmojiModifierBase>().contains(c) {
            return Self::EmojiModifierBase;
        }
        if CodePointSetData::new::<ExtendedPictographic>().contains(c) {
            return Self::ExtendedPictographic;
        }
        match CodePointMapData::<IndicConjunctBreak>::new().get(c) {
            IndicConjunctBreak::Extend => {
                if c == '\u{200D}' {
                    Self::Zwj
                } else {
                    Self::IndicConjunctBreakExtend
                }
            }
            IndicConjunctBreak::Linker => Self::IndicConjunctBreakLinker,
            IndicConjunctBreak::Consonant => Self::IndicConjunctBreakConsonant,
            _ => match grapheme {
                GraphemeClusterBreak::Prepend => Self::Prepend,
                GraphemeClusterBreak::RegionalIndicator => Self::RegionalIndicator,
                GraphemeClusterBreak::SpacingMark => Self::SpacingMark,
                GraphemeClusterBreak::L => Self::L,
                GraphemeClusterBreak::V => Self::V,
                GraphemeClusterBreak::T => Self::T,
                GraphemeClusterBreak::LV => Self::Lv,
                GraphemeClusterBreak::LVT => Self::Lvt,
                GraphemeClusterBreak::ZWJ => Self::Zwj,
                GraphemeClusterBreak::Extend => Self::Zwnj,
                _ => Self::Other,
            },
        }
    }

    /// Whether this is an extension in an Indic conjunct.
    pub(crate) fn is_indic_extend(self) -> bool {
        matches!(self, Self::IndicConjunctBreakExtend | Self::Zwj)
    }

    /// Whether this is a non-modifier extension in a tailored emoji cluster.
    pub(crate) fn is_extend(self) -> bool {
        matches!(
            self,
            Self::Zwnj | Self::IndicConjunctBreakExtend | Self::IndicConjunctBreakLinker
        )
    }

    /// Whether this can begin an emoji sequence.
    pub(crate) fn is_extended_pictographic(self) -> bool {
        matches!(self, Self::ExtendedPictographic | Self::EmojiModifierBase)
    }
}
