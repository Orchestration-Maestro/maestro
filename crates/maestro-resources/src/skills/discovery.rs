//! Ordered directory traversal of instruction entry files.
use super::operations::ProcessContext;
use super::{LoadSkillsResult, ResourceEntry, ResourceFileType, ResourceOperations, load_file};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use maestro_path::{Cwd, SEP, dirname, join};
use std::{io, path::Path};

/// Shared scan-local observations and ignore-rule state.
struct Discovery<'a> {
    /// The authored scan directory, which every candidate path is made relative to.
    root: &'a str,
    /// Caller working directory, the first operand of every resolution.
    base: &'a str,
    /// Process directories completing what `base` leaves open.
    ctx: &'a ProcessContext<'a>,
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
///
/// A directory that does not exist yields no skills. A path is made relative to
/// `dir` with `base` as the first operand of both resolutions, except that `dir`
/// itself is empty without resolving; the process directories are asked for only
/// when such a resolution leaves a part open. A directory whose entries cannot be
/// located because that read fails ends with the skills gathered before the
/// failure, and the scan never fails.
pub(super) fn scan(
    dir: &str,
    base: &str,
    ctx: &ProcessContext<'_>,
    source: &str,
    operations: &dyn ResourceOperations,
) -> LoadSkillsResult {
    // Candidates are root-relative, so a root of `.` keeps the matcher from stripping a prefix.
    let mut builder = GitignoreBuilder::new(".");
    let _ = builder.case_insensitive(true);
    let mut scan = Discovery {
        root: dir,
        base,
        ctx,
        source,
        operations,
        builder,
        matcher: Gitignore::empty(),
    };
    // The root's own relative path is empty without any resolution, so its rules cannot fail.
    scan.visit(dir, true).unwrap_or_default()
}
impl Discovery<'_> {
    /// Traverse the directory `dir` in adapter order, stopping after an attempted root entry.
    ///
    /// # Errors
    /// Returns the failure to locate `dir` beneath the scan root, which the
    /// calling directory ends with.
    fn visit(&mut self, dir: &str, include_root_files: bool) -> io::Result<LoadSkillsResult> {
        let mut result = LoadSkillsResult::default();
        if self.operations.exists(Path::new(dir)) {
            self.add_rules(dir)?;
            // A failure below ends this directory with what was gathered.
            let _ = self.read_entries(dir, include_root_files, &mut result);
        }
        Ok(result)
    }
    /// Load the entry file, or else every entry, of the existing directory `dir`.
    fn read_entries(
        &mut self,
        dir: &str,
        include_root_files: bool,
        result: &mut LoadSkillsResult,
    ) -> io::Result<()> {
        let entries = self.operations.read_dir(Path::new(dir))?;
        for entry in entries.iter().filter(|e| e.name == "SKILL.md") {
            if let Some((path, ResourceFileType::File)) = self.candidate(dir, entry)? {
                append(result, load_file(&path, self.source, self.operations));
                return Ok(());
            }
        }
        for entry in &entries {
            let name = entry.name.to_string_lossy();
            if name.starts_with('.') || entry.name == "node_modules" {
                continue;
            }
            match self.candidate(dir, entry)? {
                Some((path, ResourceFileType::Directory)) => {
                    append(result, self.visit(&path, false)?);
                }
                Some((path, ResourceFileType::File))
                    if include_root_files && name.ends_with(".md") =>
                {
                    append(result, load_file(&path, self.source, self.operations));
                }
                _ => {}
            }
        }
        Ok(())
    }
    /// The path and kind of `entry` in `dir`, unless it is a broken link or ignored.
    fn candidate(
        &self,
        dir: &str,
        entry: &ResourceEntry,
    ) -> io::Result<Option<(String, ResourceFileType)>> {
        let path = join(&[dir, &entry.name.to_string_lossy()]);
        let Some(kind) = observed_type(entry, &path, self.operations) else {
            return Ok(None);
        };
        let ignored = self.ignored(&path, kind == ResourceFileType::Directory)?;
        Ok((!ignored).then_some((path, kind)))
    }
    /// The path from the scan root to `to`, resolving only when they differ.
    ///
    /// Identical spellings give an empty path without any resolution. Otherwise
    /// both are resolved with `base` as the first operand.
    ///
    /// # Errors
    /// Returns the working-directory error when a resolution needs it and it
    /// cannot be read.
    fn relative(&self, to: &str) -> io::Result<String> {
        if self.root == to {
            return Ok(String::new());
        }
        let from = self.ctx.resolve(&[self.base, self.root])?;
        let to = self.ctx.resolve(&[self.base, to])?;
        // Both ends continue from the same base, which cancels in their difference.
        let shared_base = Cwd {
            current: &from,
            drive_directories: &[],
        };
        Ok(maestro_path::relative(&from, &to, &shared_base))
    }
    /// Check excluded ancestors before a candidate's own negations.
    fn ignored(&self, path: &str, is_dir: bool) -> io::Result<bool> {
        let candidate = self.relative(path)?;
        let mut parent = dirname(&candidate);
        while parent != "." {
            if self.matcher.matched(&parent, true).is_ignore() {
                return Ok(true);
            }
            parent = dirname(&parent);
        }
        Ok(self.matcher.matched(&candidate, is_dir).is_ignore())
    }
    /// Append authored directory-prefixed rules in ignore-file order.
    fn add_rules(&mut self, dir: &str) -> io::Result<()> {
        let prefix = self.relative(dir)?.replace(SEP, "/");
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
        Ok(())
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
