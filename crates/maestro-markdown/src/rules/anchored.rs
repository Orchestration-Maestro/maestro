//! Recognizers that can only match at the start of their input.
use regress::{Match, Regex};

/// A recognizer whose match, if any, begins at the first byte of the text it is given.
///
/// The `regress` backend that the workspace builds retries a failed `^` pattern at every later
/// start position, so a miss costs the length of the text. This owner ends the pattern in an
/// empty alternative that matches only at the start, so a miss at the start still produces a
/// match, at the first start position, and the search stops there. That sentinel match is
/// recognized by its one extra trailing group and reported as no match.
pub(crate) struct Anchored {
    /// The pattern followed by the start-only sentinel alternative.
    regex: Regex,
}

impl Anchored {
    /// Compile `source` anchored at the start of the text.
    ///
    /// The sources used here declare no named groups, so the sentinel group stays unnamed.
    pub(crate) fn new(source: &str, flags: &str) -> Option<Self> {
        let regex = Regex::with_flags(&["^(?:", source, ")|^()"].concat(), flags).ok()?;
        Some(Self { regex })
    }

    /// The match that begins at the start of `text`, with the groups the source declares.
    pub(crate) fn find(&self, text: &str) -> Option<Match> {
        let mut matched = self.regex.find(text)?;
        match matched.captures.pop()? {
            Some(_) => None,
            None => Some(matched),
        }
    }
}
