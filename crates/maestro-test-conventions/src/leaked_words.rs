//! Library and product words of other projects in Rust identifiers and tracked names.
//!
//! Every Maestro name says what the code does. The check reads each Rust identifier
//! (never a string literal or a comment) and each file and directory name that version
//! control tracks under `crates/` and `docs/`. A name splits into segments at `_`, `-`,
//! any other non-alphanumeric character and case changes; a denied word matches when its
//! segments appear next to each other in the name, ignoring case. `DENIED` is the one
//! list to edit when a word joins it, and `EXEMPT` holds the names a user, a provider or a
//! stored file carries.

use crate::source::Member;
use crate::tracked_files;
use ra_ap_rustc_lexer::TokenKind;
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

/// Words that belong to a library or product Maestro was written from.
///
/// A word is one or more segments joined by `_`. Edit this list to deny another word.
const DENIED: &[&str] = &["ssf", "docx_preview", "minimatch", "plugin"];

/// A name that may keep a denied word because a user, a provider or a stored file carries it.
struct Exemption {
    /// Workspace-relative path of the file or directory, or of the source file holding the identifier.
    path: &'static str,
    /// The denied word the name may keep.
    word: &'static str,
    /// Who carries the name and where it is stored.
    reason: &'static str,
}

/// Exempt names, each with its reason. A row that matches nothing fails the check.
const EXEMPT: &[Exemption] = &[];

/// Directories whose tracked file and directory names are checked.
const NAME_ROOTS: &[&str] = &["crates/", "docs/"];

/// One name that holds a denied word.
struct Hit {
    /// Where the name is: an absolute source path, or a workspace-relative tracked path.
    location: String,
    /// Workspace-relative path used to match an exemption.
    relative: String,
    /// The line of an identifier; absent for a file or directory name.
    line: Option<usize>,
    /// The identifier or the file or directory name.
    name: String,
    /// The denied word it holds.
    word: &'static str,
}

/// Reject identifiers and tracked file and directory names that hold a denied word.
pub(crate) fn check(root: &Path, members: &[Member]) -> Result<(), String> {
    check_with(root, members, EXEMPT)
}

/// Run the check against a given exemption table.
fn check_with(root: &Path, members: &[Member], exempt: &[Exemption]) -> Result<(), String> {
    let mut hits = identifier_hits(root, members);
    hits.extend(name_hits(root)?);
    let mut used = vec![false; exempt.len()];
    hits.retain(|hit| {
        let row = exempt
            .iter()
            .position(|row| row.path == hit.relative && row.word == hit.word);
        if let Some(index) = row {
            used[index] = true;
        }
        row.is_none()
    });
    let mut diagnostics: Vec<String> = hits.iter().map(describe).collect();
    diagnostics.sort();
    diagnostics.extend(
        exempt
            .iter()
            .zip(&used)
            .filter(|(_, used)| !**used)
            .map(|(row, _)| {
                format!(
                    "exemption for `{}` and `{}` ({}) matches no name; remove the row from EXEMPT",
                    row.path, row.word, row.reason
                )
            }),
    );
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics.join("\n"))
    }
}

/// The diagnostic for one hit.
fn describe(hit: &Hit) -> String {
    let place = hit.line.map_or_else(
        || hit.location.clone(),
        |line| format!("{}:{line}", hit.location),
    );
    let kind = if hit.line.is_some() {
        "identifier"
    } else {
        "file or directory name"
    };
    format!(
        "{place}: {kind} `{}` holds the word `{}`; name it for what it does (words: DENIED in leaked_words.rs)",
        hit.name, hit.word
    )
}

/// Every identifier in the members' sources that holds a denied word.
fn identifier_hits(root: &Path, members: &[Member]) -> Vec<Hit> {
    let mut hits = Vec::new();
    for source in members.iter().flat_map(|member| &member.sources) {
        let relative = relative_path(root, &source.path);
        let mut known: HashMap<&str, Vec<&'static str>> = HashMap::new();
        for (kind, range) in &source.tokens {
            if !matches!(kind, TokenKind::Ident | TokenKind::RawIdent) {
                continue;
            }
            let name = source.contents[range.clone()]
                .strip_prefix("r#")
                .unwrap_or(&source.contents[range.clone()]);
            let words = known.entry(name).or_insert_with(|| denied_words(name));
            for word in words.iter() {
                hits.push(Hit {
                    location: source.path.display().to_string(),
                    relative: relative.clone(),
                    line: Some(source.contents[..range.start].matches('\n').count() + 1),
                    name: name.to_owned(),
                    word,
                });
            }
        }
    }
    hits
}

/// A path relative to the workspace root, with forward slashes.
fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Every tracked file and directory name under the name roots that holds a denied word.
fn name_hits(root: &Path) -> Result<Vec<Hit>, String> {
    let mut seen = BTreeSet::new();
    let mut hits = Vec::new();
    for path in tracked_files::paths(root)? {
        if !NAME_ROOTS.iter().any(|prefix| path.starts_with(prefix))
            || std::fs::symlink_metadata(root.join(&path)).is_err()
        {
            continue;
        }
        let mut prefix = String::new();
        for component in path.split('/') {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(component);
            if !seen.insert(prefix.clone()) {
                continue;
            }
            for word in denied_words(component) {
                hits.push(Hit {
                    location: prefix.clone(),
                    relative: prefix.clone(),
                    line: None,
                    name: component.to_owned(),
                    word,
                });
            }
        }
    }
    Ok(hits)
}

/// The denied words a name holds.
fn denied_words(name: &str) -> Vec<&'static str> {
    let compact: String = name
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    let mut parts = None;
    DENIED
        .iter()
        .copied()
        .filter(|word| compact.contains(&word.replace('_', "")))
        .filter(|word| {
            let parts = parts.get_or_insert_with(|| segments(name));
            let wanted = segments(word);
            parts
                .windows(wanted.len())
                .any(|window| window == wanted.as_slice())
        })
        .collect()
}

/// A name's lowercase segments, split at non-alphanumerics and case changes.
fn segments(name: &str) -> Vec<String> {
    let chars: Vec<char> = name.chars().collect();
    let mut parts = Vec::new();
    let mut current = String::new();
    for (index, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            if !current.is_empty() {
                parts.push(std::mem::take(&mut current));
            }
            continue;
        }
        let after_lower = index > 0 && chars[index - 1].is_lowercase() && c.is_uppercase();
        let ends_acronym = index > 0
            && c.is_uppercase()
            && chars[index - 1].is_uppercase()
            && chars.get(index + 1).is_some_and(|next| next.is_lowercase());
        if (after_lower || ends_acronym) && !current.is_empty() {
            parts.push(std::mem::take(&mut current));
        }
        current.extend(c.to_lowercase());
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

#[cfg(test)]
mod tests;
