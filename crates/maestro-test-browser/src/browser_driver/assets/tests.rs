//! Verify the report cache before admitting executable bytes.

use super::*;

/// Known SHA-256 inputs distinguish exact bytes from a corrupt or empty asset.
#[test]
fn report_assets_require_the_exact_pinned_digest() {
    let asset = Asset {
        url: "https://example.invalid/asset",
        source: "https://example.invalid/asset",
        file: "asset",
        digest: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
    };
    verify(b"abc", &asset).unwrap();
    assert!(verify(b"abd", &asset).is_err());
    assert!(verify(b"", &asset).is_err());
}
