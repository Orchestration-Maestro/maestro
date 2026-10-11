//! Conventional resource collection over replaceable filesystem observations.
use maestro_path::join;
use maestro_pattern::ignore::{Ignore, ScopedIgnoreRules};
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
    Ok(collect_files(
        dir,
        dir,
        Discovery::Conventional(kind),
        operations,
        &mut rules,
    ))
}
/// Recurses through conventional resource directories.
fn collect_files(
    dir: &str,
    root: &str,
    mode: Discovery,
    operations: &dyn ResourceOperations,
    rules: &mut ScopedIgnoreRules,
) -> Vec<String> {
    let kind = mode.kind();
    let mut result = Vec::new();
    if !operations.exists(Path::new(dir)) {
        return result;
    }
    let mut local = Ignore::new();
    load_ignore(&mut local, dir, operations);
    rules.push(relative_path(root, dir), local);
    let entries = operations.read_dir(Path::new(dir)).unwrap_or_default();
    if matches!(kind, ResourceType::Skills)
        && admitted_skill(dir, root, &entries, operations, rules)
    {
        rules.pop();
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
        if rules.ignored(
            &relative_path(root, &path),
            file_type == ResourceFileType::Directory,
        ) {
            continue;
        }
        if file_type == ResourceFileType::Directory && !matches!(mode, Discovery::Shallow(_)) {
            match collect_child(&path, root, mode, operations, rules) {
                Ok(entries) => result.extend(entries),
                Err(_) => break,
            }
        } else if file_type == ResourceFileType::File
            && kind.accepts(&name)
            && (!matches!(kind, ResourceType::Skills)
                || (dir == root && !matches!(mode, Discovery::Agents)))
        {
            result.push(path);
        }
    }
    rules.pop();
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
            .field("extensions")
            .filter(|raw| raw.get().starts_with('['))
    {
        let mut found = Vec::new();
        let entries: Vec<String> = serde_json::from_str(entries.get())
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        for entry in entries {
            let path = resolve_entry_path(dir, &entry, operations)?;
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
/// Reads the three conventional ignore files in declaration order.
fn load_ignore(rules: &mut Ignore, dir: &str, operations: &dyn ResourceOperations) {
    for name in [".gitignore", ".ignore", ".fdignore"] {
        let path = join(&[dir, name]);
        let Ok(text) = operations.read_file(Path::new(&path)) else {
            continue;
        };
        for line in text
            .split('\n')
            .map(|line| line.strip_suffix('\r').unwrap_or(line))
            .filter(|line| !line.trim_matches(crate::whitespace).starts_with('#'))
        {
            rules.add(line);
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
    mode: Discovery,
    operations: &dyn ResourceOperations,
    rules: &mut ScopedIgnoreRules,
) -> io::Result<Vec<String>> {
    if matches!(mode.kind(), ResourceType::Extensions) {
        resolve_extension_entries(dir, operations).map(Option::unwrap_or_default)
    } else {
        Ok(collect_files(dir, root, mode, operations, rules))
    }
}
/// Determines whether a file-like skill entry overrides directory scanning.
fn admitted_skill(
    dir: &str,
    root: &str,
    entries: &[ResourceEntry],
    operations: &dyn ResourceOperations,
    rules: &ScopedIgnoreRules,
) -> bool {
    entries.iter().any(|entry| {
        let path = join(&[dir, "SKILL.md"]);
        entry.name == "SKILL.md"
            && entry_type(entry, &path, operations) == Some(ResourceFileType::File)
            && !rules.ignored(&relative_path(root, &path), false)
    })
}
/// Resolves literal manifest operands, consulting ambient directories only on demand.
pub(super) fn resolve_entry_path(
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

/// Collects files and resource directories from already-resolved literal paths.
pub(super) fn collect_files_from_paths(
    paths: Vec<String>,
    kind: ResourceType,
    operations: &dyn ResourceOperations,
) -> Vec<String> {
    let mut files = Vec::new();
    for path in paths {
        if !operations.exists(Path::new(&path)) {
            continue;
        }
        match operations.metadata(Path::new(&path)) {
            Ok(ResourceFileType::File) => files.push(path),
            Ok(ResourceFileType::Directory) => {
                if let Ok(entries) = collect_resource_files(&path, kind, operations) {
                    files.extend(entries);
                }
            }
            _ => {}
        }
    }
    files
}

/// Scan framing chosen by the resource entry point.
#[derive(Clone, Copy)]
enum Discovery {
    /// Conventional recursive resources.
    Conventional(ResourceType),
    /// Directory-based skill entries without loose root Markdown.
    Agents,
    /// Direct prompt or theme entries only.
    Shallow(ResourceType),
}
impl Discovery {
    /// The resource suffix and terminal entry policy.
    fn kind(self) -> ResourceType {
        match self {
            Self::Conventional(kind) | Self::Shallow(kind) => kind,
            Self::Agents => ResourceType::Skills,
        }
    }
}
/// Collects automatic resources with the root's skill mode and shallow document framing.
pub(super) fn collect_auto_files(
    dir: &str,
    kind: ResourceType,
    agents: bool,
    operations: &dyn ResourceOperations,
) -> io::Result<Vec<String>> {
    if matches!(kind, ResourceType::Extensions) {
        return collect_resource_files(dir, kind, operations);
    }
    let mode = match kind {
        ResourceType::Skills if agents => Discovery::Agents,
        ResourceType::Skills => Discovery::Conventional(kind),
        _ => Discovery::Shallow(kind),
    };
    Ok(collect_files(
        dir,
        dir,
        mode,
        operations,
        &mut ScopedIgnoreRules::default(),
    ))
}
