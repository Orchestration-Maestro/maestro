//! Ordered directory traversal of instruction entry files.
use super::{LoadSkillsResult, ResourceEntry, ResourceFileType, ResourceOperations, load_file};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use maestro_path::{Cwd, SEP, dirname, join, relative, resolve};
use std::path::Path;

/// Shared scan-local observations and ignore-rule state.
struct Discovery<'a> {
    /// Containing scan root for authored rule prefixes.
    root: String,
    /// Working directory for matcher coordinates.
    cwd: Cwd<'a>,
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
    dir: &str,
    cwd: &Cwd<'_>,
    source: &str,
    operations: &dyn ResourceOperations,
) -> LoadSkillsResult {
    let root = resolve(&[dir], cwd);
    let mut builder = GitignoreBuilder::new(&root);
    let _ = builder.case_insensitive(true);
    let mut scan = Discovery {
        root,
        cwd: *cwd,
        source,
        operations,
        builder,
        matcher: Gitignore::empty(),
    };
    scan.visit(dir, true)
}
impl Discovery<'_> {
    /// Traverse in adapter order, stopping after an attempted root entry.
    fn visit(&mut self, dir: &str, include_root_files: bool) -> LoadSkillsResult {
        let mut result = LoadSkillsResult::default();
        if !self.operations.exists(Path::new(dir)) {
            return result;
        }
        self.add_rules(dir);
        let Ok(entries) = self.operations.read_dir(Path::new(dir)) else {
            return result;
        };
        for entry in entries.iter().filter(|e| e.name == "SKILL.md") {
            let path = join(&[dir, &entry.name.to_string_lossy()]);
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
            let path = join(&[dir, &entry.name.to_string_lossy()]);
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
    fn ignored(&self, path: &str, is_dir: bool) -> bool {
        let candidate = resolve(&[path], &self.cwd);
        let candidate = relative(&self.root, &candidate, &self.cwd);
        let mut parent = dirname(&candidate);
        while parent != "." {
            if self.matcher.matched(&parent, true).is_ignore() {
                return true;
            }
            parent = dirname(&parent);
        }
        self.matcher.matched(&candidate, is_dir).is_ignore()
    }
    /// Append authored directory-prefixed rules in ignore-file order.
    fn add_rules(&mut self, dir: &str) {
        let prefix = relative(&self.root, dir, &self.cwd).replace(SEP, "/");
        let prefix = if prefix.is_empty() {
            prefix
        } else {
            format!("{prefix}/")
        };
        let mut changed = false;
        for filename in [".gitignore", ".ignore", ".fdignore"] {
            let path = join(&[dir, filename]);
            if !self.operations.exists(Path::new(&path)) {
                continue;
            }
            let Ok(text) = self.operations.read_file(Path::new(&path)) else {
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
    path: &str,
    operations: &dyn ResourceOperations,
) -> Option<ResourceFileType> {
    if entry.file_type == ResourceFileType::Symlink {
        operations.metadata(Path::new(path)).ok()
    } else {
        Some(entry.file_type)
    }
}
/// Append ordered discoveries and diagnostics without copying resources.
fn append(result: &mut LoadSkillsResult, next: LoadSkillsResult) {
    result.skills.extend(next.skills);
    result.diagnostics.extend(next.diagnostics);
}
