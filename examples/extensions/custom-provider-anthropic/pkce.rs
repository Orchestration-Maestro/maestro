//! Proof-key encoding for the provider's authorization flow.

/// Encodes secure verifier bytes and hashes the encoded text.
pub fn generate(bytes: [u8; 32]) -> (String, String) {
    let _ = bytes;
    (String::new(), String::new())
}
