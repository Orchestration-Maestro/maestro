//! Discovery of supplied skill resources.

use crate::{ResourceDiagnostic, SourceInfo};

/// A retained Markdown instruction resource.
#[derive(Debug, Clone, PartialEq)]
pub struct Skill {
    /// Declared name or directory basename.
    pub name: String,
    /// Description retained without trimming.
    pub description: String,
    /// Path of the instruction file.
    pub file_path: String,
    /// Base directory for relative resource references.
    pub base_dir: String,
    /// Provenance of the retained file.
    pub source_info: SourceInfo,
    /// Whether the skill is omitted from model prompts.
    pub disable_model_invocation: bool,
}

/// Ordered skills with warnings and collisions.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadSkillsResult {
    /// Retained skills in discovery order.
    pub skills: Vec<Skill>,
    /// Ordered discovery diagnostics.
    pub diagnostics: Vec<ResourceDiagnostic>,
}

impl LoadSkillsResult {
    fn empty() -> Self {
        Self {
            skills: vec![],
            diagnostics: vec![],
        }
    }
}
/// Supplied directory and provenance for one scan.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoadSkillsFromDirOptions<'a> {
    /// Directory to scan.
    pub dir: &'a str,
    /// Caller-supplied source identifier.
    pub source: &'a str,
}

/// Caller-resolved locations for ordered skill discovery.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoadSkillsOptions<'a> {
    /// Working directory for project paths.
    pub cwd: &'a str,
    /// User resource directory supplied by the caller.
    pub agent_dir: &'a str,
    /// Explicit files or directories in input order.
    pub skill_paths: &'a [String],
    /// Whether to scan user and project directories first.
    pub include_defaults: bool,
    /// Project configuration directory name.
    pub config_dir_name: &'a str,
    /// Home directory used for tilde expansion.
    pub home: &'a str,
}

mod discovery;
use discovery::load_skills_from_dir_internal;
pub(crate) mod diagnostics;
pub(crate) mod frontmatter;
pub(crate) mod operations;
pub(crate) mod paths;
use crate::{
    FrontmatterValue, ResourceError, ResourceOperations, create_synthetic_source_info,
    parse_frontmatter,
};

/// Loads a supplied directory without cross-source deduplication.
///
/// # Errors
/// Scan failures are caught as diagnostics or empty results; none propagate.
pub fn load_skills_from_dir(
    options: LoadSkillsFromDirOptions<'_>,
    operations: &dyn ResourceOperations,
) -> Result<LoadSkillsResult, ResourceError> {
    Ok(load_skills_from_dir_internal(
        options.dir,
        options.source,
        true,
        operations,
    ))
}
fn load_skill_from_file(
    file: &str,
    source: &str,
    operations: &dyn ResourceOperations,
) -> LoadSkillsResult {
    let mut result = LoadSkillsResult {
        skills: vec![],
        diagnostics: vec![],
    };
    match read_skill(file, source, operations, &mut result.diagnostics) {
        Ok(Some(skill)) => result.skills.push(skill),
        Ok(None) => {}
        Err(error) => result.diagnostics.push(warning(file, error.to_string())),
    }
    result
}
fn read_skill(
    file: &str,
    source: &str,
    operations: &dyn ResourceOperations,
    diagnostics: &mut Vec<ResourceDiagnostic>,
) -> Result<Option<Skill>, ResourceError> {
    let content = operations.read_file(file).map_err(|error| ResourceError {
        message: Some(
            error
                .message
                .unwrap_or_else(|| "failed to parse skill file".into()),
        ),
    })?;
    let parsed = parse_frontmatter(&content)?;
    let description = metadata_string(&parsed.frontmatter, "description", "")?;
    let dir = paths::dirname(file);
    let parent = paths::basename(&dir);
    diagnostics.extend(
        validate_description(description)
            .into_iter()
            .map(|m| warning(file, m)),
    );
    let name = metadata_string(&parsed.frontmatter, "name", &parent)?;
    diagnostics.extend(
        validate_name(name, &parent)
            .into_iter()
            .map(|m| warning(file, m)),
    );
    if !paths::trim(description).is_empty() {
        let source_info = create_skill_source_info(file, &dir, source);
        let disable_model_invocation = matches!(
            property(&parsed.frontmatter, "disable-model-invocation"),
            Some(FrontmatterValue::Bool(true))
        );
        return Ok(Some(Skill {
            name: name.into(),
            description: description.into(),
            file_path: file.into(),
            base_dir: dir,
            source_info,
            disable_model_invocation,
        }));
    }
    Ok(None)
}
fn metadata_string<'a>(
    metadata: &'a FrontmatterValue,
    key: &str,
    fallback: &'a str,
) -> Result<&'a str, ResourceError> {
    match property(metadata, key).filter(|value| truthy(value)) {
        None => Ok(fallback),
        Some(value) => string(value).ok_or_else(|| ResourceError {
            message: Some(format!("skill {key} must be a string")),
        }),
    }
}
fn create_skill_source_info(file: &str, dir: &str, source: &str) -> SourceInfo {
    match source {
        "user" | "project" => {
            create_synthetic_source_info(file, "local", Some(source), None, Some(dir))
        }
        "path" => create_synthetic_source_info(file, "local", None, None, Some(dir)),
        _ => create_synthetic_source_info(file, source, None, None, Some(dir)),
    }
}

