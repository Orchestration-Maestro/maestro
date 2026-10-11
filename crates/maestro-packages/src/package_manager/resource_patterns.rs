//! Manifest resource selection over authored path views.
use maestro_path::{basename, dirname, relative};
use std::{collections::HashSet, io};
/// Converts only native separators to matcher separators.
fn posix(text: &str) -> String {
    if cfg!(windows) {
        text.replace('\\', "/")
    } else {
        text.to_owned()
    }
}
/// The relative, basename and absolute spellings of one candidate.
struct PathViews {
    /// Root-relative identity.
    relative: String,
    /// Last path component.
    name: String,
    /// Absolute identity.
    absolute: String,
}
impl PathViews {
    /// Builds matcher views without changing candidate identity.
    fn new(path: &str, root: &str) -> Self {
        Self {
            relative: posix(&relative(
                root,
                path,
                &maestro_path::Cwd {
                    current: "",
                    drive_directories: &[],
                },
            )),
            name: basename(path, None),
            absolute: posix(path),
        }
    }
    /// Tests relative and absolute identities, never basename alone.
    fn exact(&self, patterns: &HashSet<String>) -> bool {
        patterns.contains(&self.relative) || patterns.contains(&self.absolute)
    }
    /// Tries each observable path spelling against one exclusion pattern.
    fn matches(&self, pattern: &str) -> io::Result<bool> {
        for path in [&self.relative, &self.name, &self.absolute] {
            if crate::minimatch(path, pattern)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
/// Selects admitted identities; the caller retains candidate emission order.
pub(super) fn apply_patterns<'a>(
    files: &'a [String],
    entries: &[String],
    root: &str,
) -> io::Result<HashSet<&'a str>> {
    let exclusions: Vec<_> = entries
        .iter()
        .filter_map(|entry| entry.strip_prefix('!'))
        .map(posix)
        .collect();
    let includes = exact_patterns(entries, '+');
    let removals = exact_patterns(entries, '-');
    let mut enabled = HashSet::new();
    for file in files {
        let views = PathViews::new(file, root);
        let parent = (views.name == "SKILL.md").then(|| PathViews::new(&dirname(file), root));
        let mut excluded = false;
        for pattern in &exclusions {
            if views.matches(pattern)?
                || parent
                    .as_ref()
                    .map(|parent| parent.matches(pattern))
                    .transpose()?
                    .unwrap_or(false)
            {
                excluded = true;
                break;
            }
        }
        let restored = views.exact(&includes)
            || parent
                .as_ref()
                .is_some_and(|parent| parent.exact(&includes));
        let removed = views.exact(&removals)
            || parent
                .as_ref()
                .is_some_and(|parent| parent.exact(&removals));
        if (!excluded || restored) && !removed {
            enabled.insert(file.as_str());
        }
    }
    Ok(enabled)
}

/// Normalizes exact overrides by removing at most one authored dot prefix.
fn exact_patterns(entries: &[String], modifier: char) -> HashSet<String> {
    entries
        .iter()
        .filter_map(|entry| entry.strip_prefix(modifier))
        .map(|pattern| {
            posix(
                pattern
                    .strip_prefix("./")
                    .or_else(|| pattern.strip_prefix(".\\"))
                    .unwrap_or(pattern),
            )
        })
        .collect()
}
