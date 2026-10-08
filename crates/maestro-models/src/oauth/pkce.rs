//! Proof Key for Code Exchange generation.

use crate::DiagnosticErrorInfo;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest as _, Sha256};

/// Proof key and its SHA-256 challenge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pkce {
    /// Unpadded URL-safe Base64 of 32 secure random bytes.
    pub verifier: String,
    /// Unpadded URL-safe Base64 of the verifier text's SHA-256 digest.
    pub challenge: String,
}

/// Generate a proof key, returning the native entropy failure if entropy is unavailable.
///
/// # Errors
/// Returns the system entropy error with its native message.
pub fn generate_pkce() -> Result<Pkce, DiagnosticErrorInfo> {
    generate_with_entropy(|bytes| {
        getrandom::fill(bytes).map_err(|error| DiagnosticErrorInfo {
            message: error.to_string(),
            name: None,
            stack: None,
            code: None,
        })
    })
}

/// Request entropy once and hash the encoded verifier text.
fn generate_with_entropy(
    entropy: impl FnOnce(&mut [u8; 32]) -> Result<(), DiagnosticErrorInfo>,
) -> Result<Pkce, DiagnosticErrorInfo> {
    let mut bytes = [0; 32];
    entropy(&mut bytes)?;
    let verifier = URL_SAFE_NO_PAD.encode(bytes);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    Ok(Pkce {
        verifier,
        challenge,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    /// Controlled oracle data.
    struct Vector {
        /// Supplied oracle bytes.
        bytes: Vec<u8>,
        /// Supplied oracle verifier.
        verifier: String,
        /// Supplied oracle challenge.
        challenge: String,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    /// Controlled oracle data.
    struct Corpus {
        /// Supplied oracle pkce.
        pkce: Vec<Vector>,
        /// Supplied oracle entropy failure.
        entropy_failure: String,
    }

    #[test]
    fn pkce_hashes_verifier_text() {
        let corpus: Corpus =
            serde_json::from_str(include_str!("../../tests/fixtures/oauth_pkce.json")).unwrap();
        assert_eq!(corpus.pkce.len(), 4);
        for Vector {
            bytes,
            verifier,
            challenge,
        } in corpus.pkce
        {
            let mut calls = 0;
            let actual = generate_with_entropy(|output| {
                calls += 1;
                assert_eq!(output.len(), 32);
                output.copy_from_slice(&bytes);
                Ok(())
            })
            .unwrap();
            assert_eq!(calls, 1);
            assert_eq!(
                actual,
                Pkce {
                    verifier,
                    challenge
                }
            );
        }
    }
    #[test]
    fn pkce_propagates_entropy_failure() {
        let value: Corpus =
            serde_json::from_str(include_str!("../../tests/fixtures/oauth_pkce.json")).unwrap();
        let error = DiagnosticErrorInfo {
            message: value.entropy_failure,
            name: Some("EntropyError".into()),
            code: Some(crate::DiagnosticCode::Number(7.0)),
            stack: Some("supplied trace".into()),
        };
        let actual = generate_with_entropy(|_| Err(error.clone())).unwrap_err();
        assert_eq!(actual, error);
    }
}
