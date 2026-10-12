//! Package file patterns.
//!
//! Source attribution and [BlueOak-1.0.0](https://blueoakcouncil.org/license/1.0.0)
//! license details are in [third-party notices](https://github.com/Orchestration-Maestro/maestro/blob/main/THIRD-PARTY-NOTICES.md).

use std::collections::BTreeSet;

mod assert_valid_pattern;
mod ast;
mod brace_expressions;
mod preprocess;
pub use assert_valid_pattern::PatternError;

/// Matches a path against a package file pattern, including classes and nested groups.
///
/// See the [file-pattern guide](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/package-patterns.md)
/// for group cardinality, exclusion context and named-class definitions.
///
/// Matching is case-sensitive, except that native Windows framing compares a
/// recognized absolute or extended drive without case; a drive-relative prefix such
/// as `C:Foo` keeps its authored case. [`minimatch_with_nocase`] selects the other mode.
///
/// # Errors
/// Returns [`PatternError::TooLong`] for patterns longer than 65,536 scalars.
pub fn minimatch(path: &str, pattern: &str) -> Result<bool, PatternError> {
    minimatch_with_nocase(path, pattern, false)
}

/// Matches like [`minimatch`], comparing case-insensitively when `nocase` is true.
///
/// Case-insensitivity applies to literal and magic components alike; pattern syntax,
/// including named class names, is never rewritten. Under Windows framing a nonmagic
/// UNC server/share prefix and a drive-relative prefix keep their authored case, while
/// a recognized drive aligns without case as in [`minimatch`]; later components fold.
///
/// # Errors
/// Returns [`PatternError::TooLong`] for patterns longer than 65,536 scalars.
pub fn minimatch_with_nocase(
    path: &str,
    pattern: &str,
    nocase: bool,
) -> Result<bool, PatternError> {
    let platform = if cfg!(windows) {
        Platform::Windows
    } else {
        Platform::Posix
    };
    match_platform(path, pattern, platform, nocase)
}

/// Native path framing policy.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// Slash-separated paths.
    Posix,
    /// Slash framing with case-insensitive magic components.
    Darwin,
    /// Native input separators and drive/UNC identity.
    Windows,
}

/// Runs the same operation with explicit native framing.
fn match_platform(
    path: &str,
    pattern: &str,
    platform: Platform,
    nocase: bool,
) -> Result<bool, PatternError> {
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
    let hit = brace_expand(pattern, None).iter().any(|alternative| {
        let pattern = optimize_pattern(slash_split(alternative, platform));
        let mut parts = compile_parts(&pattern, platform);
        if nocase {
            fold_parts(&mut parts, &pattern, platform);
        }
        let (file_start, pattern_start) = align_roots(&file, &mut parts, platform);
        match_one(&file[file_start..], &parts[pattern_start..], false)
    });
    Ok(hit ^ (bangs % 2 == 1))
}

