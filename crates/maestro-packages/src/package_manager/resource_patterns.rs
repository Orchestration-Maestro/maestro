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
/// The root-relative, basename and authored spellings of one candidate.
struct PathViews {
    /// Root-relative identity.
    relative: String,
    /// Last path component.
    name: String,
    /// Authored identity.
    authored: String,
}
impl PathViews {
    /// Builds matcher views without changing candidate identity.
    fn new(
        path: &str,
        root: &str,
        current: &mut Option<String>,
        operations: &dyn maestro_resources::skills::ResourceOperations,
    ) -> io::Result<Self> {
        let directories = operations.drive_directories();
        let drives: Vec<_> = directories.iter().map(|(d, p)| (*d, p.as_str())).collect();
        if current.is_none()
            && [root, path]
                .iter()
                .any(|p| maestro_path::try_resolve(&[p], &drives).is_err())
        {
            *current = Some(operations.current_directory()?);
        }
        Ok(Self {
            relative: posix(&relative(
                root,
                path,
                &maestro_path::Cwd {
                    current: current.as_deref().unwrap_or(""),
                    drive_directories: &drives,
                },
            )),
            name: basename(path, None),
            authored: posix(path),
        })
    }
    /// Tests root-relative and authored identities, never basename alone.
    fn exact(&self, patterns: &HashSet<String>) -> bool {
        patterns.contains(&self.relative) || patterns.contains(&self.authored)
    }
    /// Tries each observable path spelling against one exclusion pattern.
    fn matches(&self, pattern: &str) -> io::Result<bool> {
        for path in [&self.relative, &self.name, &self.authored] {
            if crate::minimatch(path, pattern)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
/// Selected identities in survivor-then-restoration order.
pub(super) struct Selection<'a> {
    /// Ordered selected candidate identities.
    pub(super) ordered: Vec<&'a str>,
    /// Membership for candidate-ordered callers.
    members: HashSet<&'a str>,
}
impl Selection<'_> {
    /// Tests whether a candidate survives selection.
    pub(super) fn contains(&self, path: &str) -> bool {
        self.members.contains(path)
    }
}
/// Applies only manifest and automatic modifier patterns.
pub(super) fn apply_patterns<'a>(
    files: &'a [String],
    entries: &[impl AsRef<str>],
    root: &str,
    operations: &dyn maestro_resources::skills::ResourceOperations,
) -> io::Result<Selection<'a>> {
    selection(files, entries, root, operations, false)
}
/// Selects positive matches, exclusions, exact restorations and final removals.
pub(super) fn select_patterns<'a>(
    files: &'a [String],
    entries: &[impl AsRef<str>],
    root: &str,
    operations: &dyn maestro_resources::skills::ResourceOperations,
) -> io::Result<Selection<'a>> {
    selection(files, entries, root, operations, true)
}
/// Shares ordered selection while retaining the caller's positive-pattern framing.
fn selection<'a>(
    files: &'a [String],
    entries: &[impl AsRef<str>],
    root: &str,
    operations: &dyn maestro_resources::skills::ResourceOperations,
    positive: bool,
) -> io::Result<Selection<'a>> {
    let positives: Vec<_> = entries
        .iter()
        .map(AsRef::as_ref)
        .filter(|entry| positive && !entry.starts_with(['!', '+', '-']))
        .map(posix)
        .collect();
    let exclusions: Vec<_> = entries
        .iter()
        .map(AsRef::as_ref)
        .filter_map(|entry| entry.strip_prefix('!'))
        .map(posix)
        .collect();
    let includes = exact_patterns(entries, '+');
    let removals = exact_patterns(entries, '-');
    let mut selected = Vec::new();
    let mut restored = Vec::new();
    let mut current = None;
    for file in files {
        if positives.is_empty() && exclusions.is_empty() && removals.is_empty() {
            selected.push(file.as_str());
            continue;
        }
        let views = PathViews::new(file, root, &mut current, operations)?;
        let parent = (views.name == "SKILL.md")
            .then(|| PathViews::new(&dirname(file), root, &mut current, operations))
            .transpose()?;
        let exact = |patterns| {
            views.exact(patterns) || parent.as_ref().is_some_and(|parent| parent.exact(patterns))
        };
        let admitted = positives.is_empty() || matches_any(&views, parent.as_ref(), &positives)?;
        let surviving = admitted && !matches_any(&views, parent.as_ref(), &exclusions)?;
        if exact(&removals) {
            continue;
        }
        if surviving {
            selected.push(file.as_str());
        } else if exact(&includes) {
            restored.push(file.as_str());
        }
    }
    let mut members = HashSet::new();
    selected.extend(restored);
    selected.retain(|path| members.insert(*path));
    Ok(Selection {
        ordered: selected,
        members,
    })
}
/// Matches either a file or its skill-directory views against the supplied patterns.
fn matches_any(
    views: &PathViews,
    parent: Option<&PathViews>,
    patterns: &[String],
) -> io::Result<bool> {
    for pattern in patterns {
        if views.matches(pattern)?
            || parent
                .map(|parent| parent.matches(pattern))
                .transpose()?
                .unwrap_or(false)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Normalizes exact overrides by removing at most one authored dot prefix.
fn exact_patterns(entries: &[impl AsRef<str>], modifier: char) -> HashSet<String> {
    entries
        .iter()
        .map(AsRef::as_ref)
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
