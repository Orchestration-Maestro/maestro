//! Ordered character-based ignore rules.
//!
//! MIT attribution is in [third-party notices](https://github.com/Orchestration-Maestro/maestro/blob/main/THIRD-PARTY-NOTICES.md#package-ignore-rules).

use crate::minimatch::{IgnorePattern, canonical};

/// The decision of the last matching rule, or absence of a matching rule.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TestResult {
    /// No rule selected the entry.
    Unmatched,
    /// A positive rule excludes the entry.
    Ignored,
    /// A negative rule readmits the entry.
    Unignored,
}
/// Compiled ordered rules belonging to one directory.
pub(crate) struct Ignore {
    /// Rules in authored order.
    rules: Vec<Rule>,
}
/// One compiled entry decision.
struct Rule {
    /// The decision emitted on a match.
    result: TestResult,
    /// Whether the rule selects only directories.
    directory: bool,
    /// Compiled component syntax.
    pattern: IgnorePattern,
}
impl Ignore {
    /// Creates an empty rule list.
    pub(crate) fn new() -> Self {
        Self { rules: Vec::new() }
    }
    /// Admits nonblank, noncomment lines with no dangling escape.
    pub(crate) fn add(&mut self, pattern: &str) {
        if pattern.is_empty()
            || pattern.starts_with('#')
            || pattern.chars().all(crate::whitespace)
            || pattern.chars().rev().take_while(|c| *c == '\\').count() % 2 == 1
        {
            return;
        }
        let (result, body) = pattern
            .strip_prefix('!')
            .map_or((TestResult::Ignored, pattern), |body| {
                (TestResult::Unignored, body)
            });
        let prepared = prepare(body);
        let anchored = prepared.starts_with('/');
        let body = prepared.strip_prefix('/').unwrap_or(&prepared);
        let directory = body.ends_with('/');
        let body = if directory {
            &body[..body.len() - 1]
        } else {
            body
        };
        let mut pattern = if !anchored && !body.contains('/') && body != "**" {
            format!("**/{body}")
        } else {
            body.to_owned()
        };
        if directory {
            pattern.push('/');
        } else if pattern.ends_with("/**") {
            pattern.push_str("/*");
        }
        self.rules.push(Rule {
            result,
            directory,
            pattern: IgnorePattern::new(&pattern),
        });
    }
    /// Folds and splits an entry once, then selects its last matching rule.
    pub(crate) fn test(&self, path: &str, directory: bool) -> TestResult {
        let folded: String = path.chars().map(canonical).collect();
        let mut components: Vec<_> = folded.split('/').collect();
        if directory {
            components.push("");
        }
        self.rules
            .iter()
            .rev()
            .find(|rule| (!rule.directory || directory) && rule.pattern.matches(&components))
            .map_or(TestResult::Unmatched, |rule| rule.result)
    }
}
/// Removes trailing authored whitespace and decodes quoted whitespace to ASCII space.
fn prepare(pattern: &str) -> String {
    let pattern = pattern.strip_prefix('\u{feff}').unwrap_or(pattern);
    let trimmed = pattern.trim_end_matches(crate::whitespace);
    let escapes = trimmed.chars().rev().take_while(|c| *c == '\\').count();
    let mut tail = trimmed.to_owned();
    if trimmed.len() != pattern.len() && escapes % 2 == 1 {
        tail.pop();
        tail.push(' ');
    }
    let mut result = String::new();
    let mut chars = tail.chars().peekable();
    while let Some(scalar) = chars.next() {
        if scalar != '\\' {
            result.push(scalar);
            continue;
        }
        let mut count = 1;
        while chars.peek() == Some(&'\\') {
            chars.next();
            count += 1;
        }
        if chars.peek().is_some_and(|c| crate::whitespace(*c)) {
            chars.next();
            result.extend(std::iter::repeat_n('\\', count - count % 2));
            result.push(' ');
        } else {
            result.extend(std::iter::repeat_n('\\', count));
            if let Some(scalar) = chars.next() {
                result.push(scalar);
            }
        }
    }
    result
}

#[cfg(test)]
mod tests;
