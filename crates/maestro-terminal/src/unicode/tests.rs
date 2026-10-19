//! Exhaustive scalar-property conformance against captured Unicode data.

use super::props::{GraphemeBreak, Properties};

#[test]
fn unicode_properties_cover_all_scalar_fields() {
    let runs: Vec<_> = include_str!("../../tests/fixtures/unicode/property-runs.txt")
        .lines()
        .map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            assert_eq!(fields.len(), 5);
            (
                u32::from_str_radix(fields[0], 16).unwrap(),
                fields[1].parse::<u8>().unwrap(),
                fields[2].parse::<bool>().unwrap(),
                parse_property(fields[3]),
                fields[4].parse::<bool>().unwrap(),
            )
        })
        .collect();
    assert_eq!(runs.len(), 2499);
    assert_eq!(runs[0].0, 0);
    assert!(runs.windows(2).all(|pair| pair[0].0 < pair[1].0));
    let mut current = 0;
    let mut scalars = 0;
    for scalar in 0..=0x10_FFFF {
        while current + 1 < runs.len() && scalar >= runs[current + 1].0 {
            current += 1;
        }
        let Some(cp) = char::from_u32(scalar) else {
            continue;
        };
        let p = Properties::get(cp);
        let (_, width, zero, gb, vs) = runs[current];
        assert_eq!(p.width, width, "width U+{scalar:04X}");
        assert_eq!(
            p.width_zero_in_grapheme, zero,
            "cluster width U+{scalar:04X}"
        );
        assert_eq!(p.grapheme_break, gb, "break U+{scalar:04X}");
        assert_eq!(p.emoji_vs_base, vs, "variation U+{scalar:04X}");
        scalars += 1;
    }
    assert_eq!(scalars, 0x11_0000 - 0x800);
}

/// Decode each captured property without changing the fixture.
pub(super) fn parse_property(name: &str) -> GraphemeBreak {
    match name {
        "other" => GraphemeBreak::Other,
        "prepend" => GraphemeBreak::Prepend,
        "regional_indicator" => GraphemeBreak::RegionalIndicator,
        "spacing_mark" => GraphemeBreak::SpacingMark,
        "l" => GraphemeBreak::L,
        "v" => GraphemeBreak::V,
        "t" => GraphemeBreak::T,
        "lv" => GraphemeBreak::Lv,
        "lvt" => GraphemeBreak::Lvt,
        "zwj" => GraphemeBreak::Zwj,
        "zwnj" => GraphemeBreak::Zwnj,
        "extended_pictographic" => GraphemeBreak::ExtendedPictographic,
        "emoji_modifier_base" => GraphemeBreak::EmojiModifierBase,
        "emoji_modifier" => GraphemeBreak::EmojiModifier,
        "indic_conjunct_break_extend" => GraphemeBreak::IndicConjunctBreakExtend,
        "indic_conjunct_break_linker" => GraphemeBreak::IndicConjunctBreakLinker,
        "indic_conjunct_break_consonant" => GraphemeBreak::IndicConjunctBreakConsonant,
        _ => panic!("unknown property {name}"),
    }
}
