//! Copy already-normalized header entries without applying header policy.
/// Copy entries verbatim; repeated assignment keeps key position and the last value.
pub fn headers_to_record<I>(headers: I) -> serde_json::Map<String, serde_json::Value>
where
    I: IntoIterator<Item = (String, String)>,
{
    let mut result = serde_json::Map::new();
    for (k, v) in headers {
        result.insert(k, serde_json::Value::String(v));
    }
    result
}