/// Expands only patterns containing a same-line brace pair.
pub(super) fn brace_expand(pattern: &str, maximum: Option<usize>) -> Vec<String> {
    let mut open = false;
    for c in pattern.chars() {
        match c {
            '{' => open = true,
            '}' if open => {
                return maximum.map_or_else(
                    || brace_expansion::expand(pattern),
                    |max| brace_expansion::expand_with_max(pattern, max),
                );
            }
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
    /// A component compared case-insensitively.
    Folded(MmRegExp),
}

impl Part {
    /// Borrows decoded identity text without treating wildcards as literal roots.
    fn literal(&self) -> Option<&str> {
        match self {
            Self::Segment(segment) => segment.literal(),
            Self::Globstar | Self::Folded(_) => None,
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

/// Count of leading components kept as authored under case-insensitive Windows matching.
fn protected_root(pattern: &[&str]) -> usize {
    static MAGIC: std::sync::LazyLock<Option<regress::Regex>> =
        std::sync::LazyLock::new(|| regress::Regex::new(r"[?*]|[+@!]\(.*?\)|\[|\]").ok());
    let magic = |part: Option<&&str>| {
        part.is_some_and(|p| MAGIC.as_ref().is_some_and(|re| re.find(p).is_some()))
    };
    if pattern.starts_with(&["", ""])
        && (pattern.get(2) == Some(&"?") || !magic(pattern.get(2)))
        && !magic(pattern.get(3))
    {
        4
    } else {
        usize::from(pattern.first().is_some_and(|first| {
            first
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphabetic)
                && first.as_bytes().get(1) == Some(&b':')
        }))
    }
}

/// Switches components to case-insensitive evaluation, sparing protected Windows roots.
fn fold_parts(parts: &mut [Part], pattern: &[&str], platform: Platform) {
    let protected = if platform == Platform::Windows {
        protected_root(pattern)
    } else {
        0
    };
    for (index, part) in parts.iter_mut().enumerate() {
        if index < protected {
            *part = Part::Segment(ast::Ast::Literal(pattern[index].into()));
        } else if let Part::Segment(ast) = std::mem::replace(part, Part::Globstar) {
            *part = Part::Folded(MmRegExp::new(ast, pattern[index], true));
        } else {
            *part = Part::Globstar;
        }
    }
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
fn match_one(file: &[&str], parts: &[Part], dot: bool) -> bool {
    let mut globstars = parts
        .iter()
        .enumerate()
        .filter_map(|(i, part)| matches!(part, Part::Globstar).then_some(i));
    if let Some(index) = globstars.next()
        && globstars.next().is_none()
    {
        return match_single_globstar(file, parts, index, dot);
    }
    let mut states = BTreeSet::from([0]);
    close_globstars(parts, &mut states);
    for (position, text) in file.iter().enumerate() {
        if states.contains(&parts.len()) && position + 1 == file.len() && text.is_empty() {
            return true;
        }
        let mut next = BTreeSet::new();
        for &i in &states {
            advance_part(parts, i, text, &mut next, dot);
        }
        close_globstars(parts, &mut next);
        states = next;
    }
    states.contains(&parts.len())
}

/// Anchors the head and tail before checking the globstar's middle once.
fn match_single_globstar(file: &[&str], parts: &[Part], index: usize, dot: bool) -> bool {
    let tail = &parts[index + 1..];
    if index + tail.len() > file.len() || !match_one(&file[..index], &parts[..index], dot) {
        return false;
    }
    let mut tail_start = file.len() - tail.len();
    if !tail.is_empty() && !match_one(&file[tail_start..], tail, dot) {
        if file.last() != Some(&"") || tail_start == index {
            return false;
        }
        tail_start -= 1;
        if !match_one(&file[tail_start..], tail, dot) {
            return false;
        }
    }
    (tail_start > index || !tail.is_empty())
        && file[index..tail_start]
            .iter()
            .all(|text| dot || !text.starts_with('.'))
}

/// Consumes a component at an active pattern position.
fn advance_part(
    parts: &[Part],
    position: usize,
    text: &str,
    next: &mut BTreeSet<usize>,
    dot: bool,
) {
    match parts.get(position) {
        Some(Part::Globstar) if dot || !text.starts_with('.') => {
            next.insert(position);
            if position + 1 == parts.len() {
                next.insert(position + 1);
            }
        }
        Some(Part::Segment(segment)) if segment.matches(text) => {
            next.insert(position + 1);
        }
        Some(Part::Folded(segment)) if segment.test(text) => {
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

/// Compiled slash-relative ignore syntax without public file-pattern defaults.
pub(crate) struct IgnorePattern {
    /// Shared component and globstar states.
    parts: Vec<Part>,
}
impl IgnorePattern {
    /// Compiles authored components, collapsing adjacent recursive wildcards.
    pub(crate) fn new(pattern: &str) -> Self {
        let mut parts: Vec<_> = pattern
            .split('/')
            .map(|part| {
                if part == "**" {
                    Part::Globstar
                } else {
                    Part::Segment(ast::Ast::parse_ignore(part))
                }
            })
            .collect();
        parts.dedup_by(|a, b| matches!(a, Part::Globstar) && matches!(b, Part::Globstar));
        Self { parts }
    }
    /// Matches components already prepared by the rule owner.
    pub(crate) fn matches(&self, components: &[&str]) -> bool {
        match_one(components, &self.parts, true)
    }
}
/// Canonicalizes one scalar without expansions or non-ASCII-to-ASCII mappings.
pub(crate) fn canonical(scalar: char) -> char {
    let mut uppercase = scalar.to_uppercase();
    let first = uppercase.next().unwrap_or(scalar);
    if uppercase.next().is_some() || (!scalar.is_ascii() && first.is_ascii()) {
        scalar
    } else {
        first
    }
}

#[cfg(test)]
mod tests;

/// Fixed matcher policy selected by filesystem expansion.
#[derive(Clone, Copy)]
pub enum MinimatchOptions {
    /// Native framing with comments and leading negation disabled.
    Filesystem(Platform),
}
/// A prepared filesystem component.
pub enum ParseReturnFiltered {
    /// Decoded text for a literal filesystem lookup.
    Literal(String),
    /// A recursive wildcard between components.
    Globstar,
    /// One component evaluated by the shared matcher.
    Pattern(MmRegExp),
}
/// Compiled component syntax without filesystem effects.
pub struct MmRegExp {
    /// Shared parsed syntax.
    ast: ast::Ast,
    /// Lowercased fast suffix with optional scalar cardinality.
    suffix: Option<(String, Option<usize>)>,
}
impl MmRegExp {
    /// Selects case predicates or the existing literal-suffix evaluation branch.
    fn new(mut ast: ast::Ast, authored: &str, nocase: bool) -> Self {
        let suffix = nocase.then(|| fast_suffix(authored)).flatten();
        if nocase && suffix.is_none() {
            ast.fold_case();
        }
        Self { ast, suffix }
    }

    /// Evaluates one native entry name.
    #[must_use]
    pub fn test(&self, text: &str) -> bool {
        if let Some((suffix, length)) = &self.suffix {
            return !text.starts_with('.')
                && !text.is_empty()
                && length.is_none_or(|n| text.chars().count() == n)
                && text.to_lowercase().ends_with(suffix);
        }
        self.ast.matches(text)
    }
}
/// Ordered prepared alternatives for filesystem expansion.
pub struct Minimatch {
    /// Compiled alternatives aligned with authored components.
    set: Vec<Vec<ParseReturnFiltered>>,
    /// Prepared authored components before literal escape decoding.
    glob_parts: Vec<Vec<String>>,
}
impl Minimatch {
    /// Prepares a filesystem pattern under its selected native policy.
    ///
    /// # Errors
    /// Returns the existing pattern-length error.
    pub fn new(pattern: &str, options: MinimatchOptions) -> Result<Self, PatternError> {
        assert_valid_pattern::assert_valid_pattern(pattern)?;
        let MinimatchOptions::Filesystem(platform) = options;
        let glob_parts = preprocess::prepare(pattern, platform);
        let set = glob_parts
            .iter()
            .map(|parts| {
                let borrowed: Vec<_> = parts.iter().map(String::as_str).collect();
                compile_parts(&borrowed, platform)
                    .into_iter()
                    .enumerate()
                    .map(|(index, part)| match part {
                        Part::Globstar => ParseReturnFiltered::Globstar,
                        Part::Segment(ast::Ast::Literal(text)) => {
                            ParseReturnFiltered::Literal(text)
                        }
                        Part::Segment(ast) => ParseReturnFiltered::Pattern(MmRegExp::new(
                            ast,
                            &parts[index],
                            platform != Platform::Posix,
                        )),
                        Part::Folded(regexp) => ParseReturnFiltered::Pattern(regexp),
                    })
                    .collect()
            })
            .collect();
        Ok(Self { set, glob_parts })
    }
    /// Borrows the ordered compiled component alternatives.
    #[must_use]
    pub fn set(&self) -> &[Vec<ParseReturnFiltered>] {
        &self.set
    }
    /// Borrows the aligned prepared authored components.
    #[must_use]
    pub fn glob_parts(&self) -> &[Vec<String>] {
        &self.glob_parts
    }
}

/// Selects the dependency's literal-suffix shortcuts after component parsing.
fn fast_suffix(pattern: &str) -> Option<(String, Option<usize>)> {
    let operator = pattern.chars().next()?;
    if !matches!(operator, '*' | '?') {
        return None;
    }
    let count = pattern.chars().take_while(|c| *c == operator).count();
    let suffix = &pattern[count..];
    if suffix.contains(['+', '@', '!', '?', '*', '[', '(', '\\']) {
        return None;
    }
    let length = (operator == '?').then_some(count + suffix.chars().count());
    Some((suffix.to_lowercase(), length))
}
