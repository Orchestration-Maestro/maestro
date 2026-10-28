//! Authorization helpers delegate to the native OAuth primitive owner.
use super::{decode, encode};

/// Runs one OAuth operation without interpreting its result in the adapter.
pub(super) fn call(operation: &str, data: &str) -> Result<String, String> {
    let (): () = decode(data)?;
    encode(&operation)
}

#[cfg(test)]
mod tests {
    use super::call;
    use serde_json::Value;

    /// The URL-safe alphabet of unpadded Base64.
    fn is_url_safe(text: &str) -> bool {
        text.bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    }

    /// A proof key is the native owner's record: 32 random bytes encode to 43 characters.
    #[test]
    fn generate_pkce_returns_the_native_proof_key() {
        let key: Value =
            serde_json::from_str(&call("oauth.generate_pkce", "null").unwrap()).unwrap();
        for field in ["verifier", "challenge"] {
            let text = key[field].as_str().unwrap();
            assert_eq!(text.len(), 43, "{field}");
            assert!(is_url_safe(text), "{field}");
        }
        assert_ne!(key["verifier"], key["challenge"]);
    }

    /// Every call draws fresh entropy.
    #[test]
    fn generate_pkce_draws_fresh_entropy_per_call() {
        let first = call("oauth.generate_pkce", "null").unwrap();
        assert_ne!(first, call("oauth.generate_pkce", "null").unwrap());
    }

    /// Operands and unknown operations are refused, not ignored.
    #[test]
    fn generate_pkce_refuses_operands_and_unknown_operations() {
        assert!(call("oauth.generate_pkce", "[1]").is_err());
        assert_eq!(
            call("oauth.other", "null"),
            Err("Unknown native helper: oauth.other".into())
        );
    }
}
