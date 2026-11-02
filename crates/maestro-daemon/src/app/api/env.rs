//! Validation of the extra environment a workspace or tab launches its root pane with.

use super::responses::api_error;
use maestro_protocol::schema::ErrorBody;
use std::collections::HashMap;

/// Checks every pair a caller asked the root pane to receive and orders them by key.
///
/// # Errors
/// Returns `invalid_env` for an empty key, a key holding `=` or NUL, or a value holding
/// NUL; among several invalid pairs, which one is reported is unspecified.
pub(super) fn normalize_launch_env(
    env: HashMap<String, String>,
) -> Result<Vec<(String, String)>, ErrorBody> {
    let mut normalized = Vec::with_capacity(env.len());
    for (key, value) in env {
        if key.is_empty() {
            return Err(api_error("invalid_env", "env key must not be empty"));
        }
        if key.contains('=') {
            return Err(api_error(
                "invalid_env",
                format!("env key {key} must not contain '='"),
            ));
        }
        if key.contains('\0') {
            return Err(api_error(
                "invalid_env",
                "env key must not contain NUL bytes",
            ));
        }
        if value.contains('\0') {
            return Err(api_error(
                "invalid_env",
                format!("env value for {key} must not contain NUL bytes"),
            ));
        }
        normalized.push((key, value));
    }
    normalized.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(normalized)
}

#[cfg(test)]
mod tests;
