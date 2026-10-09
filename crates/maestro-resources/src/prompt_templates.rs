//! Ordered prompt-template loading and literal argument expansion.
#![doc = include_str!("../../../docs/resources/prompt-templates.md")]

use crate::{
    FrontmatterValue, ResourceFileType, ResourceOperations, SourceInfo, SourceScope,
    SyntheticSourceOptions, create_synthetic_source_info, frontmatter::text_whitespace,
    parse_frontmatter, skills::ProcessContext,
};
use maestro_path::{basename, dirname, is_absolute, join};
use std::{io, path::Path};

/// Replace original positional and wildcard markers with literal arguments.
#[must_use]
pub fn substitute_args(content: &str, args: &[String]) -> String {
    let mut output = String::new();
    let mut remaining = content;
    while !remaining.is_empty() {
        if let Some((width, selection)) = token(remaining, args.len()) {
            output.push_str(&args[selection].join(" "));
            remaining = &remaining[width..];
        } else if let Some(character) = remaining.chars().next() {
            output.push(character);
            remaining = &remaining[character.len_utf8()..];
        }
    }
    output
}

/// Recognize a marker at the start and select its argument range.
fn token(text: &str, count: usize) -> Option<(usize, std::ops::Range<usize>)> {
    let tail = text.strip_prefix('$')?;
    if tail.starts_with("ARGUMENTS") {
        return Some((10, 0..count));
    }
    if tail.starts_with('@') {
        return Some((2, 0..count));
    }
    if let Some(slice) = tail.strip_prefix("{@:") {
        let (width, index) = decimal(slice, count.saturating_add(1))?;
        let start = index.saturating_sub(1).min(count);
        let suffix = &slice[width..];
        let (extra, end) = if let Some(length) = suffix.strip_prefix(':') {
            let (digits, length) = decimal(length, count)?;
            (digits + 1, start.saturating_add(length).min(count))
        } else {
            (0, count)
        };
        if slice[width + extra..].starts_with('}') {
            return Some((width + extra + 5, start..end));
        }
        return None;
    }
    let (width, index) = decimal(tail, count.saturating_add(1))?;
    let start = index.saturating_sub(1).min(count);
    let end = if index == 0 {
        start
    } else {
        (start + 1).min(count)
    };
    Some((width + 1, start..end))
}

/// Consume an entire ASCII decimal, saturating at the useful selection bound.
fn decimal(text: &str, cap: usize) -> Option<(usize, usize)> {
    let digits = text.bytes().take_while(u8::is_ascii_digit);
    let mut width = 0;
    let mut value = 0usize;
    for digit in digits {
        width += 1;
        value = value
            .saturating_mul(10)
            .saturating_add(usize::from(digit - b'0'))
            .min(cap);
    }
    (width > 0).then_some((width, value))
}

/// Split on unquoted ASCII spaces and tabs, removing quote delimiters.
#[must_use]
pub fn parse_command_args(args_string: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    for character in args_string.chars() {
        match (quote, character) {
            (Some(delimiter), c) if delimiter == c => quote = None,
            (None, c @ ('\'' | '"')) => quote = Some(c),
            (None, ' ' | '\t') => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            (_, c) => current.push(c),
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

/// A loaded prompt with authored text and provenance.
#[derive(Debug)]
pub struct PromptTemplate {
    /// File basename with one terminal `.md` removed.
    pub name: String,
    /// Authored description or a body-line preview.
    pub description: String,
    /// Nonempty authored argument hint.
    pub argument_hint: Option<String>,
    /// Body after shared frontmatter parsing.
    pub content: String,
    /// Authored-path provenance.
    pub source_info: SourceInfo,
    /// File spelling used for the read.
    pub file_path: String,
}

/// Expand the first exact template name following a leading slash.
#[must_use]
pub fn expand_prompt_template(text: &str, templates: &[PromptTemplate]) -> String {
    let Some(command) = text.strip_prefix('/') else {
        return text.into();
    };
    let (name, arguments) = command.split_once(' ').unwrap_or((command, ""));
    templates
        .iter()
        .find(|template| template.name == name)
        .map_or_else(
            || text.into(),
            |template| substitute_args(&template.content, &parse_command_args(arguments)),
        )
}

/// Caller-selected roots and ordered explicit prompt paths.
#[derive(Clone, Copy)]
pub struct LoadPromptTemplatesOptions<'a> {
    /// Working directory operand for project and explicit paths.
    pub cwd: &'a str,
    /// Home spelling used by tilde expansion.
    pub home: &'a str,
    /// User configuration directory.
    pub agent_dir: &'a str,
    /// Project configuration directory name.
    pub config_dir_name: &'a str,
    /// Ordered explicit files or directories.
    pub prompt_paths: &'a [String],
    /// Whether to scan user and project defaults.
    pub include_defaults: bool,
}

/// Read a file and project only the selected metadata fields.
fn load_file(
    path: &str,
    source_info: SourceInfo,
    operations: &dyn ResourceOperations,
) -> Option<PromptTemplate> {
    let raw = operations.read_file(Path::new(path)).ok()?;
    let parsed = parse_frontmatter(&raw).ok()?;
    let description = selected_string(&parsed.frontmatter, "description")
        .ok()?
        .unwrap_or_default();
    let hint = selected_string(&parsed.frontmatter, "argument-hint").ok()?;
    let name = basename(path, None);
    Some(PromptTemplate {
        name: name.strip_suffix(".md").unwrap_or(&name).into(),
        description: if description.is_empty() {
            preview(&parsed.body)
        } else {
            description.into()
        },
        argument_hint: hint.filter(|hint| !hint.is_empty()).map(str::to_owned),
        content: parsed.body,
        source_info,
        file_path: path.into(),
    })
}

/// Accept absent/null/string selected fields without validating unrelated values.
fn selected_string<'a>(metadata: &'a FrontmatterValue, key: &str) -> io::Result<Option<&'a str>> {
    let FrontmatterValue::Mapping(fields) = metadata else {
        return Ok(None);
    };
    match fields
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value)
    {
        None | Some(FrontmatterValue::Null) => Ok(None),
        Some(FrontmatterValue::String(value)) => Ok(Some(value)),
        Some(_) => Err(io::ErrorKind::InvalidData.into()),
    }
}

