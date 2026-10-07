//! Copy already-normalized response header entries.

use std::collections::BTreeMap;

/// Copy entries, replacing an earlier value for each repeated exact key.
#[must_use]
pub fn headers_to_record<'a>(
    headers: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> BTreeMap<String, String> {
    headers
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect()
}