/// Renders visible skills as an escaped XML list.
#[must_use]
pub fn format_skills_for_prompt(skills: &[Skill]) -> String {
    let visible = skills
        .iter()
        .filter(|s| !s.disable_model_invocation)
        .collect::<Vec<_>>();
    if visible.is_empty() {
        return String::new();
    }
    let mut lines = vec!["\n\nThe following skills provide specialized instructions for specific tasks.".into(), "Use the read tool to load a skill's file when the task matches its description.".into(), "When a skill file references a relative path, resolve it against the skill directory (parent of SKILL.md / dirname of the path) and use that absolute path in tool commands.".into(), String::new(), "<available_skills>".into()];
    for s in visible {
        lines.extend([
            "  <skill>".into(),
            format!("    <name>{}</name>", escape_xml(&s.name)),
            format!(
                "    <description>{}</description>",
                escape_xml(&s.description)
            ),
            format!("    <location>{}</location>", escape_xml(&s.file_path)),
            "  </skill>".into(),
        ]);
    }
    lines.push("</available_skills>".into());
    lines.join("\n")
}
fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn property<'a>(value: &'a FrontmatterValue, key: &str) -> Option<&'a FrontmatterValue> {
    match value {
        FrontmatterValue::Mapping(values) => values
            .iter()
            .find(|(k, _)| matches!(k,FrontmatterValue::String(s) if s == key))
            .map(|(_, v)| v),
        _ => None,
    }
}
fn string(value: &FrontmatterValue) -> Option<&str> {
    if let FrontmatterValue::String(s) = value {
        Some(s)
    } else {
        None
    }
}
fn warning(path: &str, message: String) -> ResourceDiagnostic {
    ResourceDiagnostic {
        r#type: "warning".into(),
        message,
        path: Some(path.into()),
        collision: None,
    }
}

const MAX_NAME_LENGTH: usize = 64;
const MAX_DESCRIPTION_LENGTH: usize = 1024;
fn validate_name(name: &str, parent: &str) -> Vec<String> {
    let mut errors = vec![];
    if name != parent {
        errors.push(format!(
            "name \"{name}\" does not match parent directory \"{parent}\""
        ));
    }
    let length = name.chars().count();
    if length > MAX_NAME_LENGTH {
        errors.push(format!(
            "name exceeds {MAX_NAME_LENGTH} characters ({length})"
        ));
    }
    if name.is_empty()
        || !name
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
    {
        errors.push(
            "name contains invalid characters (must be lowercase a-z, 0-9, hyphens only)".into(),
        );
    }
    if name.starts_with('-') || name.ends_with('-') {
        errors.push("name must not start or end with a hyphen".into());
    }
    if name.contains("--") {
        errors.push("name must not contain consecutive hyphens".into());
    }
    errors
}
fn validate_description(description: &str) -> Vec<String> {
    let length = description.chars().count();
    if paths::trim(description).is_empty() {
        vec!["description is required".into()]
    } else if length > MAX_DESCRIPTION_LENGTH {
        vec![format!(
            "description exceeds {MAX_DESCRIPTION_LENGTH} characters ({length})"
        )]
    } else {
        vec![]
    }
}

