//! Exact integrity checks for the optional shareware download.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use sha2::{Digest, Sha256};
use std::fmt::Write;

/// Accepts only the complete expected SHA-256 digest.
pub(super) fn matches_checksum(bytes: &[u8], expected: &str) -> bool {
    let mut actual = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        if write!(&mut actual, "{byte:02x}").is_err() {
            return false;
        }
    }
    actual == expected
}
