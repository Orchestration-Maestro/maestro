//! Recursive attachment candidates behind the provider's host seam.
use super::paths::{expand_home, to_display_path};
use super::{
    AutocompleteItem, AutocompleteOperations, AutocompleteSuggestions, CombinedAutocompleteProvider,
};
use maestro_path::{basename, is_absolute, join};
use std::io;

/// Search and project attachment records; host failures are caught by the provider.
pub(super) async fn suggest<O: AutocompleteOperations>(
    provider: &CombinedAutocompleteProvider<O>,
    prefix: &str,
    signal: &O::Signal,
) -> io::Result<Option<AutocompleteSuggestions>> {
    let Some(executable) = provider.fd_path.as_deref().filter(|path| !path.is_empty()) else {
        return Ok(None);
    };
    if provider.operations.is_aborted(signal) {
        return Ok(None);
    }
    let raw = &prefix[1..];
    let quoted = raw.starts_with('"');
    let raw = to_display_path(raw.strip_prefix('"').unwrap_or(raw));
    let scope = resolve_scope(provider, &raw)?;
    let base = scope
        .as_ref()
        .map_or(provider.base_path.as_str(), |scope| scope.base.as_str());
    let display = scope.as_ref().map_or("", |scope| scope.display.as_str());
    let query = scope.as_ref().map_or_else(
        || std::borrow::Cow::Owned(normalize_query(&raw)),
        |scope| std::borrow::Cow::Borrowed(scope.query.as_str()),
    );
    let args = arguments(base, &query);
    let bytes = provider
        .operations
        .run_fd(executable, &args, signal)
        .await?;
    if provider.operations.is_aborted(signal) {
        return Ok(None);
    }
    let text = String::from_utf8_lossy(&bytes);
    let mut ranked: Vec<_> = text
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(to_display_path)
        .filter(|path| !path.split('/').any(|component| component == ".git"))
        .map(|path| (score(&path, &query), path))
        .filter(|(score, _)| *score > 0)
        .collect();
    ranked.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    let items = ranked
        .into_iter()
        .take(20)
        .map(|(_, path)| project(&path, display, quoted))
        .collect();
    Ok(super::commands::nonempty(items, prefix))
}
/// Add literal basename or escaped full-path operands only for nonempty queries.
fn arguments(base: &str, query: &str) -> Vec<String> {
    let mut args = [
        "--print0",
        "--strip-cwd-prefix=always",
        "--ignore-case",
        "--base-directory",
        base,
        "--max-results",
        "100",
        "--type",
        "f",
        "--type",
        "d",
        "--follow",
        "--hidden",
        "--exclude",
        ".git",
        "--exclude",
        ".git/*",
        "--exclude",
        ".git/**",
    ]
    .map(str::to_owned)
    .to_vec();
    if !query.is_empty() {
        let (mode, pattern) = if query.contains('/') {
            ("--full-path", path_pattern(query))
        } else {
            ("--fixed-strings", query.to_owned())
        };
        args.extend([mode.into(), "--".into(), pattern]);
    }
    args
}

/// A reached directory scope and its native-separator-converted display prefix.
struct Scope {
    /// Native directory operand.
    base: String,
    /// Basename query.
    query: String,
    /// Display directory after native separator conversion.
    display: String,
}
/// Resolve only the directory before the final display separator.
fn resolve_scope<O: AutocompleteOperations>(
    provider: &CombinedAutocompleteProvider<O>,
    raw: &str,
) -> io::Result<Option<Scope>> {
    let Some(offset) = raw.rfind('/') else {
        return Ok(None);
    };
    let display = &raw[..=offset];
    let base = if display.starts_with("~/") {
        expand_home(&provider.operations, display)?
    } else if is_absolute(display) {
        display.to_owned()
    } else {
        join(&[&provider.base_path, display])
    };
    if !provider.operations.is_directory(&base).unwrap_or(false) {
        return Ok(None);
    }
    Ok(Some(Scope {
        base,
        query: raw[offset + 1..].into(),
        display: display.into(),
    }))
}
/// Escape literal path segments for the search executable's expression dialect.
fn escape_regex(segment: &str) -> String {
    let mut pattern = String::new();
    for ch in segment.chars() {
        if r"\^$.*+?()|[]{}".contains(ch) {
            pattern.push('\\');
        }
        pattern.push(ch);
    }
    pattern
}

/// Collapse repeated display separators without losing leading or trailing ones.
fn normalize_query(raw: &str) -> String {
    let middle = raw
        .split('/')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("/");
    if middle.is_empty() {
        return if raw.is_empty() {
            String::new()
        } else {
            "/".into()
        };
    }
    format!(
        "{}{middle}{}",
        if raw.starts_with('/') { "/" } else { "" },
        if raw.ends_with('/') { "/" } else { "" }
    )
}
/// Full-path pattern whose segments remain literal.
fn path_pattern(query: &str) -> String {
    let trimmed = query.trim_matches('/');
    if trimmed.is_empty() {
        return query.to_owned();
    }
    let separator = if cfg!(windows) { r"[\\/]" } else { "/" };
    let mut pattern = trimmed
        .split('/')
        .map(escape_regex)
        .collect::<Vec<_>>()
        .join(separator);
    if query.ends_with('/') {
        pattern.push_str(separator);
    }
    pattern
}

/// Higher scores favor basename matches, with a bonus for matching directories.
fn score(path: &str, query: &str) -> u16 {
    if query.is_empty() {
        return 1;
    }
    let name = basename(path, None).to_lowercase();
    let query = query.to_lowercase();
    let score = if name == query {
        100
    } else if name.starts_with(&query) {
        80
    } else if name.contains(&query) {
        50
    } else if path.to_lowercase().contains(&query) {
        30
    } else {
        0
    };
    score
        + if score > 0 && path.ends_with('/') {
            10
        } else {
            0
        }
}
/// Prepend the captured display directory and quote the selected path when needed.
fn project(path: &str, display: &str, quoted: bool) -> AutocompleteItem {
    let directory = path.ends_with('/');
    let path = path.strip_suffix('/').unwrap_or(path);
    let projected = format!("{display}{path}");
    let suffix = if directory { "/" } else { "" };
    let completion = format!("{projected}{suffix}");
    let value = if quoted || completion.contains(' ') {
        format!("@\"{completion}\"")
    } else {
        format!("@{completion}")
    };
    AutocompleteItem {
        value,
        label: format!("{}{suffix}", basename(path, None)),
        description: Some(projected),
    }
}