fn truthy(value: &FrontmatterValue) -> bool {
    match value {
        FrontmatterValue::Null => false,
        FrontmatterValue::Bool(b) => *b,
        FrontmatterValue::Number(n) => *n != 0.0 && !n.is_nan(),
        FrontmatterValue::String(s) => !s.is_empty(),
        _ => true,
    }
}
/// Loads default directories followed by explicit paths in caller order.
///
/// # Errors
/// Propagates working-directory resolution errors outside filesystem catches.
pub fn load_skills(
    options: LoadSkillsOptions<'_>,
    operations: &dyn ResourceOperations,
) -> Result<LoadSkillsResult, ResourceError> {
    let mut merge = SkillMerge {
        result: LoadSkillsResult::empty(),
        real_paths: std::collections::HashSet::new(),
        collisions: vec![],
        operations,
    };
    let user_dir = paths::join(&[options.agent_dir, "skills"]);
    if options.include_defaults {
        merge.add_skills(load_skills_from_dir_internal(
            &user_dir, "user", true, operations,
        ));
        let project_dir = paths::resolve(
            &[options.cwd, options.config_dir_name, "skills"],
            operations,
        )?;
        merge.add_skills(load_skills_from_dir_internal(
            &project_dir,
            "project",
            true,
            operations,
        ));
    }
    let project_dir = paths::resolve(
        &[options.cwd, options.config_dir_name, "skills"],
        operations,
    )?;
    for raw in options.skill_paths {
        let path = resolve_skill_path(raw, options.cwd, options.home, operations)?;
        if !operations.exists(&path) {
            merge.add_skills(LoadSkillsResult {
                skills: vec![],
                diagnostics: vec![warning(&path, "skill path does not exist".into())],
            });
            continue;
        }
        let sub = load_explicit(&path, &options, (&user_dir, &project_dir), operations);
        merge.add_skills(sub);
    }
    merge.result.diagnostics.extend(merge.collisions);
    Ok(merge.result)
}
struct SkillMerge<'a> {
    result: LoadSkillsResult,
    real_paths: std::collections::HashSet<String>,
    collisions: Vec<ResourceDiagnostic>,
    operations: &'a dyn ResourceOperations,
}
impl SkillMerge<'_> {
    fn add_skills(&mut self, sub: LoadSkillsResult) {
        self.result.diagnostics.extend(sub.diagnostics);
        for skill in sub.skills {
            let real = crate::canonicalize_path(&skill.file_path, self.operations);
            if self.real_paths.contains(&real) {
                continue;
            }
            if let Some(winner) = self.result.skills.iter().find(|s| s.name == skill.name) {
                self.collisions.push(ResourceDiagnostic {
                    r#type: "collision".into(),
                    message: format!("name \"{}\" collision", skill.name),
                    path: Some(skill.file_path.clone()),
                    collision: Some(crate::ResourceCollision {
                        resource_type: "skill".into(),
                        name: skill.name,
                        winner_path: winner.file_path.clone(),
                        loser_path: skill.file_path,
                        winner_source: None,
                        loser_source: None,
                    }),
                });
            } else {
                self.real_paths.insert(real);
                self.result.skills.push(skill);
            }
        }
    }
}
fn source_for<'a>(
    path: &str,
    options: &LoadSkillsOptions<'_>,
    roots: (&str, &str),
    operations: &dyn ResourceOperations,
) -> Result<&'a str, ResourceError> {
    if !options.include_defaults {
        for (dir, source) in [(roots.0, "user"), (roots.1, "project")] {
            let root = paths::resolve(&[dir], operations)?;
            let prefix = format!(
                "{}{}",
                root.trim_end_matches(std::path::MAIN_SEPARATOR),
                std::path::MAIN_SEPARATOR
            );
            if path == root || path.starts_with(&prefix) {
                return Ok(source);
            }
        }
    }
    Ok("path")
}
// Markdown extensions are deliberately case-sensitive.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
fn load_explicit(
    path: &str,
    options: &LoadSkillsOptions<'_>,
    roots: (&str, &str),
    operations: &dyn ResourceOperations,
) -> LoadSkillsResult {
    let loaded = operations.stat(path).and_then(|stats| {
        source_for(path, options, roots, operations).map(|source| (stats, source))
    });
    match loaded {
        Ok((stats, source)) if stats.is_directory => {
            load_skills_from_dir_internal(path, source, true, operations)
        }
        Ok((stats, source)) if stats.is_file && path.ends_with(".md") => {
            load_skill_from_file(path, source, operations)
        }
        Ok(_) => LoadSkillsResult {
            skills: vec![],
            diagnostics: vec![warning(path, "skill path is not a markdown file".into())],
        },
        Err(error) => LoadSkillsResult {
            skills: vec![],
            diagnostics: vec![warning(
                path,
                error
                    .message
                    .unwrap_or_else(|| "failed to read skill path".into()),
            )],
        },
    }
}
fn normalize_path(input: &str, home: &str) -> String {
    let input = paths::trim(input);
    if input == "~" {
        home.into()
    } else if let Some(rest) = input.strip_prefix("~/") {
        paths::join(&[home, rest])
    } else if let Some(rest) = input.strip_prefix('~') {
        paths::join(&[home, rest])
    } else {
        input.into()
    }
}
fn resolve_skill_path(
    input: &str,
    cwd: &str,
    home: &str,
    operations: &dyn ResourceOperations,
) -> Result<String, ResourceError> {
    let normalized = normalize_path(input, home);
    if std::path::Path::new(&normalized).is_absolute() {
        Ok(normalized)
    } else {
        paths::resolve(&[cwd, &normalized], operations)
    }
}
