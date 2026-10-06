use super::semantics::{trim, whitespace};
use std::collections::HashMap;

pub(super) const APPROVED_FILE: &str = ".github/APPROVED_CONTRIBUTORS";
pub(super) const VALID_CAPABILITIES: [&str; 2] = ["issue", "pr"];

pub(super) enum Entry {
    Other(String),
    User {
        username: String,
        capability: String,
    },
}

pub(super) struct ApprovedUsers {
    pub entries: Vec<Entry>,
    pub users: HashMap<String, usize>,
}

impl ApprovedUsers {
    pub fn capability(&self, username: &str) -> Option<&str> {
        match &self.entries[*self.users.get(&username.to_lowercase())?] {
            Entry::User { capability, .. } => Some(capability),
            Entry::Other(_) => None,
        }
    }
}

pub(super) fn parse_approved_users(content: &str, diagnostics: bool) -> ApprovedUsers {
    let mut result = ApprovedUsers {
        entries: vec![],
        users: HashMap::new(),
    };
    for line in content.split('\n') {
        let trimmed = trim(line);
        let parts: Vec<_> = trimmed
            .split(whitespace)
            .filter(|s| !s.is_empty())
            .collect();
        if !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && parts.len() == 2
            && VALID_CAPABILITIES.contains(&parts[1].to_lowercase().as_str())
        {
            result
                .users
                .insert(parts[0].to_lowercase(), result.entries.len());
            result.entries.push(Entry::User {
                username: parts[0].into(),
                capability: parts[1].to_lowercase(),
            });
        } else {
            if diagnostics && !trimmed.is_empty() && !trimmed.starts_with('#') {
                if parts.len() != 2 {
                    eprintln!("Skipping malformed line: {line}");
                } else {
                    eprintln!("Skipping line with invalid capability: {line}");
                }
            }
            result.entries.push(Entry::Other(line.into()));
        }
    }
    result
}

pub(super) fn stringify_approved_users(entries: &[Entry]) -> String {
    let mut end = entries.len();
    while end > 0 && matches!(&entries[end-1], Entry::Other(line) if trim(line).is_empty()) {
        end -= 1;
    }
    let lines: Vec<_> = entries[..end]
        .iter()
        .map(|entry| match entry {
            Entry::Other(line) => line.clone(),
            Entry::User {
                username,
                capability,
            } => format!("{username} {capability}"),
        })
        .collect();
    format!("{}\n", lines.join("\n"))
}
