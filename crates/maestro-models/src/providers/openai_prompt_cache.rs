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

#[cfg(test)]
mod tests {
    use super::prompt_cache_key;

    /// A four-byte character that straddles the limit is cut whole, never split.
    #[test]
    fn four_byte_characters_at_the_limit_are_never_split() {
        let sixty_three = "a".repeat(63);
        let session = format!("{sixty_three}😀😀");
        assert_eq!(prompt_cache_key(&session), format!("{sixty_three}😀"));
        let exact = format!("{sixty_three}😀");
        assert_eq!(prompt_cache_key(&exact), exact);
    }
}
