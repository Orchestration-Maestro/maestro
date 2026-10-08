//! Cleanup at an explicitly UTF-16 encoded text boundary.

/// Remove unmatched UTF-16 units while preserving valid characters.
#[must_use]
pub fn sanitize_surrogates(text: &[u16]) -> String {
    char::decode_utf16(text.iter().copied())
        .filter_map(Result::ok)
        .collect()
}
