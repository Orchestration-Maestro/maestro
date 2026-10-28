//! Synthetic integrity checks never require game data.
use super::matches_checksum;

/// Standard synthetic bytes accept only their own complete SHA-256 digest.
#[test]
fn checksum_accepts_exact_bytes_and_rejects_modified_body() {
    let digest = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    assert!(matches_checksum(b"abc", digest));
    assert!(!matches_checksum(b"abd", digest));
    assert!(!matches_checksum(b"abc", ""));
}
