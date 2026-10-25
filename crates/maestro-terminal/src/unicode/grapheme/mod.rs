//! Pairwise terminal grapheme boundaries and cluster-width effects.

use super::props::{GraphemeBreak as G, Properties};

/// Context retained between successive pairwise boundary decisions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum BreakState {
    /// No partial sequence.
    #[default]
    Default,
    /// One unpaired regional indicator.
    RegionalIndicator,
    /// An emoji followed by possible extensions and a joiner.
    ExtendedPictographic,
    /// An Indic consonant followed by extensions.
    IndicConjunctBreakConsonant,
    /// An Indic consonant followed by at least one linker.
    IndicConjunctBreakLinker,
}

/// Effect of a continuing scalar on its existing cluster.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GraphemeWidthEffect {
    /// Discard an invalid variation selector without changing context.
    Ignore,
    /// Append without changing the occupied width.
    NoChange,
    /// Occupy two cells.
    Wide,
    /// Occupy one cell.
    Narrow,
}

/// Decide a boundary between non-control scalars, updating sequence context.
pub(crate) fn grapheme_break(previous: char, cp: char, state: &mut BreakState) -> bool {
    compute_break(
        Properties::get(previous).grapheme_break,
        Properties::get(cp).grapheme_break,
        state,
    )
}

/// Determine the width effect after a pair has been classified as continuing.
pub(crate) fn grapheme_width_effect(previous: char, cp: char) -> GraphemeWidthEffect {
    if matches!(cp, '\u{FE0E}' | '\u{FE0F}') {
        if !Properties::get(previous).emoji_vs_base {
            return GraphemeWidthEffect::Ignore;
        }
        return if cp == '\u{FE0F}' {
            GraphemeWidthEffect::Wide
        } else {
            GraphemeWidthEffect::Narrow
        };
    }
    if Properties::get(cp).width_zero_in_grapheme {
        GraphemeWidthEffect::NoChange
    } else {
        GraphemeWidthEffect::Wide
    }
}

/// Apply terminal-tailored Hangul, Indic, emoji and regional-indicator rules.
fn compute_break(a: G, b: G, state: &mut BreakState) -> bool {
    normalize(a, b, state);
    if matches!(
        (a, b),
        (G::L, G::L | G::V | G::Lv | G::Lvt) | (G::Lv | G::V, G::V | G::T) | (G::Lvt | G::T, G::T)
    ) || b == G::SpacingMark
        || a == G::Prepend
    {
        return false;
    }
    if indic(a, b, state) || emoji(a, b, state) {
        return false;
    }
    if a == G::RegionalIndicator && b == G::RegionalIndicator {
        let paired = *state == BreakState::Default;
        *state = if paired {
            BreakState::RegionalIndicator
        } else {
            BreakState::Default
        };
        return !paired;
    }
    !(b.is_extend() || b == G::Zwj)
}

/// Forget sequence context when either property cannot occur in that sequence.
fn normalize(a: G, b: G, state: &mut BreakState) {
    let valid = match state {
        BreakState::Default => true,
        BreakState::RegionalIndicator => a == G::RegionalIndicator && b == G::RegionalIndicator,
        BreakState::ExtendedPictographic => [a, b].iter().all(|g| {
            g.is_extend() || g.is_extended_pictographic() || matches!(g, G::Zwj | G::EmojiModifier)
        }),
        BreakState::IndicConjunctBreakConsonant | BreakState::IndicConjunctBreakLinker => {
            [a, b].iter().all(|g| {
                g.is_indic_extend()
                    || matches!(
                        g,
                        G::IndicConjunctBreakConsonant | G::IndicConjunctBreakLinker
                    )
            })
        }
    };
    if !valid {
        *state = BreakState::Default;
    }
}

/// Continue an Indic conjunct, including transitions observed in partial context.
fn indic(a: G, b: G, state: &mut BreakState) -> bool {
    if a == G::IndicConjunctBreakConsonant {
        if b.is_indic_extend() {
            *state = BreakState::IndicConjunctBreakConsonant;
            return true;
        }
        if b == G::IndicConjunctBreakLinker {
            *state = BreakState::IndicConjunctBreakLinker;
            return true;
        }
    } else if *state == BreakState::IndicConjunctBreakConsonant {
        if b == G::IndicConjunctBreakLinker {
            *state = BreakState::IndicConjunctBreakLinker;
            return true;
        }
        if b.is_indic_extend() {
            return true;
        }
        *state = BreakState::Default;
    } else if *state == BreakState::IndicConjunctBreakLinker {
        if b == G::IndicConjunctBreakLinker || b.is_indic_extend() {
            return true;
        }
        *state = BreakState::Default;
        return b == G::IndicConjunctBreakConsonant;
    }
    false
}

/// Continue a tailored emoji modifier or joiner sequence.
fn emoji(a: G, b: G, state: &mut BreakState) -> bool {
    if a.is_extended_pictographic() {
        if b.is_extend() || b == G::Zwj || (a == G::EmojiModifierBase && b == G::EmojiModifier) {
            *state = BreakState::ExtendedPictographic;
            return true;
        }
    } else if *state == BreakState::ExtendedPictographic {
        if (a.is_extend() || a == G::EmojiModifier) && (b.is_extend() || b == G::Zwj) {
            return true;
        }
        *state = BreakState::Default;
        return a == G::Zwj && b.is_extended_pictographic();
    }
    false
}

#[cfg(test)]
mod tests;
