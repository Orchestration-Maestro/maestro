//! Recorded sources and their windows, read from committed fixtures.
use serde::Deserialize;

/// Recorded sources and their windows.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Corpus<W> {
    /// Whole sources the windows select from, each with its own windows.
    pub(super) texts: Vec<Source<W>>,
}

/// One whole source with every recorded window over it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Source<W> {
    /// Whole source text.
    pub(super) text: String,
    /// Recorded windows over this source.
    pub(super) windows: Vec<W>,
}
