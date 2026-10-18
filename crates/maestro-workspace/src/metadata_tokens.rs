//! Explicitly expired, per-key metadata values.
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

/// A value and its optional expiry.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MetadataToken {
    /// Supplied text, including empty text.
    value: String,
    /// Deadline applied by an explicit expiry sweep.
    expires_at: Option<Instant>,
}

/// Metadata whose reads do not advance expiry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MetadataTokens {
    /// Current keys and their values.
    entries: HashMap<String, MetadataToken>,
}
impl MetadataTokens {
    /// Apply replacements and deletions, reporting whether stored state changed.
    pub fn patch(
        &mut self,
        patch: HashMap<String, Option<String>>,
        ttl: Option<Duration>,
        now: Instant,
    ) -> bool {
        let expires_at = ttl.and_then(|ttl| now.checked_add(ttl));
        let mut changed = false;
        for (key, value) in patch {
            let Some(value) = value else {
                changed |= self.entries.remove(&key).is_some();
                continue;
            };
            let token = MetadataToken { value, expires_at };
            if self.entries.get(&key) != Some(&token) {
                self.entries.insert(key, token);
                changed = true;
            }
        }
        changed
    }

    /// Copy the currently stored values without expiring them.
    #[must_use]
    pub fn values(&self) -> HashMap<String, String> {
        self.entries
            .iter()
            .map(|(key, token)| (key.clone(), token.value.clone()))
            .collect()
    }

    /// Remove every token whose deadline has arrived.
    pub fn expire_at(&mut self, now: Instant) -> bool {
        let before = self.entries.len();
        self.entries
            .retain(|_, token| token.expires_at.is_none_or(|deadline| deadline > now));
        self.entries.len() != before
    }

    /// Predict the key count without changing values or deadlines.
    #[must_use]
    pub fn key_count_after_patch(&self, patch: &HashMap<String, Option<String>>) -> usize {
        let inserted = patch
            .iter()
            .filter(|(key, value)| value.is_some() && !self.entries.contains_key(*key))
            .count();
        let removed = patch
            .iter()
            .filter(|(key, value)| value.is_none() && self.entries.contains_key(*key))
            .count();
        self.entries.len() + inserted - removed
    }

    /// Return the earliest stored deadline.
    #[must_use]
    pub fn next_expiry(&self) -> Option<Instant> {
        self.entries
            .values()
            .filter_map(|token| token.expires_at)
            .min()
    }
}

/// Maximum distinct sequenced reporters per workspace.
pub const MAX_SEQUENCE_SOURCES: usize = 32;

/// A new sequenced reporter exceeded the source capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SequenceCapacityError;

impl std::fmt::Display for SequenceCapacityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("metadata sequence source capacity reached")
    }
}
impl std::error::Error for SequenceCapacityError {}

/// Check freshness without admitting a source.
#[must_use]
pub fn sequence_is_fresh<S: std::hash::BuildHasher>(
    sequences: &HashMap<String, u64, S>,
    source: &str,
    seq: Option<u64>,
) -> bool {
    seq.is_none_or(|seq| sequences.get(source).is_none_or(|last| seq > *last))
}

/// Admit a fresh sequence, or reject stale reports without changing state.
///
/// # Errors
/// Returns an error when a new sequenced source exceeds capacity.
pub fn accept_sequence<S: std::hash::BuildHasher>(
    sequences: &mut HashMap<String, u64, S>,
    source: &str,
    seq: Option<u64>,
) -> Result<bool, SequenceCapacityError> {
    let Some(seq) = seq else {
        return Ok(true);
    };
    if !sequence_is_fresh(sequences, source, Some(seq)) {
        return Ok(false);
    }
    if !sequences.contains_key(source) && sequences.len() >= MAX_SEQUENCE_SOURCES {
        return Err(SequenceCapacityError);
    }
    sequences.insert(source.to_owned(), seq);
    Ok(true)
}
