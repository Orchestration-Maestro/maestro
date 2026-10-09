//! Direct path recognition, candidate construction and insertion.

use super::{
    AutocompleteItem, AutocompleteOperations, AutocompleteSuggestions, CompletionResult,
    CursorPosition, DirectoryEntryKind,
};
use crate::text::utils::is_whitespace_scalar;
use maestro_path::{basename, dirname, join};
use std::io;

/// The ASCII delimiters of a path token.
fn delimiter(character: char) -> bool {
    " \t\"'=".contains(character)
}

/// Final token following the last delimiter.
fn final_token(text: &str) -> &str {
    text.rsplit(delimiter).next().unwrap_or_default()
}

/// An unclosed quote is recognized only at a token boundary.
fn quoted_prefix(text: &str) -> Option<&str> {
    let mut open = None;
    for (offset, character) in text.char_indices() {
        if character == '"' {
            open = if open.is_some() { None } else { Some(offset) };
        }
    }
    let offset = open?;
    let start = if text[..offset].ends_with('@') {
        offset - 1
    } else {
        offset
    };
    if text[..start].chars().next_back().is_none_or(delimiter) {
        Some(&text[start..])
    } else {
        None
    }
}

/// Attachment routing precedes commands and direct paths.
pub(super) fn attachment_prefix(text: &str) -> bool {
    quoted_prefix(text).is_some_and(|prefix| prefix.starts_with("@\""))
        || final_token(text).starts_with('@')
}

/// Select a quoted, natural path-like or explicitly forced prefix.
pub(super) fn extract_prefix(text: &str, force: bool) -> Option<&str> {
    if let Some(quoted) = quoted_prefix(text) {
        return Some(quoted);
    }
    let token = final_token(text);
    (force
        || token.contains('/')
        || token.starts_with('.')
        || (token.is_empty() && text.ends_with(' ')))
    .then_some(token)
}

/// Separate display quoting from the authored path operand.
fn raw_prefix(prefix: &str) -> (&str, bool) {
    prefix
        .strip_prefix('"')
        .map_or((prefix, false), |raw| (raw, true))
}

/// Expand only the two supported home forms.
fn expand_home<O: AutocompleteOperations>(operations: &O, raw: &str) -> io::Result<String> {
    if raw == "~" {
        return operations.home_dir();
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        let mut expanded = join(&[&operations.home_dir()?, rest]);
        if raw.ends_with('/') && !expanded.ends_with('/') {
            expanded.push('/');
        }
        return Ok(expanded);
    }
    Ok(raw.to_owned())
}

/// Choose the directory operand and entry-name prefix.
fn search_location(base: &str, raw: &str, expanded: &str) -> (String, String) {
    let contents = matches!(raw, "" | "./" | "../" | "~" | "~/" | "/") || raw.ends_with('/');
    let (directory, name) = if contents {
        (expanded.to_owned(), String::new())
    } else {
        (dirname(expanded), basename(expanded, None))
    };
    let directory = if raw.starts_with('~') || expanded.starts_with('/') {
        directory
    } else {
        join(&[base, &directory])
    };
    (directory, name)
}

/// Construct the authored display spelling for one matching entry.
fn display_path(raw: &str, name: &str) -> String {
    let path = if raw.ends_with('/') {
        format!("{raw}{name}")
    } else if raw.contains(['/', '\\']) {
        if let Some(relative) = raw.strip_prefix("~/") {
            let dir = dirname(relative);
            format!(
                "~/{}",
                if dir == "." {
                    name.to_owned()
                } else {
                    join(&[&dir, name])
                }
            )
        } else if raw.starts_with('/') {
            let dir = dirname(raw);
            format!("{}{name}", if dir == "/" { dir } else { format!("{dir}/") })
        } else {
            let path = join(&[&dirname(raw), name]);
            if raw.starts_with("./") && !path.starts_with("./") {
                format!("./{path}")
            } else {
                path
            }
        }
    } else if raw.starts_with('~') {
        format!("~/{name}")
    } else {
        name.to_owned()
    };
    if cfg!(windows) {
        path.replace('\\', "/")
    } else {
        path
    }
}

