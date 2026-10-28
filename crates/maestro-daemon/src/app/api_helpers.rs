//! Normalization of the metadata a caller reports for a workspace.

use std::{collections::HashMap, time::Duration};

/// Trims a reporter's agent label and gives a recognized agent its catalogue spelling.
///
/// Returns `None` for a blank label; any other label stays as a custom identity.
pub(super) fn normalize_reported_agent_label(agent: &str) -> Option<String> {
    let trimmed = agent.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(
        maestro_detection::agents::parse_agent_label(trimmed).map_or_else(
            || trimmed.to_owned(),
            |known| maestro_detection::agents::agent_label(known).to_owned(),
        ),
    )
}

/// Longest accepted metadata source, in Unicode scalars.
const METADATA_SOURCE_MAX_CHARS: usize = 80;
/// Shortest accepted token lifetime, in milliseconds.
const METADATA_TTL_MIN_MS: u64 = 1;
/// Longest accepted token lifetime, in milliseconds: one day.
const METADATA_TTL_MAX_MS: u64 = 86_400_000;
/// Most tokens one report may set or clear.
const MAX_METADATA_TOKEN_KEYS_PER_REQUEST: usize = 16;
/// Most tokens one workspace may hold.
pub(super) const MAX_METADATA_TOKEN_KEYS_PER_RESOURCE: usize = 32;
/// Longest accepted token key, in bytes.
const MAX_METADATA_TOKEN_KEY_LEN: usize = 32;
/// Longest kept token value, in Unicode scalars.
const MAX_METADATA_TOKEN_VALUE_LEN: usize = 80;

/// Trims a reporter's source name and checks its length and ASCII grammar.
///
/// # Errors
/// Returns the reason an empty, overlong or malformed source is refused.
pub(super) fn normalize_metadata_source(value: &str) -> Result<String, &'static str> {
    let value = value.trim();
    if value.is_empty() {
        return Err("metadata source must not be empty");
    }
    if value.chars().count() > METADATA_SOURCE_MAX_CHARS {
        return Err("metadata source must be 80 characters or fewer");
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ':' | '.' | '_' | '-'))
    {
        return Err(
            "metadata source may contain only ASCII letters, digits, colon, dot, underscore, and hyphen",
        );
    }
    Ok(value.to_owned())
}

/// Converts an optional token lifetime, checking it lies between 1 ms and one day.
///
/// # Errors
/// Returns the reason a lifetime outside those bounds is refused.
pub(super) fn normalize_metadata_ttl(
    ttl_ms: Option<u64>,
) -> Result<Option<Duration>, &'static str> {
    let Some(ttl_ms) = ttl_ms else {
        return Ok(None);
    };
    if ttl_ms < METADATA_TTL_MIN_MS {
        return Err("metadata ttl_ms must be at least 1");
    }
    if ttl_ms > METADATA_TTL_MAX_MS {
        return Err("metadata ttl_ms must be 86400000 or less");
    }
    Ok(Some(Duration::from_millis(ttl_ms)))
}

/// Checks a token patch and cleans its values: trimmed, without control characters, at
/// most 80 scalars; a missing, blank or control-only value clears its token.
///
/// # Errors
/// Returns the reason an empty or oversized patch, or a malformed key, is refused.
pub(super) fn normalize_metadata_tokens(
    tokens: HashMap<String, Option<String>>,
) -> Result<HashMap<String, Option<String>>, String> {
    if tokens.is_empty() {
        return Err("missing token to set or clear".into());
    }
    if tokens.len() > MAX_METADATA_TOKEN_KEYS_PER_REQUEST {
        return Err(format!(
            "a metadata report may update at most {MAX_METADATA_TOKEN_KEYS_PER_REQUEST} tokens"
        ));
    }
    tokens
        .into_iter()
        .map(|(key, value)| {
            if key.is_empty()
                || key.len() > MAX_METADATA_TOKEN_KEY_LEN
                || !key
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
            {
                return Err(format!("invalid metadata token key: {key}"));
            }
            let value = value.and_then(|value| {
                let normalized = value
                    .trim()
                    .chars()
                    .filter(|ch| !ch.is_control())
                    .take(MAX_METADATA_TOKEN_VALUE_LEN)
                    .collect::<String>();
                let normalized = normalized.trim();
                (!normalized.is_empty()).then(|| normalized.to_owned())
            });
            Ok((key, value))
        })
        .collect()
}
