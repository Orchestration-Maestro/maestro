//! Conventional resource collection over replaceable filesystem observations.
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
    let mut local = IgnoreRules::new();
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
    stack: Vec<(String, IgnoreRules)>,
}
impl ScopedIgnoreRules {
    /// The deepest matching rule decides; pruned parents are never traversed.
    fn ignored(&self, path: &str, directory: bool) -> bool {
        self.stack
            .iter()
            .rev()
            .find_map(|(dir, rules)| {
                let decision = rules
                    .matcher
                    .matched(fold_non_ascii(&relative_path(dir, path)), directory);
                (!decision.is_none()).then(|| decision.is_ignore())
            })
            .unwrap_or(false)
    }
}
/// Ordered ignore rules belonging to one directory.
pub(super) struct IgnoreRules {
    /// Incrementally accumulated rules.
    builder: ignore::gitignore::GitignoreBuilder,
    /// The compiled rules after the latest directory load.
    matcher: ignore::gitignore::Gitignore,
}
impl IgnoreRules {
    /// Starts an ASCII case-insensitive matcher for non-ASCII folded input.
    pub(super) fn new() -> Self {
        let mut builder = ignore::gitignore::GitignoreBuilder::new("");
        let _ = builder.case_insensitive(true);
        Self {
            builder,
            matcher: ignore::gitignore::Gitignore::empty(),
        }
    }
    /// Adds a rule without replacing accumulated rules; invalid lines are skipped.
    pub(super) fn add(&mut self, rule: &str) {
        let _ = self
            .builder
            .add_line(None, &fold_non_ascii(&encode_ignore_rule(rule)));
    }
    /// Publishes accumulated rules when they compile.
    pub(super) fn compile(&mut self) {
        if let Ok(matcher) = self.builder.build() {
            self.matcher = matcher;
        }
    }
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
        self.compile();
    }
    /// Tests parent exclusions before the entry itself.
    #[cfg(test)]
    pub(super) fn ignored(&self, path: &str, directory: bool) -> bool {
        Path::new(path)
            .ancestors()
            .skip(1)
            .filter(|path| !path.as_os_str().is_empty())
            .any(|parent| {
                self.matcher
                    .matched(fold_non_ascii(&parent.to_string_lossy()), true)
                    .is_ignore()
            })
            || self
                .matcher
                .matched(fold_non_ascii(path), directory)
                .is_ignore()
    }
}
/// Folds non-ASCII characters while preserving native ASCII syntax and case matching.
fn fold_non_ascii(text: &str) -> String {
    let mut folded = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_ascii() {
            folded.push(c);
        } else {
            folded.extend(c.to_lowercase());
        }
    }
    folded
}
/// Encodes literal braces and C-locale named bracket classes for the native matcher.
fn encode_ignore_rule(pattern: &str) -> String {
    let mut result = String::new();
    let mut escaped = false;
    let mut bracket = false;
    let mut chars = pattern.char_indices().peekable();
    while let Some((index, c)) = chars.next() {
        if escaped {
            result.push(c);
            escaped = false;
            continue;
        }
        if bracket
            && c == '['
            && let Some((length, range)) = named_class(&pattern[index..])
        {
            result.push_str(range);
            for _ in 1..length {
                chars.next();
            }
            continue;
        }
        match c {
            '\\' => {
                result.push(c);
                escaped = true;
            }
            '[' => {
                bracket = true;
                result.push(c);
            }
            ']' => {
                bracket = false;
                result.push(c);
            }
            '{' | '}' if !bracket => {
                result.push('\\');
                result.push(c);
            }
            _ => result.push(c),
        }
    }
    result
}
/// Selects the twelve Git C-locale classes without adding slash or a leading negation.
fn named_class(pattern: &str) -> Option<(usize, &'static str)> {
    [
        ("[:alnum:]", "a-zA-Z0-9"),
        ("[:alpha:]", "a-zA-Z"),
        ("[:blank:]", " \t"),
        ("[:cntrl:]", "\u{0}-\u{1f}\u{7f}"),
        ("[:digit:]", "0-9"),
        ("[:graph:]", "#-~!\""),
        ("[:lower:]", "a-z"),
        ("[:print:]", " -~"),
        ("[:punct:]", ":-@[-`{-~!-."),
        ("[:space:]", " \t\r\n\u{b}\u{c}"),
        ("[:upper:]", "A-Z"),
        ("[:xdigit:]", "a-fA-F0-9"),
    ]
    .into_iter()
    .find_map(|(name, range)| pattern.starts_with(name).then_some((name.len(), range)))
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
