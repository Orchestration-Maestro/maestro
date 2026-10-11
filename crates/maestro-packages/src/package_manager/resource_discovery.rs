//! Conventional resource collection over replaceable filesystem observations.
use crate::ignore::{Ignore, TestResult};
use maestro_path::join;
use maestro_resources::skills::{ResourceEntry, ResourceFileType, ResourceOperations};
use std::{io, path::Path};
/// The conventional resource directories.
#[derive(Clone, Copy)]
pub(super) enum ResourceType {
    /// Extension entries.
    Extensions,
    /// Skill entries.
    Skills,
    /// Prompt templates.
    Prompts,
    /// Theme files.
    Themes,
}
impl ResourceType {
    /// Discovery order.
    pub(super) const ALL: [Self; 4] = [Self::Extensions, Self::Skills, Self::Prompts, Self::Themes];
    /// The directory for this kind.
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Extensions => "extensions",
            Self::Skills => "skills",
            Self::Prompts => "prompts",
            Self::Themes => "themes",
        }
    }
}
/// Collects resources according to their kind and extension entry points.
pub(super) fn collect_resource_files(
    dir: &str,
    kind: ResourceType,
    operations: &dyn ResourceOperations,
) -> io::Result<Vec<String>> {
    if !operations.exists(Path::new(dir)) {
        return Ok(Vec::new());
    }
    if matches!(kind, ResourceType::Extensions)
        && let Some(entries) = resolve_extension_entries(dir, operations)?
    {
        return Ok(entries);
    }
    let mut rules = ScopedIgnoreRules::default();
    Ok(collect_files(dir, dir, kind, operations, &mut rules))
}
/// Recurses through conventional resource directories.
fn collect_files(
    dir: &str,
    root: &str,
    kind: ResourceType,
    operations: &dyn ResourceOperations,
    rules: &mut ScopedIgnoreRules,
) -> Vec<String> {
    let mut result = Vec::new();
    if !operations.exists(Path::new(dir)) {
        return result;
    }
    let mut local = Ignore::new();
    local.load(dir, operations);
    rules.stack.push((dir.to_owned(), local));
    let entries = operations.read_dir(Path::new(dir)).unwrap_or_default();
    if matches!(kind, ResourceType::Skills) && admitted_skill(dir, &entries, operations, rules) {
        rules.stack.pop();
        return vec![join(&[dir, "SKILL.md"])];
    }
    for entry in entries {
        let name = entry.name.to_string_lossy();
        if name.starts_with('.') || name == "node_modules" {
            continue;
        }
        let path = join(&[dir, &name]);
        let Some(file_type) = entry_type(&entry, &path, operations) else {
            continue;
        };
        if rules.ignored(&path, file_type == ResourceFileType::Directory) {
            continue;
        }
        if file_type == ResourceFileType::Directory {
            match collect_child(&path, root, kind, operations, rules) {
                Ok(entries) => result.extend(entries),
                Err(_) => break,
            }
        } else if file_type == ResourceFileType::File
            && kind.accepts(&name)
            && (!matches!(kind, ResourceType::Skills) || dir == root)
        {
            result.push(path);
        }
    }
    rules.stack.pop();
    result
}
impl ResourceType {
    /// The source suffix admitted by this collector.
    fn accepts(self, name: &str) -> bool {
        match self {
            Self::Extensions => name
                .rsplit_once('.')
                .is_some_and(|(_, extension)| matches!(extension, "ts" | "js")),
            Self::Skills | Self::Prompts => name
                .rsplit_once('.')
                .is_some_and(|(_, extension)| extension == "md"),
            Self::Themes => name
                .rsplit_once('.')
                .is_some_and(|(_, extension)| extension == "json"),
        }
    }
}

