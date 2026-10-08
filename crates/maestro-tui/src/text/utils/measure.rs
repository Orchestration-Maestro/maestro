//! Cell widths of graphemes and the cache of measured strings.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::LazyLock;

use regress::Regex;
use unicode_width::UnicodeWidthChar;

use super::parsed::Parsed;

/// Cells a tab occupies when a text is measured, wrapped or truncated.
pub(super) const TAB_CELLS: usize = 3;

/// Measured strings the width cache keeps before it evicts the oldest.
const WIDTH_CACHE_SIZE: usize = 512;

/// Compiles one of the fixed Unicode property expressions.
fn property(pattern: &str) -> Option<Regex> {
    Regex::with_flags(pattern, "v").ok()
}

/// A cluster made only of ignorable, control and mark scalars.
static ZERO_WIDTH: LazyLock<Option<Regex>> = LazyLock::new(|| {
    property(r"^(?:\p{Default_Ignorable_Code_Point}|\p{Control}|\p{Mark}|\p{Surrogate})+$")
});

/// Leading scalars that never carry the width of a cluster.
static LEADING_NON_PRINTING: LazyLock<Option<Regex>> = LazyLock::new(|| {
    property(r"^[\p{Default_Ignorable_Code_Point}\p{Control}\p{Format}\p{Mark}\p{Surrogate}]+")
});

/// A complete recommended-for-general-interchange emoji sequence.
static RGI_EMOJI: LazyLock<Option<Regex>> = LazyLock::new(|| property(r"^\p{RGI_Emoji}$"));

/// Whether the fixed expression matches `text`.
fn matches(expression: &LazyLock<Option<Regex>>, text: &str) -> bool {
    expression
        .as_ref()
        .is_some_and(|expression| expression.find(text).is_some())
}

/// Cells of one scalar: wide, fullwidth and regional indicator forms take two, everything
/// else one.
///
/// The width library lists U+17A4 and U+17D8 as two and three cells; both take one.
fn scalar_cells(scalar: char) -> usize {
    match scalar {
        '\u{17a4}' | '\u{17d8}' => 1,
        '\u{1f1e6}'..='\u{1f1ff}' => 2,
        _ => scalar.width().unwrap_or(0).clamp(1, 2),
    }
}

/// Cells a trailing scalar adds to the cluster it belongs to.
fn trailing_cells(scalar: char) -> usize {
    match scalar {
        '\u{ff00}'..='\u{ffef}' => scalar_cells(scalar),
        '\u{e33}' | '\u{eb3}' => 1,
        _ => 0,
    }
}

/// Cells of one grapheme cluster, counting a tab as zero.
pub(super) fn grapheme_cells(segment: &str) -> usize {
    let mut scalars = segment.chars();
    if let (Some(only), None) = (scalars.next(), scalars.next())
        && only.is_ascii()
    {
        return usize::from(!only.is_ascii_control());
    }
    if matches(&ZERO_WIDTH, segment) {
        return 0;
    }
    if matches(&RGI_EMOJI, segment) {
        return 2;
    }
    let skipped = LEADING_NON_PRINTING
        .as_ref()
        .and_then(|expression| expression.find(segment))
        .map_or(0, |found| found.range.end);
    let mut scalars = segment[skipped..].chars();
    scalars.next().map_or(0, |base| {
        scalars.fold(scalar_cells(base), |cells, next| {
            cells + trailing_cells(next)
        })
    })
}

/// Whether every scalar is printable ASCII.
pub(super) fn printable_ascii(text: &str) -> bool {
    text.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
}

/// Strings already measured, evicted oldest first and never refreshed by a hit.
#[derive(Default)]
struct WidthCache {
    /// Measured widths by original string.
    widths: HashMap<Rc<str>, usize>,
    /// Keys in insertion order.
    order: VecDeque<Rc<str>>,
}

impl WidthCache {
    /// Cached width of `text`, if any.
    fn get(&self, text: &str) -> Option<usize> {
        self.widths.get(text).copied()
    }

    /// Stores a freshly measured width, evicting the oldest entry when full.
    fn insert(&mut self, text: &str, width: usize) {
        if self.order.len() >= WIDTH_CACHE_SIZE
            && let Some(oldest) = self.order.pop_front()
        {
            self.widths.remove(&oldest);
        }
        let key: Rc<str> = Rc::from(text);
        self.order.push_back(Rc::clone(&key));
        self.widths.insert(key, width);
    }
}

thread_local! {
    /// Per-thread cache behind [`visible_width`].
    static CACHE: RefCell<WidthCache> = RefCell::new(WidthCache::default());
}

/// Terminal columns `text` occupies; supported escapes take none and a tab takes three.
#[must_use]
pub fn visible_width(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    if printable_ascii(text) {
        return text.len();
    }
    if let Some(width) = CACHE.with_borrow(|cache| cache.get(text)) {
        return width;
    }
    let parsed = Parsed::parse(text);
    let width = parsed
        .graphemes(0..parsed.visible.len(), TAB_CELLS)
        .map(|grapheme| grapheme.cells)
        .sum();
    CACHE.with_borrow_mut(|cache| cache.insert(text, width));
    width
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cached(text: &str) -> bool {
        CACHE.with_borrow(|cache| cache.get(text).is_some())
    }

    #[test]
    fn width_cache_preserves_original_keys_and_insertion_eviction() {
        assert_eq!(visible_width("plain ascii"), 11);
        assert!(!cached("plain ascii"), "printable ASCII bypasses the cache");

        let variants = [("\t", 3), ("\x1b[31m\u{754c}", 2), ("\u{754c}", 2)];
        for (text, width) in variants {
            assert_eq!(visible_width(text), width);
            assert!(cached(text), "{text:?} keeps its own key");
        }

        let fillers: Vec<String> = (0..WIDTH_CACHE_SIZE - variants.len())
            .map(|n| format!("\u{754c}{n}"))
            .collect();
        for (n, filler) in fillers.iter().enumerate() {
            assert_eq!(visible_width(filler), 2 + n.to_string().len());
        }
        assert!(
            variants.iter().all(|(text, _)| cached(text)) && fillers.iter().all(|f| cached(f)),
            "a full cache has evicted nothing"
        );
        assert_eq!(visible_width("\t"), 3);

        assert_eq!(visible_width("\u{754c}overflow"), 10);
        assert!(!cached("\t"), "a hit must not refresh the oldest entry");
        assert!(
            cached("\x1b[31m\u{754c}") && cached("\u{754c}") && cached("\u{754c}overflow"),
            "only the oldest entry is evicted"
        );
        assert!(fillers.iter().all(|filler| cached(filler)));
    }
}
