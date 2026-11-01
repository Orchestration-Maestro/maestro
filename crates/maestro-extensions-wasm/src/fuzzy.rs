//! Native fuzzy ranking with caller-owned item identity.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{ExtensionAPI, ExtensionResult};
/// A native match decision; lower scores rank first.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct FuzzyMatch {
    /// Whether the query matches.
    pub matches: bool,
    /// The native score, including its consecutive and boundary preferences.
    pub score: f64,
}
/// Scores the supplied query through the native fuzzy owner.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn fuzzy_match(api: &ExtensionAPI, query: &str, text: &str) -> ExtensionResult<FuzzyMatch> {
    api.helper("fuzzy.match", &(query, text))
}

/// Filters a fixed borrowed slice without serializing or replacing its items.
/// An empty query calls no getter. Otherwise each original item is projected once in input
/// order, the host ranks those strings, and checked positions select the original references.
/// The getter is borrowed only for this synchronous call, including an error return.
///
/// # Errors
/// Returns host failures, invalid responses or an out-of-range native selection.
pub fn fuzzy_filter<'a, T>(
    api: &ExtensionAPI,
    items: &'a [T],
    query: &str,
    get_text: impl FnMut(&T) -> String,
) -> ExtensionResult<Vec<&'a T>> {
    if !api.helper::<_, bool>("fuzzy.project", &(query,))? {
        return Ok(items.iter().collect());
    }
    let texts: Vec<String> = items.iter().map(get_text).collect();
    let selected: Vec<usize> = api.helper("fuzzy.filter", &(texts, query))?;
    selected
        .into_iter()
        .map(|index| {
            items
                .get(index)
                .ok_or_else(|| format!("Native fuzzy selection index {index} is out of range"))
        })
        .collect()
}
