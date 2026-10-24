//! Ordered directory traversal of instruction entry files.
use super::{
    LoadSkillsResult, ResourceEntry, ResourceFileType, files, load_file, metadata, read_text,
    resolve,
};
use maestro_path::{Cwd, SEP, join};
use maestro_pattern::ignore::{Ignore, ScopedIgnoreRules};
use std::{io, path::Path};

/// Shared scan-local observations and ignore-rule state.
struct Discovery<'a> {
    /// The authored scan directory, which every candidate path is made relative to.
    root: &'a str,
    /// Caller working directory, the first operand of every resolution.
    base: &'a str,
    /// Source label for every discovery.
    source: &'a str,
    /// Rules on the active directory branch.
    rules: ScopedIgnoreRules,
}
/// Scan nested directories, preferring each directory's entry file.
///
/// A directory that does not exist yields no skills. A path is made relative to
/// `dir` with `base` as the first operand of both resolutions, except that `dir`
/// itself is empty without resolving; the process directories are asked for only
/// when such a resolution leaves a part open. A directory whose entries cannot be
/// located because that read fails ends with the skills gathered before the
/// failure, and the scan never fails.
pub(super) fn scan(dir: &str, base: &str, source: &str) -> LoadSkillsResult {
    let mut scan = Discovery {
        root: dir,
        base,
        source,
        rules: ScopedIgnoreRules::default(),
    };
    // The root's own relative path is empty without any resolution, so its rules cannot fail.
    scan.visit(dir, true).unwrap_or_default()
}
impl Discovery<'_> {
    /// Traverse the directory `dir` in name order, stopping after an attempted root entry.
    ///
    /// # Errors
    /// Returns the failure to locate `dir` beneath the scan root, which the
    /// calling directory ends with.
    fn visit(&mut self, dir: &str, include_root_files: bool) -> io::Result<LoadSkillsResult> {
        let mut result = LoadSkillsResult::default();
        if Path::new(dir).exists() {
            self.add_rules(dir)?;
            // A failure below ends this directory with what was gathered.
            let _ = self.read_entries(dir, include_root_files, &mut result);
            self.rules.pop();
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
        let entries = files::read_entries(Path::new(dir))?;
        for entry in entries.iter().filter(|e| e.name == "SKILL.md") {
            if let Some((path, ResourceFileType::File)) = self.candidate(dir, entry)? {
                append(result, load_file(&path, self.source));
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
                    append(result, load_file(&path, self.source));
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
        let Some(kind) = observed_type(entry, &path) else {
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
        let from = resolve(&[self.base, self.root])?;
        let to = resolve(&[self.base, to])?;
        // Both ends continue from the same base, which cancels in their difference.
        let shared_base = Cwd {
            current: &from,
            drive_directories: &[],
        };
        Ok(maestro_path::relative(&from, &to, &shared_base))
    }
    /// Applies directory-local rules after traversal has admitted the parent.
    fn ignored(&self, path: &str, is_dir: bool) -> io::Result<bool> {
        let candidate = self.relative(path)?.replace(SEP, "/");
        Ok(self.rules.ignored(&candidate, is_dir))
    }
    /// Compiles one directory's authored rules in ignore-file order.
    fn add_rules(&mut self, dir: &str) -> io::Result<()> {
        let scope = self.relative(dir)?.replace(SEP, "/");
        let mut rules = Ignore::new();
        for filename in [".gitignore", ".ignore", ".fdignore"] {
            let path = join(&[dir, filename]);
            if !Path::new(&path).exists() {
                continue;
            }
            let Ok(text) = read_text(Path::new(&path)) else {
                continue;
            };
            for line in text
                .split('\n')
                .map(|s| s.strip_suffix('\r').unwrap_or(s))
                .filter(|line| {
                    let trimmed = line.trim_matches(crate::metadata::text_whitespace);
                    !trimmed.is_empty() && !trimmed.starts_with('#')
                })
            {
                rules.add(line);
            }
        }
        self.rules.push(scope, rules);
        Ok(())
    }
}
/// Returns a non-link's listed kind without reading metadata; for a link, returns the kind of its
/// target, or `None` if that read fails.
#[must_use]
pub fn observed_type(entry: &ResourceEntry, path: &str) -> Option<ResourceFileType> {
    if entry.file_type == ResourceFileType::Symlink {
        metadata(Path::new(path)).ok()
    } else {
        Some(entry.file_type)
    }
}
/// Append ordered discoveries and diagnostics without copying resources.
fn append(result: &mut LoadSkillsResult, next: LoadSkillsResult) {
    result.skills.extend(next.skills);
    result.diagnostics.extend(next.diagnostics);
}
