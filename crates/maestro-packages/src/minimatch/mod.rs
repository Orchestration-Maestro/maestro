//! Package file patterns.
//!
//! Source attribution and [BlueOak-1.0.0](https://blueoakcouncil.org/license/1.0.0)
//! license details are in [third-party notices](https://github.com/Orchestration-Maestro/maestro/blob/main/THIRD-PARTY-NOTICES.md).

use std::collections::BTreeSet;

mod assert_valid_pattern;
mod ast;
mod brace_expressions;
pub use assert_valid_pattern::PatternError;

/// Matches a path against a package file pattern, including bracket classes.
///
/// Extended groups are not yet implemented.
/// See the [file-pattern guide](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/package-patterns.md)
/// for class forms and named-category definitions.
///
/// # Errors
/// Returns [`PatternError::TooLong`] for patterns longer than 65,536 scalars.
pub fn minimatch(path: &str, pattern: &str) -> Result<bool, PatternError> {
    let platform = if cfg!(windows) {
        Platform::Windows
    } else {
        Platform::Posix
    };
    match_platform(path, pattern, platform)
}

/// Native path framing policy.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Platform {
    /// Slash-separated paths.
    Posix,
    /// Native input separators and drive/UNC identity.
    Windows,
}

/// Runs the same operation with explicit native framing.
fn match_platform(path: &str, pattern: &str, platform: Platform) -> Result<bool, PatternError> {
    assert_valid_pattern::assert_valid_pattern(pattern)?;
    if pattern.starts_with('#') {
        return Ok(false);
    }
    if pattern.is_empty() {
        return Ok(path.is_empty());
    }
    let bangs = pattern.bytes().take_while(|b| *b == b'!').count();
    let pattern = &pattern[bangs..];
    let native_path;
    let path = if platform == Platform::Windows {
        native_path = path.replace('\\', "/");
        &native_path
    } else {
        path
    };
    let file = slash_split(path, platform);
    let hit = brace_expand(pattern).iter().any(|alternative| {
        let pattern = optimize_pattern(slash_split(alternative, platform));
        let mut parts = compile_parts(&pattern, platform);
        let (file_start, pattern_start) = align_roots(&file, &mut parts, platform);
        match_one(&file[file_start..], &parts[pattern_start..])
    });
    Ok(hit ^ (bangs % 2 == 1))
}

/// Expands only patterns containing a same-line brace pair.
fn brace_expand(pattern: &str) -> Vec<String> {
    let mut open = false;
    for c in pattern.chars() {
        match c {
            '{' => open = true,
            '}' if open => return brace_expansion::expand(pattern),
            '\n' | '\r' | '\u{2028}' | '\u{2029}' => open = false,
            _ => {}
        }
    }
    vec![pattern.to_owned()]
}

/// Finds a drive component, including the extended UNC prefix.
fn drive_index(parts: &[&str]) -> Option<usize> {
    let index = if parts.starts_with(&["", "", "?"]) {
        3
    } else {
        0
    };
    parts
        .get(index)
        .filter(|p| {
            let bytes = p.as_bytes();
            bytes.len() == 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
        })
        .map(|_| index)
}

/// Aligns equal native drives without folding ordinary component case.
fn align_roots(file: &[&str], pattern: &mut [Part], platform: Platform) -> (usize, usize) {
    if platform == Platform::Windows
        && let (Some(f), Some(p)) = (drive_index(file), pattern_drive_index(pattern))
        && pattern[p]
            .literal()
            .is_some_and(|root| file[f].eq_ignore_ascii_case(root))
    {
        pattern[p] = Part::Segment(ast::Ast::Literal(file[f].into()));
        return (f, p);
    }
    (0, 0)
}

/// Reduces eligible pattern parents and adjacent globstars, never input paths.
fn optimize_pattern(parts: Vec<&str>) -> Vec<&str> {
    let mut reduced = Vec::new();
    for part in parts {
        let previous = reduced.last().copied();
        if part == "**" && previous == Some("**") {
            continue;
        }
        if part == ".." && previous.is_some_and(|p| !matches!(p, "" | "." | ".." | "**")) {
            reduced.pop();
        } else {
            reduced.push(part);
        }
    }
    if reduced.is_empty() {
        reduced.push("");
    }
    reduced
}

/// A compiled path component.
enum Part {
    /// A recursive wildcard between components.
    Globstar,
    /// A scalar component matcher.
    Segment(ast::Ast),
}

