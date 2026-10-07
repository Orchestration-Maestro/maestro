//! Recursive discovery with shared ignore rules.

use super::{LoadSkillsResult, load_skill_from_file, paths, warning};
use crate::ResourceOperations;

pub(super) fn load_skills_from_dir_internal(
    dir: &str,
    source: &str,
    include_root_files: bool,
    operations: &dyn ResourceOperations,
) -> LoadSkillsResult {
    let mut builder = ignore::gitignore::GitignoreBuilder::new("");
    let _ = builder.case_insensitive(true);
    let mut context = ScanContext {
        source,
        operations,
        root: dir,
        builder,
        matcher: ignore::gitignore::Gitignore::empty(),
    };
    context.scan(dir, include_root_files)
}
struct ScanContext<'a> {
    source: &'a str,
    operations: &'a dyn ResourceOperations,
    root: &'a str,
    builder: ignore::gitignore::GitignoreBuilder,
    matcher: ignore::gitignore::Gitignore,
}
impl ScanContext<'_> {
    fn scan(&mut self, dir: &str, include_root_files: bool) -> LoadSkillsResult {
        let mut result = LoadSkillsResult::empty();
        if !self.operations.exists(dir) {
            return result;
        }
        if add_ignore_rules(&mut self.builder, dir, self.root, self.operations) {
            match self.builder.build() {
                Ok(matcher) => self.matcher = matcher,
                Err(error) => {
                    result.diagnostics.push(warning(dir, error.to_string()));
                    return result;
                }
            }
        }
        let Ok(entries) = self.operations.read_dir(dir) else {
            return result;
        };
        if let Some(path) = entries
            .iter()
            .filter(|entry| entry.name == "SKILL.md")
            .find_map(|entry| self.root_candidate(dir, entry))
        {
            return load_skill_from_file(&path, self.source, self.operations);
        }
        for entry in entries {
            if let Some(sub) = self.load_entry(dir, &entry, include_root_files) {
                result.skills.extend(sub.skills);
                result.diagnostics.extend(sub.diagnostics);
            }
        }
        result
    }
    fn root_candidate(&self, dir: &str, entry: &crate::Dirent) -> Option<String> {
        let path = paths::join(&[dir, &entry.name]);
        let file = if entry.is_symbolic_link {
            self.operations.stat(&path).is_ok_and(|s| s.is_file)
        } else {
            entry.is_file
        };
        (file && !ignored(&self.matcher, &path, self.root, false)).then_some(path)
    }
    // Markdown extensions are deliberately case-sensitive.
    #[allow(clippy::case_sensitive_file_extension_comparisons)]
    fn load_entry(
        &mut self,
        dir: &str,
        entry: &crate::Dirent,
        include_root_files: bool,
    ) -> Option<LoadSkillsResult> {
        if entry.name.starts_with('.') || entry.name == "node_modules" {
            return None;
        }
        let path = paths::join(&[dir, &entry.name]);
        let (directory, file) = if entry.is_symbolic_link {
            let stats = self.operations.stat(&path).ok()?;
            (stats.is_directory, stats.is_file)
        } else {
            (entry.is_directory, entry.is_file)
        };
        if ignored(&self.matcher, &path, self.root, directory) {
            return None;
        }
        if directory {
            Some(self.scan(&path, false))
        } else if file && include_root_files && entry.name.ends_with(".md") {
            Some(load_skill_from_file(&path, self.source, self.operations))
        } else {
            None
        }
    }
}
const IGNORE_FILE_NAMES: [&str; 3] = [".gitignore", ".ignore", ".fdignore"];
fn to_posix_path(value: &str) -> String {
    value.replace(std::path::MAIN_SEPARATOR, "/")
}
fn prefix_ignore_pattern(line: &str, prefix: &str) -> Option<String> {
    let trimmed = paths::trim(line);
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let (negated, pattern) = if let Some(s) = line.strip_prefix('!') {
        (true, s)
    } else {
        (false, line)
    };
    let pattern = pattern.strip_prefix('/').unwrap_or(pattern);
    Some(format!(
        "{}{prefix}{pattern}",
        if negated { "!" } else { "" }
    ))
}
fn add_ignore_rules(
    builder: &mut ignore::gitignore::GitignoreBuilder,
    dir: &str,
    root: &str,
    operations: &dyn ResourceOperations,
) -> bool {
    let mut changed = false;
    let relative = dir
        .strip_prefix(root)
        .unwrap_or(dir)
        .trim_start_matches(std::path::MAIN_SEPARATOR);
    let prefix = if relative.is_empty() {
        String::new()
    } else {
        format!("{}/", to_posix_path(relative))
    };
    for name in IGNORE_FILE_NAMES {
        let path = paths::join(&[dir, name]);
        if !operations.exists(&path) {
            continue;
        }
        let Ok(content) = operations.read_file(&path) else {
            continue;
        };
        let content = content.replace("\r\n", "\n");
        for line in content.split('\n') {
            if let Some(pattern) = prefix_ignore_pattern(line, &prefix) {
                changed |= builder.add_line(None, &pattern).is_ok();
            }
        }
    }
    changed
}
fn ignored(
    matcher: &ignore::gitignore::Gitignore,
    path: &str,
    root: &str,
    directory: bool,
) -> bool {
    let relative = to_posix_path(
        path.strip_prefix(root)
            .unwrap_or(path)
            .trim_start_matches(std::path::MAIN_SEPARATOR),
    );
    matcher
        .matched_path_or_any_parents(&relative, directory)
        .is_ignore()
}
