//! The prompt-cache key that OpenAI-style requests derive from a session identifier.

/// Longest cache key, in Unicode scalar values, that the endpoints accept.
const MAX_KEY_CHARACTERS: usize = 64;

/// Cut `session` to the endpoint limit without splitting a character.
pub(crate) fn prompt_cache_key(session: &str) -> &str {
    session
        .char_indices()
        .nth(MAX_KEY_CHARACTERS)
        .map_or(session, |(end, _)| session.split_at(end).0)
}
