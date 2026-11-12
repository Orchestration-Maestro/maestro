//! Tailored boundary result and state conformance.

use super::{BreakState, compute_break, grapheme_break};
use crate::unicode::tests::parse_property;

#[test]
fn tailored_break_transitions_cover_all_property_states() {
    let data = include_str!("../../../tests/fixtures/unicode/break-transitions.txt");
    let mut inputs = std::collections::HashSet::new();
    for line in data.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        assert_eq!(fields.len(), 5);
        assert!(inputs.insert((fields[0], fields[1], fields[2])), "{line}");
        let mut state = parse_state(fields[0]);
        let result = compute_break(
            parse_property(fields[1]),
            parse_property(fields[2]),
            &mut state,
        );
        assert_eq!(result, fields[3].parse::<bool>().unwrap(), "{line}");
        assert_eq!(state, parse_state(fields[4]), "{line}");
    }
    assert_eq!(inputs.len(), 1445);
}

/// Decode retained context from the unmodified transition fixture.
fn parse_state(name: &str) -> BreakState {
    match name {
        "default" => BreakState::Default,
        "regional_indicator" => BreakState::RegionalIndicator,
        "extended_pictographic" => BreakState::ExtendedPictographic,
        "indic_conjunct_break_consonant" => BreakState::IndicConjunctBreakConsonant,
        "indic_conjunct_break_linker" => BreakState::IndicConjunctBreakLinker,
        _ => panic!("unknown state {name}"),
    }
}

#[test]
fn cluster_boundary_grapheme_break_emoji_modifier() {
    assert!(!grapheme_break(
        '\u{261D}',
        '\u{1F3FF}',
        &mut BreakState::default()
    ));
    assert!(grapheme_break('"', '\u{1F3FF}', &mut BreakState::default()));
}

#[test]
fn cluster_boundary_long_emoji_zwj_sequences() {
    let mut state = BreakState::default();
    let sequence = [
        '\u{1F469}',
        '\u{200D}',
        '\u{1F469}',
        '\u{200D}',
        '\u{1F467}',
        '\u{200D}',
        '\u{1F466}',
        '_',
    ];
    let mut chars = "👩‍👩‍👧‍👦_".chars();
    let mut previous = chars.next().unwrap();
    for (i, expected) in sequence[..7].iter().enumerate() {
        assert_eq!(previous, *expected);
        let next = chars.next().unwrap();
        assert_eq!(
            grapheme_break(previous, next, &mut state),
            i == 6,
            "pair {i}"
        );
        previous = next;
    }
    assert!(chars.next().is_none());
}

#[test]
fn cluster_width_effects_validate_selectors_and_spacing_contributions() {
    use super::{GraphemeWidthEffect as E, grapheme_width_effect};
    for (previous, cp, expected) in [
        ('"', '\u{FE0F}', E::Ignore),
        ('\u{2600}', '\u{FE0F}', E::Wide),
        ('\u{2600}', '\u{FE0E}', E::Narrow),
        ('o', '\u{301}', E::NoChange),
        ('\u{915}', '\u{93E}', E::Wide),
    ] {
        assert_eq!(grapheme_width_effect(previous, cp), expected);
    }
}
