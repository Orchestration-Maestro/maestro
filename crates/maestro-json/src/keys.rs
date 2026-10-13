//! Canonical array-index keys and the entry order built on them.

/// Parse a canonical array index: ASCII digits only, no sign, no leading zero except for
/// `0` itself, and below 2^32 - 1.
#[must_use]
pub fn array_index(key: &str) -> Option<u32> {
    let canonical =
        key == "0" || (!key.starts_with('0') && key.bytes().all(|byte| byte.is_ascii_digit()));
    canonical
        .then(|| key.parse::<u32>().ok())
        .flatten()
        .filter(|index| *index != u32::MAX)
}

/// Order entries with array-index keys first in ascending numeric order, then every other
/// entry in its given order. The sort is stable and neither drops nor merges entries.
#[must_use]
pub fn canonical_entries<K: AsRef<str>, V>(
    entries: impl IntoIterator<Item = (K, V)>,
) -> Vec<(K, V)> {
    let mut ordered: Vec<_> = entries.into_iter().collect();
    ordered.sort_by_key(|(key, _)| array_index(key.as_ref()).map_or(u64::MAX, u64::from));
    ordered
}
