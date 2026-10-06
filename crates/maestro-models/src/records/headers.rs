//! Copy normalized header entries with ordinary-object enumeration semantics.
/// Assign entries, enumerating array-index keys first and retaining last values.
pub fn headers_to_record<I>(headers: I) -> serde_json::Map<String, serde_json::Value>
where
    I: IntoIterator<Item = (String, String)>,
{
    let mut result = serde_json::Map::new();
    for (k, v) in headers {
        if k != "__proto__" {
            result.insert(k, serde_json::Value::String(v));
        }
    }
    let mut entries = result.into_iter().collect::<Vec<_>>();
    entries.sort_by_key(|(key, _)| {
        key.parse::<u32>()
            .ok()
            .filter(|index| *index != u32::MAX && index.to_string() == *key)
            .map_or((1, 0), |index| (0, index))
    });
    entries.into_iter().collect()
}
