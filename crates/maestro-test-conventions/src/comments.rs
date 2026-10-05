use std::path::Path;

use serde_json::Value;

/// Numbered planning nouns; technical uses without a number remain valid.
const NUMBERED: &[&str] = &["slice", "spec", "task", "ticket", "issue", "pr"];

pub(super) fn check(metadata: &Value) -> Result<(), String> {
    let members = super::array(metadata, "workspace_members")?;
    for package in super::array(metadata, "packages")? {
        if members.contains(&package["id"]) {
            let manifest = Path::new(super::string(package, "manifest_path")?);
            scan_directory(manifest.parent().ok_or("manifest has no parent")?)?;
        }
    }
    Ok(())
}

fn scan_directory(path: &Path) -> Result<(), String> {
    for entry in std::fs::read_dir(path).map_err(|error| format!("{}: {error}", path.display()))? {
        let entry = entry.map_err(|error| format!("{}: {error}", path.display()))?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if kind.is_dir()
            && path
                .file_name()
                .is_none_or(|name| name != "target" && name != ".git")
        {
            scan_directory(&path)?;
        } else if kind.is_file() && path.extension().is_some_and(|extension| extension == "rs") {
            let source = std::fs::read_to_string(&path)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            for (start, comment) in comments(&source) {
                if let Some(offset) = forbidden(comment) {
                    let line = source[..start + offset]
                        .bytes()
                        .filter(|byte| *byte == b'\n')
                        .count()
                        + 1;
                    return Err(format!(
                        "{}:{line}: planning reference in comment",
                        path.display()
                    ));
                }
            }
        }
    }
    Ok(())
}

// Forbidden pattern families (the complete list, without exemptions):
// - NUMBERED nouns followed by an ASCII decimal number, ignoring case.
// - A hash directly followed by an ASCII digit, anywhere in a comment.
// - "pull request" followed by a decimal number, ignoring case.
// - Whole planning delivery/iteration nouns and narrative phrase, ignoring case.
// - Uppercase US, D, F or T followed by digits; FR-S followed by digits,
//   a hyphen and more digits. Identifiers must occupy a whole token.
// Reword false positives to describe the code instead.
fn forbidden(comment: &str) -> Option<usize> {
    let bytes = comment.as_bytes();
    if let Some(offset) = bytes
        .windows(2)
        .position(|pair| pair[0] == b'#' && pair[1].is_ascii_digit())
    {
        return Some(offset);
    }
    let mut words = Vec::new();
    let mut start = 0;
    while start < bytes.len() {
        if !bytes[start].is_ascii_alphanumeric() {
            start += 1;
            continue;
        }
        let mut end = start + 1;
        while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'-') {
            end += 1;
        }
        words.push((start, &comment[start..end]));
        start = end;
    }
    for (index, &(offset, word)) in words.iter().enumerate() {
        let lower = word.to_ascii_lowercase();
        let next = words
            .get(index + 1)
            .map(|entry| entry.1.to_ascii_lowercase());
        let after = words.get(index + 2).map(|entry| entry.1);
        if NUMBERED.contains(&lower.as_str()) && next.as_deref().is_some_and(digits)
            || lower == "pull" && next.as_deref() == Some("request") && after.is_some_and(digits)
            || matches!(lower.as_str(), "milestone" | "sprint")
            || lower == "user" && next.as_deref() == Some("story")
            || planning_id(word)
        {
            return Some(offset);
        }
    }
    None
}

fn digits(word: &str) -> bool {
    !word.is_empty() && word.bytes().all(|byte| byte.is_ascii_digit())
}

fn planning_id(word: &str) -> bool {
    for prefix in ["US", "D", "F", "T"] {
        if word.strip_prefix(prefix).is_some_and(digits) {
            return true;
        }
    }
    if let Some(rest) = word.strip_prefix("FR-S")
        && let Some((stage, number)) = rest.split_once('-')
    {
        return digits(stage) && digits(number);
    }
    false
}

// Lex strings and character literals before recognizing comment delimiters.
// Nested block comments are included; raw strings may contain arbitrary hashes.
fn comments(source: &str) -> Vec<(usize, &str)> {
    let bytes = source.as_bytes();
    let mut result = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index..].starts_with(b"//") {
            let start = index + 2;
            index = start;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            result.push((start, &source[start..index]));
        } else if bytes[index..].starts_with(b"/*") {
            let start = index + 2;
            index = start;
            let mut depth = 1;
            while index < bytes.len() && depth > 0 {
                if bytes[index..].starts_with(b"/*") {
                    depth += 1;
                    index += 2;
                } else if bytes[index..].starts_with(b"*/") {
                    depth -= 1;
                    index += 2;
                } else {
                    index += 1;
                }
            }
            let end = if depth == 0 { index - 2 } else { index };
            result.push((start, &source[start..end]));
        } else if let Some(end) = raw_string_end(bytes, index) {
            index = end;
        } else if bytes[index] == b'"' {
            index = quoted_end(bytes, index, b'"').unwrap_or(bytes.len());
        } else if bytes[index] == b'\'' {
            // Lifetimes have no closing quote; only consume a valid character shape.
            let tail = &source[index + 1..];
            let length = if tail.starts_with('\\') {
                quoted_end(bytes, index, b'\'').map(|end| end - index)
            } else {
                tail.chars()
                    .next()
                    .map(|character| 2 + character.len_utf8())
            };
            if let Some(length) = length
                && bytes.get(index + length - 1) == Some(&b'\'')
            {
                index += length;
            } else {
                index += 1;
            }
        } else {
            // Advancing bytes is safe here: slicing only follows ASCII delimiters.
            index += 1;
        }
    }
    result
}

fn quoted_end(bytes: &[u8], start: usize, quote: u8) -> Option<usize> {
    let mut index = start + 1;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index += 2;
        } else if bytes[index] == quote {
            return Some(index + 1);
        } else {
            index += 1;
        }
    }
    None
}

fn raw_string_end(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes[start] != b'r' {
        return None;
    }
    let mut quote = start + 1;
    while bytes.get(quote) == Some(&b'#') {
        quote += 1;
    }
    if bytes.get(quote) != Some(&b'"') {
        return None;
    }
    let hashes = quote - start - 1;
    let mut index = quote + 1;
    while index < bytes.len() {
        if bytes[index] == b'"'
            && bytes
                .get(index + 1..index + 1 + hashes)
                .is_some_and(|suffix| suffix.iter().all(|byte| *byte == b'#'))
        {
            return Some(index + 1 + hashes);
        }
        index += 1;
    }
    Some(bytes.len())
}
