//! Proof-key encoding for the provider's authorization flow.
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest as _, Sha256};

/// Encodes secure verifier bytes and hashes the encoded text.
///
/// The native counterpart belongs to `maestro-models/src/oauth/pkce.rs`.
pub fn generate(bytes: [u8; 32]) -> (String, String) {
    let verifier = URL_SAFE_NO_PAD.encode(bytes);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}
