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

/// Supplied directory and provenance for one scan.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadSkillsFromDirOptions<'a> {
    /// Directory to scan.
    pub dir: &'a str,
    /// Caller-supplied source identifier.
    pub source: &'a str,
}

/// Caller-resolved locations for ordered skill discovery.
#[derive(Debug, Clone, PartialEq)]
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

pub(crate) mod diagnostics;
pub(crate) mod frontmatter;
pub(crate) mod operations;
pub(crate) mod paths;
use crate::{
    FrontmatterValue, ResourceError, ResourceOperations, create_synthetic_source_info,
    parse_frontmatter,
};

/// Loads a supplied directory without cross-source deduplication.
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
fn load_skills_from_dir_internal(
    dir: &str,
    source: &str,
    include_root_files: bool,
    operations: &dyn ResourceOperations,
) -> LoadSkillsResult {
    let mut builder = ignore::gitignore::GitignoreBuilder::new("");
    builder
        .case_insensitive(true)
        .expect("case-insensitive matching is supported");
    scan(
        dir,
        source,
        include_root_files,
        operations,
        &mut builder,
        dir,
    )
}
fn scan(
    dir: &str,
    source: &str,
    include_root_files: bool,
    operations: &dyn ResourceOperations,
    builder: &mut ignore::gitignore::GitignoreBuilder,
    root: &str,
) -> LoadSkillsResult {
    let mut result = LoadSkillsResult {
        skills: vec![],
        diagnostics: vec![],
    };
    if !operations.exists(dir) {
        return result;
    }
    add_ignore_rules(builder, dir, root, operations);
    let matcher = builder
        .build()
        .expect("only valid ignore rules are retained");
    let Ok(entries) = operations.read_dir(dir) else {
        return result;
    };
    for entry in &entries {
        if entry.name != "SKILL.md" {
            continue;
        }
        let path = paths::join(&[dir, &entry.name]);
        let file = if entry.is_symbolic_link {
            operations.stat(&path).map(|s| s.is_file).unwrap_or(false)
        } else {
            entry.is_file
        };
        if !file || ignored(&matcher, &path, root, false) {
            continue;
        }
        return load_skill_from_file(&path, source, operations);
    }
    for entry in entries {
        if entry.name.starts_with('.') || entry.name == "node_modules" {
            continue;
        }
        let path = paths::join(&[dir, &entry.name]);
        let (directory, file) = if entry.is_symbolic_link {
            match operations.stat(&path) {
                Ok(s) => (s.is_directory, s.is_file),
                Err(_) => continue,
            }
        } else {
            (entry.is_directory, entry.is_file)
        };
        let matcher = builder
            .build()
            .expect("only valid ignore rules are retained");
        if ignored(&matcher, &path, root, directory) {
            continue;
        }
        let sub = if directory {
            scan(&path, source, false, operations, builder, root)
        } else if file && include_root_files && entry.name.ends_with(".md") {
            load_skill_from_file(&path, source, operations)
        } else {
            continue;
        };
        result.skills.extend(sub.skills);
        result.diagnostics.extend(sub.diagnostics);
    }
    result
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
    let parsed = operations
        .read_file(file)
        .and_then(|s| parse_frontmatter(&s));
    let parsed = match parsed {
        Ok(p) => p,
        Err(e) => {
            result.diagnostics.push(warning(
                file,
                e.message
                    .unwrap_or_else(|| "failed to parse skill file".into()),
            ));
            return result;
        }
    };
    let description_value = property(&parsed.frontmatter, "description");
    let description = match description_value.filter(|v| truthy(v)) {
        None => "",
        Some(value) => match string(value) {
            Some(s) => s,
            None => {
                result
                    .diagnostics
                    .push(warning(file, "skill description must be a string".into()));
                return result;
            }
        },
    };
    let dir = paths::dirname(file);
    let parent = paths::basename(&dir);
    result.diagnostics.extend(
        validate_description(description)
            .into_iter()
            .map(|m| warning(file, m)),
    );
    let name = match property(&parsed.frontmatter, "name").filter(|v| truthy(v)) {
        None => parent.as_str(),
        Some(value) => match string(value) {
            Some(s) => s,
            None => {
                let message = "skill name must be a string";
                result.diagnostics.push(warning(file, message.into()));
                return result;
            }
        },
    };
    result.diagnostics.extend(
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
        result.skills.push(Skill {
            name: name.into(),
            description: description.into(),
            file_path: file.into(),
            base_dir: dir,
            source_info,
            disable_model_invocation,
        });
    }
    result
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
/// Working-directory resolution errors outside filesystem catches propagate.
pub fn load_skills(
    options: LoadSkillsOptions<'_>,
    operations: &dyn ResourceOperations,
) -> Result<LoadSkillsResult, ResourceError> {
    let mut result = LoadSkillsResult {
        skills: vec![],
        diagnostics: vec![],
    };
    let mut real_paths = std::collections::HashSet::new();
    let mut collisions = vec![];
    let mut add_skills = |sub: LoadSkillsResult| {
        result.diagnostics.extend(sub.diagnostics);
        for skill in sub.skills {
            let real = crate::canonicalize_path(&skill.file_path, operations);
            if real_paths.contains(&real) {
                continue;
            }
            if let Some(winner) = result.skills.iter().find(|s| s.name == skill.name) {
                collisions.push(ResourceDiagnostic {
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
                real_paths.insert(real);
                result.skills.push(skill)
            }
        }
    };
    let user_dir = paths::join(&[options.agent_dir, "skills"]);
    if options.include_defaults {
        add_skills(load_skills_from_dir_internal(
            &user_dir, "user", true, operations,
        ));
        let project_dir = paths::resolve(
            &[options.cwd, options.config_dir_name, "skills"],
            operations,
        )?;
        add_skills(load_skills_from_dir_internal(
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
    let source_for = |path: &str| -> Result<&str, ResourceError> {
        if !options.include_defaults {
            for (dir, source) in [(&user_dir, "user"), (&project_dir, "project")] {
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
    };
    for raw in options.skill_paths {
        let path = resolve_skill_path(raw, options.cwd, options.home, operations)?;
        if !operations.exists(&path) {
            add_skills(LoadSkillsResult {
                skills: vec![],
                diagnostics: vec![warning(&path, "skill path does not exist".into())],
            });
            continue;
        }
        let stats = operations.stat(&path);
        let source = if stats.is_ok() {
            source_for(&path)
        } else {
            Ok("path")
        };
        let sub = match stats.and_then(|stats| source.map(|source| (stats, source))) {
            Ok((stats, source)) if stats.is_directory => {
                load_skills_from_dir_internal(&path, source, true, operations)
            }
            Ok((stats, source)) if stats.is_file && path.ends_with(".md") => {
                load_skill_from_file(&path, source, operations)
            }
            Ok(_) => LoadSkillsResult {
                skills: vec![],
                diagnostics: vec![warning(&path, "skill path is not a markdown file".into())],
            },
            Err(e) => LoadSkillsResult {
                skills: vec![],
                diagnostics: vec![warning(
                    &path,
                    e.message
                        .unwrap_or_else(|| "failed to read skill path".into()),
                )],
            },
        };
        add_skills(sub);
    }
    result.diagnostics.extend(collisions);
    Ok(result)
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
    } else if let Some(s) = line.strip_prefix("\\!") {
        (false, s)
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
) {
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
                let _ = builder.add_line(None, &pattern);
            }
        }
    }
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
