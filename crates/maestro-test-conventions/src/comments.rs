use serde_json::Value;

/// Numbered planning nouns; technical uses without a number remain valid.
const NUMBERED: &[&str] = &["slice", "spec", "task", "ticket", "issue", "pr"];

pub(super) fn check(metadata: &Value) -> Result<(), String> {
    let members = super::array(metadata, "workspace_members")?;
    for package in super::array(metadata, "packages")? {
        if members.contains(&package["id"]) {
            let manifest = std::path::Path::new(super::string(package, "manifest_path")?);
            for path in crate::source::files(manifest.parent().ok_or("manifest has no parent")?)? {
                check_file(&path)?;
            }
        }
    }
    Ok(())
}

fn check_file(path: &std::path::Path) -> Result<(), String> {
    let source =
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    for (start, comment) in crate::source::comments(&source) {
        if let Some(offset) = forbidden(comment) {
            let line = crate::source::line(&source, start + offset);
            return Err(format!(
                "{}:{line}: planning reference in comment",
                path.display()
            ));
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
