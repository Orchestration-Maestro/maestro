//! Reported working directories.

use std::path::PathBuf;

/// Accept local file URIs or quoted and bare paths.
pub(in crate::pane) fn parse_reported_cwd(value: &[u8]) -> Option<PathBuf> {
    let value = std::str::from_utf8(value).ok()?.trim();
    if value.starts_with("file://") {
        return parse_file_uri_cwd(value);
    }
    let path = value.trim_matches('"');
    (!path.is_empty()).then(|| PathBuf::from(path))
}

/// Decode a local file URI, rejecting remote hosts and malformed UTF-8.
fn parse_file_uri_cwd(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let path = if rest.starts_with('/') {
        rest
    } else if let Some(slash) = rest.find('/') {
        let host = &rest[..slash];
        if !(host.is_empty() || host.eq_ignore_ascii_case("localhost")) {
            return None;
        }
        &rest[slash..]
    } else {
        rest
    };
    let path = percent_decode_utf8(path)?;

    #[cfg(windows)]
    {
        Some(PathBuf::from(windows_uri_path(&path)))
    }

    #[cfg(not(windows))]
    Some(PathBuf::from(path))
}

/// Spell a file-URI path as Windows reads it: a drive path loses the leading
/// slash that stands before its letter, and every `/` becomes `\`.
#[cfg(any(windows, test))]
pub(super) fn windows_uri_path(path: &str) -> String {
    let path = match path.strip_prefix('/') {
        Some(below)
            if below.len() >= 2
                && below.as_bytes()[1] == b':'
                && below.as_bytes()[0].is_ascii_alphabetic() =>
        {
            below
        }
        _ => path,
    };
    path.replace('/', "\\")
}

/// Decode percent escapes without accepting invalid bytes.
fn percent_decode_utf8(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut idx = 0;
    while idx < bytes.len() {
        if bytes[idx] == b'%' {
            let hi = *bytes.get(idx + 1)?;
            let lo = *bytes.get(idx + 2)?;
            output.push(hex_value(hi)? * 16 + hex_value(lo)?);
            idx += 3;
        } else {
            output.push(bytes[idx]);
            idx += 1;
        }
    }
    String::from_utf8(output).ok()
}

/// Read one ASCII hexadecimal digit.
fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
