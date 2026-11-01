//! Bounded native names beside a publication target.
use sha2::{Digest, Sha256};
use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

/// Maximum native file-name length in bytes on Unix or UTF-16 units on Windows.
const NAME_LIMIT: usize = 255;
/// Hexadecimal digits retained from the full file name's SHA-256.
const DIGEST_LENGTH: usize = 16;

/// Appends a suffix, shortening only a name that would exceed the native limit.
pub(crate) fn beside(target: &Path, prefix: &str, suffix: &str) -> PathBuf {
    let leaf = target.file_name().unwrap_or_default();
    let available = NAME_LIMIT.saturating_sub(prefix.len() + suffix.len());
    let mut name = OsString::from(prefix);
    if native_length(leaf) <= available {
        name.push(leaf);
    } else {
        let digest = Sha256::digest(leaf.as_encoded_bytes());
        let short = u64::from_be_bytes([
            digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7],
        ]);
        name.push(cut(leaf, available.saturating_sub(DIGEST_LENGTH + 1)));
        name.push(format!("-{short:016x}"));
    }
    name.push(suffix);
    target.with_file_name(name)
}

/// Counts units in the platform's file-name limit.
fn native_length(name: &OsStr) -> usize {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        name.encode_wide().count()
    }
    #[cfg(not(windows))]
    {
        name.as_encoded_bytes().len()
    }
}

/// Cuts a native file name without splitting a Unicode character.
fn cut(name: &OsStr, limit: usize) -> OsString {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};
        let bytes = name.as_bytes();
        let mut end = limit.min(bytes.len());
        while end > 0 && end < bytes.len() && bytes[end] & 0xc0 == 0x80 {
            end -= 1;
        }
        OsString::from_vec(bytes[..end].to_vec())
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::{OsStrExt, OsStringExt};
        let mut units: Vec<_> = name.encode_wide().take(limit).collect();
        if units
            .last()
            .is_some_and(|unit| (0xd800..=0xdbff).contains(unit))
        {
            units.pop();
        }
        OsString::from_wide(&units)
    }
    #[cfg(not(any(unix, windows)))]
    {
        name.to_string_lossy()
            .chars()
            .take(limit)
            .collect::<String>()
            .into()
    }
}
