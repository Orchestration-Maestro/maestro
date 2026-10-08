//! Ordered directory traversal of instruction entry files.
use super::{LoadSkillsResult, ResourceEntry, ResourceFileType, ResourceOperations, load_file};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::path::Path;

/// Shared scan-local observations and ignore-rule state.
struct Discovery<'a> {
    /// Containing scan root for authored rule prefixes.
    root: &'a Path,
    /// Source label for every discovery.
    source: &'a str,
    /// Replaceable filesystem adapter.
    operations: &'a dyn ResourceOperations,
    /// Ordered rules compiled only when new rules arrive.
    builder: GitignoreBuilder,
    /// Most recently successful compiled matcher.
    matcher: Gitignore,
}
/// Scan nested directories, preferring each directory's entry file.
pub(super) fn scan(
    dir: &Path,
    source: &str,
    operations: &dyn ResourceOperations,
) -> LoadSkillsResult {
    let mut builder = GitignoreBuilder::new(dir);
    let _ = builder.case_insensitive(true);
    let mut scan = Discovery {
        root: dir,
        source,
        operations,
        builder,
        matcher: Gitignore::empty(),
    };
    scan.visit(dir, true)
}
impl Discovery<'_> {
    /// Traverse in adapter order, stopping after an attempted root entry.
    fn visit(&mut self, dir: &Path, include_root_files: bool) -> LoadSkillsResult {
        let mut result = LoadSkillsResult::default();
        if !self.operations.exists(dir) {
            return result;
        }
        self.add_rules(dir);
        let Ok(entries) = self.operations.read_dir(dir) else {
            return result;
        };
        for entry in entries.iter().filter(|e| e.name == "SKILL.md") {
            let path = crate::paths::join_path(&[dir, Path::new(&entry.name)]);
            if observed_type(entry, &path, self.operations) == Some(ResourceFileType::File)
                && !self.ignored(&path, false)
            {
                return load_file(&path, self.source, self.operations);
            }
        }
        for entry in entries {
            if entry.name.to_string_lossy().starts_with('.') || entry.name == "node_modules" {
                continue;
            }
            let path = crate::paths::join_path(&[dir, Path::new(&entry.name)]);
            let kind = observed_type(&entry, &path, self.operations);
            if self.ignored(&path, kind == Some(ResourceFileType::Directory)) {
                continue;
            }
            match kind {
                Some(ResourceFileType::Directory) => append(&mut result, self.visit(&path, false)),
                Some(ResourceFileType::File)
                    if include_root_files && entry.name.to_string_lossy().ends_with(".md") =>
                {
                    append(&mut result, load_file(&path, self.source, self.operations));
                }
                _ => {}
            }
        }
        result
    }
    /// Check excluded ancestors before a candidate's own negations.
    fn ignored(&self, path: &Path, is_dir: bool) -> bool {
        let relative = path.strip_prefix(self.root).unwrap_or(path);
        relative
            .ancestors()
            .skip(1)
            .filter(|p| !p.as_os_str().is_empty())
            .any(|parent| {
                self.matcher
                    .matched(crate::paths::join_path(&[self.root, parent]), true)
                    .is_ignore()
            })
            || self.matcher.matched(path, is_dir).is_ignore()
    }
    /// Append authored directory-prefixed rules in ignore-file order.
    fn add_rules(&mut self, dir: &Path) {
        let relative = pathdiff::diff_paths(dir, self.root).unwrap_or_default();
        let prefix = relative
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        let prefix = if prefix.is_empty() {
            prefix
        } else {
            format!("{prefix}/")
        };
        let mut changed = false;
        for filename in [".gitignore", ".ignore", ".fdignore"] {
            let path = crate::paths::join_path(&[dir, Path::new(filename)]);
            if !self.operations.exists(&path) {
                continue;
            }
            let Ok(text) = self.operations.read_file(&path) else {
                continue;
            };
            for pattern in text
                .split('\n')
                .map(|s| s.strip_suffix('\r').unwrap_or(s))
                .filter_map(|line| prefix_pattern(line, &prefix))
            {
                changed |= self.builder.add_line(None, &pattern).is_ok();
            }
        }
        if changed && let Ok(matcher) = self.builder.build() {
            self.matcher = matcher;
        }
    }
}
/// Prefix an authored ignore pattern without turning escaped punctuation into operators.
fn prefix_pattern(line: &str, prefix: &str) -> Option<String> {
    let trimmed = line.trim_matches(crate::frontmatter::text_whitespace);
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let (negation, pattern) = line.strip_prefix('!').map_or(("", line), |p| ("!", p));
    let pattern = pattern.strip_prefix('/').unwrap_or(pattern);
    Some(format!("{negation}{prefix}{pattern}"))
}
/// Follow symlink metadata without turning broken links into diagnostics.
fn observed_type(
    entry: &ResourceEntry,
    path: &Path,
    operations: &dyn ResourceOperations,
) -> Option<ResourceFileType> {
    if entry.file_type == ResourceFileType::Symlink {
        operations.metadata(path).ok()
    } else {
        Some(entry.file_type)
    }
}
/// Append ordered discoveries and diagnostics without copying resources.
fn append(result: &mut LoadSkillsResult, next: LoadSkillsResult) {
    result.skills.extend(next.skills);
    result.diagnostics.extend(next.diagnostics);
}