impl Part {
    /// Borrows decoded identity text without treating wildcards as literal roots.
    fn literal(&self) -> Option<&str> {
        match self {
            Self::Segment(segment) => segment.literal(),
            Self::Globstar => None,
        }
    }
}

/// Finds a decoded literal drive without erasing wildcard identity.
fn pattern_drive_index(parts: &[Part]) -> Option<usize> {
    let index = if parts
        .iter()
        .take(3)
        .map(Part::literal)
        .eq([Some(""), Some(""), Some("?")])
    {
        3
    } else {
        0
    };
    parts
        .get(index)
        .and_then(Part::literal)
        .and_then(|drive| drive_index(&[drive]))
        .map(|_| index)
}

/// Compiles components and preserves the native extended-drive marker.
fn compile_parts(pattern: &[&str], platform: Platform) -> Vec<Part> {
    let mut parts: Vec<_> = pattern
        .iter()
        .map(|p| {
            if *p == "**" {
                Part::Globstar
            } else {
                Part::Segment(ast::Ast::parse(p))
            }
        })
        .collect();
    if platform == Platform::Windows
        && pattern.starts_with(&["", "", "?"])
        && parts
            .get(3)
            .and_then(Part::literal)
            .is_some_and(|drive| drive_index(&[drive]) == Some(0))
    {
        parts[2] = Part::Segment(ast::Ast::Literal("?".into()));
    }
    parts
}

/// Advances globstars that have a following component.
fn close_globstars(parts: &[Part], states: &mut BTreeSet<usize>) {
    let advances: Vec<_> = states
        .iter()
        .copied()
        .filter(|i| i + 1 < parts.len() && matches!(parts.get(*i), Some(Part::Globstar)))
        .map(|i| i + 1)
        .collect();
    states.extend(advances);
}

/// Walks finite component-position states without a recursion cutoff.
fn match_one(file: &[&str], parts: &[Part]) -> bool {
    let mut globstars = parts
        .iter()
        .enumerate()
        .filter_map(|(i, part)| matches!(part, Part::Globstar).then_some(i));
    if let Some(index) = globstars.next()
        && globstars.next().is_none()
    {
        return match_single_globstar(file, parts, index);
    }
    let mut states = BTreeSet::from([0]);
    close_globstars(parts, &mut states);
    for (position, text) in file.iter().enumerate() {
        if states.contains(&parts.len()) && position + 1 == file.len() && text.is_empty() {
            return true;
        }
        let mut next = BTreeSet::new();
        for &i in &states {
            advance_part(parts, i, text, &mut next);
        }
        close_globstars(parts, &mut next);
        states = next;
    }
    states.contains(&parts.len())
}

/// Anchors the head and tail before checking the globstar's middle once.
fn match_single_globstar(file: &[&str], parts: &[Part], index: usize) -> bool {
    let tail = &parts[index + 1..];
    if index + tail.len() > file.len() || !match_one(&file[..index], &parts[..index]) {
        return false;
    }
    let mut tail_start = file.len() - tail.len();
    if !tail.is_empty() && !match_one(&file[tail_start..], tail) {
        if file.last() != Some(&"") || tail_start == index {
            return false;
        }
        tail_start -= 1;
        if !match_one(&file[tail_start..], tail) {
            return false;
        }
    }
    (tail_start > index || !tail.is_empty())
        && file[index..tail_start]
            .iter()
            .all(|text| !text.starts_with('.'))
}

/// Consumes a component at an active pattern position.
fn advance_part(parts: &[Part], position: usize, text: &str, next: &mut BTreeSet<usize>) {
    match parts.get(position) {
        Some(Part::Globstar) if !text.starts_with('.') => {
            next.insert(position);
            if position + 1 == parts.len() {
                next.insert(position + 1);
            }
        }
        Some(Part::Segment(segment)) if segment.matches(text) => {
            next.insert(position + 1);
        }
        _ => {}
    }
}

/// Collapses repeated separators while preserving endpoint empty components.
fn slash_split(text: &str, platform: Platform) -> Vec<&str> {
    let mut parts: Vec<_> = text.split('/').filter(|p| !p.is_empty()).collect();
    if text.starts_with('/') {
        parts.insert(0, "");
    }
    if platform == Platform::Windows
        && text.starts_with("//")
        && text.as_bytes().get(2).is_some_and(|c| *c != b'/')
    {
        parts.insert(0, "");
    }
    if text.ends_with('/') || text.is_empty() {
        parts.push("");
    }
    parts
}

#[cfg(test)]
mod tests;
