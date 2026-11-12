//! Helpers that fail a test with a readable message.

use std::io;

use serde::de::DeserializeOwned;

/// Parses recorded JSON, failing the test with the parse error when it is malformed.
pub fn parse<T: DeserializeOwned + Default>(text: &str) -> T {
    let parsed = serde_json::from_str(text);
    assert!(
        parsed.is_ok(),
        "invalid recorded data: {:?}",
        parsed.as_ref().err()
    );
    parsed.unwrap_or_default()
}

/// Fails the test with the error of a failed effect.
pub fn succeeds(result: io::Result<()>) {
    assert_eq!(result.map_err(|error| error.to_string()), Ok(()));
}