/// Preview the first nonblank body line without splitting a scalar.
fn preview(body: &str) -> String {
    let line = body
        .split('\n')
        .find(|line| !line.trim_matches(text_whitespace).is_empty())
        .unwrap_or_default();
    let mut characters = line.chars();
    let mut description: String = characters.by_ref().take(60).collect();
    if characters.next().is_some() {
        description.push_str("...");
    }
    description
}

/// Roots and lazy process context shared by one load.
struct TemplateLoader<'a> {
    /// User prompt directory in supplied spelling.
    user: String,
    /// Resolved project prompt directory.
    project: String,
    /// Filesystem observation port.
    operations: &'a dyn ResourceOperations,
    /// Lazily observed process directories.
    context: ProcessContext<'a>,
}
impl TemplateLoader<'_> {
    /// Test authored spelling against a resolved root, with a separator boundary.
    fn under(&self, path: &str, root: &str) -> io::Result<bool> {
        let mut root = self.context.resolve(&[root])?;
        if path == root {
            return Ok(true);
        }
        if !root.ends_with(maestro_path::SEP) {
            root.push(maestro_path::SEP);
        }
        Ok(path.starts_with(&root))
    }
    /// Classify user before project, falling back to source metadata.
    fn source(&self, path: &str) -> io::Result<SourceInfo> {
        let (scope, base) = if self.under(path, &self.user)? {
            (Some(SourceScope::User), self.user.clone())
        } else if self.under(path, &self.project)? {
            (Some(SourceScope::Project), self.project.clone())
        } else {
            let base = if self.operations.metadata(Path::new(path))? == ResourceFileType::Directory
            {
                path.into()
            } else {
                dirname(path)
            };
            (None, base)
        };
        Ok(create_synthetic_source_info(
            path.into(),
            SyntheticSourceOptions {
                source: "local".into(),
                scope,
                origin: None,
                base_dir: Some(base),
            },
        ))
    }
}

/// Scan immediate markdown files, retaining partial results when source lookup fails.
fn scan(dir: &str, loader: &TemplateLoader<'_>, templates: &mut Vec<PromptTemplate>) {
    let operations = loader.operations;
    if !operations.exists(Path::new(dir)) {
        return;
    }
    let Ok(entries) = operations.read_dir(Path::new(dir)) else {
        return;
    };
    for entry in entries {
        let name = entry.name.to_string_lossy();
        let path = join(&[dir, &name]);
        let kind = if entry.file_type == ResourceFileType::Symlink {
            let Ok(kind) = operations.metadata(Path::new(&path)) else {
                continue;
            };
            kind
        } else {
            entry.file_type
        };
        if kind == ResourceFileType::File && name.ends_with(".md") {
            let Ok(info) = loader.source(&path) else {
                return;
            };
            if let Some(template) = load_file(&path, info, operations) {
                templates.push(template);
            }
        }
    }
}

/// Load user, project and explicit templates in discovery order.
///
/// # Errors
/// Returns a working-directory error when project or explicit resolution requires it.
pub fn load_prompt_templates(
    options: LoadPromptTemplatesOptions<'_>,
    operations: &dyn ResourceOperations,
) -> io::Result<Vec<PromptTemplate>> {
    let context = ProcessContext::new(operations);
    let loader = TemplateLoader {
        user: if options.agent_dir.is_empty() {
            String::new()
        } else {
            join(&[options.agent_dir, "prompts"])
        },
        project: context.resolve(&[options.cwd, options.config_dir_name, "prompts"])?,
        context,
        operations,
    };
    let mut templates = Vec::new();
    if options.include_defaults {
        scan(&loader.user, &loader, &mut templates);
        scan(&loader.project, &loader, &mut templates);
    }
    for path in options.prompt_paths {
        let path = resolve_prompt_path(path, options, &loader.context)?;
        if !operations.exists(Path::new(&path)) {
            continue;
        }
        match operations.metadata(Path::new(&path)) {
            Ok(ResourceFileType::Directory) => scan(&path, &loader, &mut templates),
            Ok(ResourceFileType::File) if path.strip_suffix(".md").is_some() => {
                if let Ok(info) = loader.source(&path)
                    && let Some(template) = load_file(&path, info, operations)
                {
                    templates.push(template);
                }
            }
            _ => {}
        }
    }
    Ok(templates)
}

/// Normalize tilde spelling before resolving an unanchored explicit path.
fn resolve_prompt_path(
    path: &str,
    options: LoadPromptTemplatesOptions<'_>,
    context: &ProcessContext<'_>,
) -> io::Result<String> {
    let path = path.trim_matches(text_whitespace);
    let normalized = if path == "~" {
        options.home.into()
    } else if let Some(suffix) = path.strip_prefix("~/").or_else(|| path.strip_prefix('~')) {
        join(&[options.home, suffix])
    } else {
        path.into()
    };
    if is_absolute(&normalized) {
        Ok(normalized)
    } else {
        context.resolve(&[options.cwd, &normalized])
    }
}
