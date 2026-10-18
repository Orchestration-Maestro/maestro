//! Unicode properties tailored for terminal cells.

use icu_properties::props::{
    DefaultIgnorableCodePoint, EastAsianWidth, EmojiModifier, GeneralCategory, GraphemeClusterBreak,
};
use icu_properties::{CodePointMapData, CodePointSetData};

/// The terminal cell properties of one Unicode scalar.
pub(super) struct Properties {
    /// Cell width, limited to zero, one or two.
    pub(super) width: u8,
}

impl Properties {
    /// Derive terminal properties from the compiled Unicode data.
    pub(super) fn get(c: char) -> Self {
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
            width: if zero_in_grapheme && !modifier && grapheme != GraphemeClusterBreak::Prepend {
                0
            } else {
                standalone
            },
        }
    }
}