/// Retained directory classification before display quoting.
struct Candidate {
    /// Public display and insertion values.
    item: AutocompleteItem,
    /// Resolved directory identity used for sorting.
    directory: bool,
}

/// Complete direct paths; all host failures except link metadata discard the request.
pub(super) fn suggest<O: AutocompleteOperations>(
    operations: &O,
    base: &str,
    prefix: &str,
) -> io::Result<Option<AutocompleteSuggestions>> {
    let (raw, quoted) = raw_prefix(prefix);
    let expanded = expand_home(operations, raw)?;
    let (directory, search) = search_location(base, raw, &expanded);
    let search = search.to_lowercase();
    let mut candidates = Vec::new();
    for entry in operations.read_dir(&directory)? {
        if !entry.name.to_lowercase().starts_with(&search) {
            continue;
        }
        let is_directory = match entry.kind {
            DirectoryEntryKind::Directory => true,
            DirectoryEntryKind::SymbolicLink => operations
                .is_directory(&join(&[&directory, &entry.name]))
                .unwrap_or(false),
            DirectoryEntryKind::Other => false,
        };
        let suffix = if is_directory { "/" } else { "" };
        let path = format!("{}{suffix}", display_path(raw, &entry.name));
        let value = if quoted || path.contains(' ') {
            format!("\"{path}\"")
        } else {
            path
        };
        candidates.push(Candidate {
            item: AutocompleteItem {
                value,
                label: format!("{}{suffix}", entry.name),
                description: None,
            },
            directory: is_directory,
        });
    }
    let mut failure = None;
    candidates.sort_by(|left, right| {
        right.directory.cmp(&left.directory).then_with(|| {
            match operations.compare(&left.item.label, &right.item.label) {
                Ok(order) => order,
                Err(error) => {
                    failure = Some(error);
                    std::cmp::Ordering::Equal
                }
            }
        })
    });
    if let Some(error) = failure {
        return Err(error);
    }
    Ok(super::commands::nonempty(
        candidates
            .into_iter()
            .map(|candidate| candidate.item)
            .collect(),
        prefix,
    ))
}

/// Replace the prefix and return a byte cursor.
///
/// Drop one leading suffix quote when the prefix is quoted and the candidate ends in a quote.
pub(super) fn apply(
    lines: &[String],
    cursor: CursorPosition,
    item: &AutocompleteItem,
    prefix: &str,
) -> CompletionResult {
    let line = lines.get(cursor.line).map_or("", String::as_str);
    let before = &line[..cursor.col - prefix.len()];
    let after = &line[cursor.col..];
    let quoted = prefix.starts_with('"') || prefix.starts_with("@\"");
    let after = if quoted && item.value.ends_with('"') {
        after.strip_prefix('"').unwrap_or(after)
    } else {
        after
    };
    let command = prefix.starts_with('/')
        && before.trim_matches(is_whitespace_scalar).is_empty()
        && !prefix[1..].contains('/')
        && !raw_prefix(&item.value).0.starts_with('/');
    let directory = item.label.ends_with('/');
    let (value, offset) = if command {
        (format!("/{} ", item.value), item.value.len() + 2)
    } else {
        let suffix = if prefix.starts_with('@') && !directory {
            " "
        } else {
            ""
        };
        let offset =
            item.value.len() - usize::from(directory && item.value.ends_with('"')) + suffix.len();
        (format!("{}{suffix}", item.value), offset)
    };
    let mut updated = lines.to_vec();
    if cursor.line >= updated.len() {
        updated.resize(cursor.line + 1, String::new());
    }
    updated[cursor.line] = format!("{before}{value}{after}");
    CompletionResult {
        lines: updated,
        cursor_line: cursor.line,
        cursor_col: before.len() + offset,
    }
}
