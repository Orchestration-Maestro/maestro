//! Changelog sections and the numeric release order they carry.

use maestro_text::{is_whitespace_scalar, lossy_string};

pub use crate::configuration::config::paths::get_changelog_path;
use std::path::Path;

/// A changelog section and the numeric version from its heading.
#[derive(Debug, Clone, PartialEq)]
pub struct ChangelogEntry {
    /// Major component.
    pub major: f64,
    /// Minor component.
    pub minor: f64,
    /// Patch component.
    pub patch: f64,
    /// The heading line followed by the section body.
    pub content: String,
}

/// Reads the changelog sections at `changelog_path`, in file order.
///
/// A path that names no file yields no sections. A file that exists but cannot be read
/// is reported to standard error and yields no sections.
#[must_use]
pub fn parse_changelog(changelog_path: &str) -> Vec<ChangelogEntry> {
    if !Path::new(changelog_path).exists() {
        return Vec::new();
    }
    let bytes = match std::fs::read(changelog_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("Warning: Could not parse changelog: {error}");
            return Vec::new();
        }
    };
    let text = lossy_string(bytes);
    let mut entries = Vec::new();
    let mut section: Option<(ChangelogEntry, String)> = None;
    for line in text.split('\n') {
        if line.starts_with("## ") {
            if let Some(flushed) = section.take() {
                entries.push(with_content(flushed));
            }
            section = parse_heading(line).map(|entry| (entry, line.to_owned()));
        } else if let Some((_, content)) = &mut section {
            content.push('\n');
            content.push_str(line);
        }
    }
    if let Some(flushed) = section {
        entries.push(with_content(flushed));
    }
    entries
}

/// Compares the first differing numeric component, retaining its difference.
///
/// The result is positive when `v1` is newer, negative when it is older and zero when
/// the numeric components match. Section content never decides the order.
#[must_use]
pub fn compare_versions(v1: &ChangelogEntry, v2: &ChangelogEntry) -> f64 {
    if !same(v1.major, v2.major) {
        return v1.major - v2.major;
    }
    if !same(v1.minor, v2.minor) {
        return v1.minor - v2.minor;
    }
    v1.patch - v2.patch
}

/// Selects the entries that are strictly newer than the saved dotted `last_version`.
///
/// The selected entries are the caller's own records, in their input order.
#[must_use]
pub fn get_new_entries<'a>(
    entries: &'a [ChangelogEntry],
    last_version: &str,
) -> Vec<&'a ChangelogEntry> {
    let mut parts = last_version.split('.').map(parse_last_component);
    let last = ChangelogEntry {
        major: parts.next().unwrap_or_default(),
        minor: parts.next().unwrap_or_default(),
        patch: parts.next().unwrap_or_default(),
        content: String::new(),
    };
    entries
        .iter()
        .filter(|entry| compare_versions(entry, &last) > 0.0)
        .collect()
}

/// Reads one saved-version component, defaulting anything unrecognized to zero.
fn parse_last_component(text: &str) -> f64 {
    let text = text.trim_matches(is_whitespace_scalar);
    match text {
        "Infinity" | "+Infinity" => f64::INFINITY,
        "-Infinity" => f64::NEG_INFINITY,
        _ if text.bytes().all(is_decimal_byte) => text.parse().unwrap_or_default(),
        _ => 0.0,
    }
}

/// Whether a byte may appear in a signed decimal component or its exponent.
fn is_decimal_byte(byte: u8) -> bool {
    byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b'e' | b'E')
}

/// Whether two numeric components order as equal; a component that is not a number never
/// matches, because the partial ordering leaves it unordered.
fn same(left: f64, right: f64) -> bool {
    left.partial_cmp(&right) == Some(std::cmp::Ordering::Equal)
}

/// Attaches the trimmed section text to its numeric record.
fn with_content((entry, mut content): (ChangelogEntry, String)) -> ChangelogEntry {
    let kept = content.trim_end_matches(is_whitespace_scalar).len();
    content.truncate(kept);
    ChangelogEntry { content, ..entry }
}

/// Reads the numeric version of an admitted heading line.
///
/// The numbers may follow later heading marks on the same line; a candidate that fails
/// to read a version keeps the overlapping start of the next one.
fn parse_heading(line: &str) -> Option<ChangelogEntry> {
    let mut rest = line;
    while let Some(at) = rest.find("##") {
        if let Some(entry) = read_version(&rest[at + 2..]) {
            return Some(entry);
        }
        rest = &rest[at + 1..];
    }
    None
}

/// Reads the heading whitespace, an optional bracket and three numeric components.
fn read_version(rest: &str) -> Option<ChangelogEntry> {
    let text = rest.trim_start_matches(is_whitespace_scalar);
    if text.len() == rest.len() {
        return None;
    }
    let text = text.strip_prefix('[').unwrap_or(text);
    let (major, text) = read_number(text)?;
    let (minor, text) = read_number(text.strip_prefix('.')?)?;
    let (patch, _) = read_number(text.strip_prefix('.')?)?;
    Some(ChangelogEntry {
        major,
        minor,
        patch,
        content: String::new(),
    })
}

/// Reads one run of ASCII digits as a number, returning the remaining text.
fn read_number(text: &str) -> Option<(f64, &str)> {
    let end = text.bytes().take_while(u8::is_ascii_digit).count();
    if end == 0 {
        return None;
    }
    let (digits, rest) = text.split_at(end);
    let number = digits.parse().ok()?;
    Some((number, rest))
}
