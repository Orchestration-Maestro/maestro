//! Request-local tool-call associations.

use std::collections::{HashMap, HashSet};

/// Forward mappings and occupied candidate identifiers.
#[derive(Default)]
pub(in crate::providers::reasoning::mistral) struct ToolCallIds {
    /// Original identifiers mapped to candidates.
    forward: HashMap<String, String>,
    /// Candidate identifiers already assigned in this request.
    occupied: HashSet<String>,
}

impl ToolCallIds {
    /// Reuse an association or choose the first unoccupied candidate.
    pub(in crate::providers::reasoning::mistral) fn normalize(&mut self, id: &str) -> String {
        if let Some(existing) = self.forward.get(id) {
            return existing.clone();
        }
        let mut attempt = 0_u64;
        loop {
            let candidate = derive(id, attempt);
            if self.occupied.insert(candidate.clone()) {
                self.forward.insert(id.to_owned(), candidate.clone());
                return candidate;
            }
            attempt = attempt.wrapping_add(1);
        }
    }
}

/// Preserve a nine-character ASCII identifier or hash a collision seed.
pub(in crate::providers::reasoning::mistral) fn derive(id: &str, attempt: u64) -> String {
    let filtered: String = id.chars().filter(char::is_ascii_alphanumeric).collect();
    if attempt == 0 && filtered.len() == 9 {
        return filtered;
    }
    let base = if filtered.is_empty() { id } else { &filtered };
    let collision_seed;
    let seed = if attempt == 0 {
        base
    } else {
        collision_seed = format!("{base}:{attempt}");
        &collision_seed
    };
    crate::short_hash(seed)
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(9)
        .collect()
}