/// Selects a manifest entry list or the first existing index.
fn resolve_extension_entries(
    dir: &str,
    operations: &dyn ResourceOperations,
) -> io::Result<Option<Vec<String>>> {
    let manifest_path = join(&[dir, "package.json"]);
    if operations.exists(Path::new(&manifest_path))
        && let Some(manifest) =
            super::manifest::read_maestro_manifest_file(&manifest_path, operations)
        && let Some(entries) = manifest
            .get("extensions")
            .and_then(serde_json::Value::as_array)
    {
        let mut found = Vec::new();
        for entry in entries {
            let entry = entry.as_str().ok_or(io::ErrorKind::InvalidData)?;
            let path = resolve_entry_path(dir, entry, operations)?;
            if operations.exists(Path::new(&path)) {
                found.push(path);
            }
        }
        if !found.is_empty() {
            return Ok(Some(found));
        }
    }
    Ok(["index.ts", "index.js"]
        .into_iter()
        .map(|name| join(&[dir, name]))
        .find(|path| operations.exists(Path::new(path)))
        .map(|path| vec![path]))
}

/// Converts the native lexical relative path to rule separators.
fn relative_path(root: &str, path: &str) -> String {
    let path = maestro_path::relative(
        root,
        path,
        &maestro_path::Cwd {
            current: "",
            drive_directories: &[],
        },
    );
    if cfg!(windows) {
        path.replace('\\', "/")
    } else {
        path
    }
}
/// Matchers scoped to the directories on the active traversal branch.
#[derive(Default)]
struct ScopedIgnoreRules {
    /// Ancestor matchers followed by the current directory's matcher.
    stack: Vec<(String, Ignore)>,
}
impl ScopedIgnoreRules {
    /// The deepest matching rule decides; pruned parents are never traversed.
    fn ignored(&self, path: &str, directory: bool) -> bool {
        self.stack
            .iter()
            .rev()
            .find_map(
                |(dir, rules)| match rules.test(&relative_path(dir, path), directory) {
                    TestResult::Unmatched => None,
                    TestResult::Ignored => Some(true),
                    TestResult::Unignored => Some(false),
                },
            )
            .unwrap_or(false)
    }
}
impl Ignore {
    /// Reads the three conventional ignore files in declaration order.
    fn load(&mut self, dir: &str, operations: &dyn ResourceOperations) {
        for name in [".gitignore", ".ignore", ".fdignore"] {
            let path = join(&[dir, name]);
            let Ok(text) = operations.read_file(Path::new(&path)) else {
                continue;
            };
            for line in text
                .strip_prefix('\u{feff}')
                .unwrap_or(&text)
                .split('\n')
                .map(|line| line.strip_suffix('\r').unwrap_or(line))
            {
                self.add(line);
            }
        }
    }
}

/// Follows only link entries, skipping failed target observations.
fn entry_type(
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

/// Selects a child entry list or recursively collects its conventional suffix kind.
fn collect_child(
    dir: &str,
    root: &str,
    kind: ResourceType,
    operations: &dyn ResourceOperations,
    rules: &mut ScopedIgnoreRules,
) -> io::Result<Vec<String>> {
    if matches!(kind, ResourceType::Extensions) {
        resolve_extension_entries(dir, operations).map(Option::unwrap_or_default)
    } else {
        Ok(collect_files(dir, root, kind, operations, rules))
    }
}
/// Determines whether a file-like skill entry overrides directory scanning.
fn admitted_skill(
    dir: &str,
    entries: &[ResourceEntry],
    operations: &dyn ResourceOperations,
    rules: &ScopedIgnoreRules,
) -> bool {
    entries.iter().any(|entry| {
        let path = join(&[dir, "SKILL.md"]);
        entry.name == "SKILL.md"
            && entry_type(entry, &path, operations) == Some(ResourceFileType::File)
            && !rules.ignored(&path, false)
    })
}
/// Resolves literal manifest operands, consulting ambient directories only on demand.
fn resolve_entry_path(
    dir: &str,
    entry: &str,
    operations: &dyn ResourceOperations,
) -> io::Result<String> {
    let operands = [dir, entry];
    if let Ok(path) = maestro_path::try_resolve(&operands, &[]) {
        return Ok(path);
    }
    let directories = operations.drive_directories();
    let drives: Vec<_> = directories
        .iter()
        .map(|(drive, path)| (*drive, path.as_str()))
        .collect();
    if let Ok(path) = maestro_path::try_resolve(&operands, &drives) {
        return Ok(path);
    }
    let current = operations.current_directory()?;
    Ok(maestro_path::resolve(
        &operands,
        &maestro_path::Cwd {
            current: &current,
            drive_directories: &drives,
        },
    ))
}
