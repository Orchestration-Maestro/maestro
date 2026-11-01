//! Conventional resource collection over replaceable filesystem observations.
use maestro_path::join;
use maestro_pattern::ignore::{Ignore, ScopedIgnoreRules};
use maestro_resources::package_manifest::{is_extension_file, resolve_extension_entries};
use maestro_resources::skills::{ResourceEntry, ResourceFileType, observed_type};
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
    /// Browser contribution files.
    Contributions,
}
impl ResourceType {
    /// Discovery order.
    pub(super) const ALL: [Self; 5] = [
        Self::Extensions,
        Self::Skills,
        Self::Prompts,
        Self::Themes,
        Self::Contributions,
    ];
    /// The directory for this kind.
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Extensions => "extensions",
            Self::Skills => "skills",
            Self::Prompts => "prompts",
            Self::Themes => "themes",
            Self::Contributions => "contributions",
        }
    }
}
/// Collects resources according to their kind and extension entry points.
pub(super) fn collect_resource_files(dir: &str, kind: ResourceType) -> io::Result<Vec<String>> {
    if !Path::new(dir).exists() {
        return Ok(Vec::new());
    }
    if matches!(kind, ResourceType::Extensions)
        && let Some(entries) = resolve_extension_entries(dir, true)?
    {
        return Ok(entries);
    }
    let mut rules = ScopedIgnoreRules::default();
    Ok(collect_files(
        dir,
        dir,
        Discovery::Conventional(kind),
        &mut rules,
    ))
}
/// Recurses through conventional resource directories.
fn collect_files(
    dir: &str,
    root: &str,
    mode: Discovery,
    rules: &mut ScopedIgnoreRules,
) -> Vec<String> {
    let kind = mode.kind();
    let mut result = Vec::new();
    if !Path::new(dir).exists() {
        return result;
    }
    let mut local = Ignore::new();
    load_ignore(&mut local, dir);
    rules.push(relative_path(root, dir), local);
    let entries = maestro_resources::read_entries(Path::new(dir)).unwrap_or_default();
    if matches!(kind, ResourceType::Skills) && admitted_skill(dir, root, &entries, rules) {
        rules.pop();
        return vec![join(&[dir, "SKILL.md"])];
    }
    for entry in entries {
        let name = entry.name.to_string_lossy();
        if name.starts_with('.') || name == "node_modules" {
            continue;
        }
        let path = join(&[dir, &name]);
        let Some(file_type) = observed_type(&entry, &path) else {
            continue;
        };
        if rules.ignored(
            &relative_path(root, &path),
            file_type == ResourceFileType::Directory,
        ) {
            continue;
        }
        if file_type == ResourceFileType::Directory && !matches!(mode, Discovery::Shallow(_)) {
            match collect_child(&path, root, mode, rules) {
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
            Self::Extensions => is_extension_file(name),
            Self::Skills | Self::Prompts => name
                .rsplit_once('.')
                .is_some_and(|(_, extension)| extension == "md"),
            Self::Themes | Self::Contributions => name
                .rsplit_once('.')
                .is_some_and(|(_, extension)| extension == "json"),
        }
    }
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
fn load_ignore(rules: &mut Ignore, dir: &str) {
    for name in [".gitignore", ".ignore", ".fdignore"] {
        let path = join(&[dir, name]);
        let Ok(text) = maestro_resources::read_text(Path::new(&path)) else {
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

/// Selects a child entry list or recursively collects its conventional suffix kind.
fn collect_child(
    dir: &str,
    root: &str,
    mode: Discovery,
    rules: &mut ScopedIgnoreRules,
) -> io::Result<Vec<String>> {
    if matches!(mode.kind(), ResourceType::Extensions) {
        resolve_extension_entries(dir, true).map(Option::unwrap_or_default)
    } else {
        Ok(collect_files(dir, root, mode, rules))
    }
}
/// Determines whether a file-like skill entry overrides directory scanning.
fn admitted_skill(
    dir: &str,
    root: &str,
    entries: &[ResourceEntry],
    rules: &ScopedIgnoreRules,
) -> bool {
    entries.iter().any(|entry| {
        let path = join(&[dir, "SKILL.md"]);
        entry.name == "SKILL.md"
            && observed_type(entry, &path) == Some(ResourceFileType::File)
            && !rules.ignored(&relative_path(root, &path), false)
    })
}
/// Collects files and resource directories from already-resolved literal paths.
pub(super) fn collect_files_from_paths(paths: Vec<String>, kind: ResourceType) -> Vec<String> {
    let mut files = Vec::new();
    for path in paths {
        if !Path::new(&path).exists() {
            continue;
        }
        match maestro_resources::metadata(Path::new(&path)) {
            Ok(ResourceFileType::File) => files.push(path),
            Ok(ResourceFileType::Directory) => {
                if let Ok(entries) = collect_resource_files(&path, kind) {
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
) -> io::Result<Vec<String>> {
    if matches!(kind, ResourceType::Extensions) {
        return collect_resource_files(dir, kind);
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
        &mut ScopedIgnoreRules::default(),
    ))
}
