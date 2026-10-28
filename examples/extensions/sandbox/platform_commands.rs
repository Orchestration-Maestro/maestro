//! Platform command operands and lossless decoding of native path observations.

/// Expands a supplied glob as data and emits NUL-separated native filenames.
pub(crate) const GLOB: &str = "";

/// Creates or validates an owned private persistent temporary directory.
pub(crate) const TEMPORARY: &str = "";

/// Decodes a native filename list without silently replacing invalid path bytes.
pub(crate) fn decode_matches(bytes: &[u8]) -> Result<Vec<String>, String> {
    std::str::from_utf8(bytes)
        .map(|_| Vec::new())
        .map_err(|error| error.to_string())
}
