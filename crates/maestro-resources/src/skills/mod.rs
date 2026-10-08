//! Skill discovery through replaceable filesystem operations.

mod operations;
mod prompt;
pub use prompt::format_skills_for_prompt;
mod discovery;
mod validation;
use crate::{
    ResourceDiagnostic, SourceInfo, SourceScope, SyntheticSourceOptions,
    create_synthetic_source_info, parse_frontmatter,
};
#[cfg(not(target_arch = "wasm32"))]
pub use operations::NativeResourceOperations;
pub use operations::{ResourceEntry, ResourceFileType, ResourceOperations};
use std::path::{Path, PathBuf};
pub use validation::SkillFrontmatter;

/// A described instruction file with its provenance.
#[derive(Debug)]
pub struct Skill {
    /// Declared name, or the containing directory name.
    pub name: String,
    /// Authored description.
    pub description: String,
    /// Instruction file location.
    pub file_path: PathBuf,
    /// Directory used for relative instruction paths.
    pub base_dir: PathBuf,
    /// Origin of this file.
    pub source_info: SourceInfo,
    /// Whether this skill is omitted from model-facing prompts.
    pub disable_model_invocation: bool,
}
/// Ordered discoveries and their diagnostics.
#[derive(Debug, Default)]
pub struct LoadSkillsResult {
    /// Retained skills in discovery order.
    pub skills: Vec<Skill>,
    /// Returned diagnostics in validation order.
    pub diagnostics: Vec<ResourceDiagnostic>,
}
/// A supplied directory and its source label.
#[derive(Clone, Copy)]
pub struct LoadSkillsFromDirOptions<'a> {
    /// Directory to scan.
    pub dir: &'a Path,
    /// Provenance label.
    pub source: &'a str,
}
/// Load instruction resources using the supplied filesystem observations.
#[must_use]
pub fn load_skills_from_dir(
    options: LoadSkillsFromDirOptions<'_>,
    operations: &dyn ResourceOperations,
) -> LoadSkillsResult {
    discovery::scan(options.dir, options.source, operations)
}
/// Load one file, retaining its native failure cause as a warning.
fn load_file(path: &Path, source: &str, operations: &dyn ResourceOperations) -> LoadSkillsResult {
    let loaded = operations
        .read_file(path)
        .map_err(|e| e.to_string())
        .and_then(|s| parse_frontmatter(&s).map_err(|e| e.to_string()))
        .and_then(|parsed| {
            validation::project_fields(&parsed.frontmatter).map_err(|e| e.to_string())
        });
    match loaded {
        Ok(fields) => validated_skill(path, source, fields),
        Err(cause) => LoadSkillsResult {
            skills: Vec::new(),
            diagnostics: vec![ResourceDiagnostic::warning(path, cause)],
        },
    }
}
/// Validate supported fields before deciding whether the file is retained.
fn validated_skill(path: &Path, source: &str, fields: SkillFrontmatter) -> LoadSkillsResult {
    let base_dir = path.parent().unwrap_or_else(|| Path::new(""));
    let parent = base_dir.file_name().unwrap_or_default().to_string_lossy();
    let name = fields
        .name
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| parent.into_owned());
    let description = fields.description.unwrap_or_default();
    let diagnostics = validation::description_warnings(&description)
        .into_iter()
        .chain(validation::name_warnings(
            &name,
            &base_dir.file_name().unwrap_or_default().to_string_lossy(),
        ))
        .map(|message| ResourceDiagnostic::warning(path, message))
        .collect();
    let mut result = LoadSkillsResult {
        skills: Vec::new(),
        diagnostics,
    };
    if !description
        .trim_matches(crate::frontmatter::text_whitespace)
        .is_empty()
    {
        let scope = match source {
            "user" => Some(SourceScope::User),
            "project" => Some(SourceScope::Project),
            _ => None,
        };
        let label = if matches!(source, "user" | "project" | "path") {
            "local"
        } else {
            source
        };
        let source_info = create_synthetic_source_info(
            path.into(),
            SyntheticSourceOptions {
                source: label.into(),
                scope,
                origin: None,
                base_dir: Some(base_dir.into()),
            },
        );
        result.skills.push(Skill {
            name,
            description,
            file_path: path.into(),
            base_dir: base_dir.into(),
            source_info,
            disable_model_invocation: fields.disable_model_invocation,
        });
    }
    result
}

/// Supplied roots and explicitly requested skill paths.
#[derive(Clone, Copy)]
pub struct LoadSkillsOptions<'a> {
    /// Working directory for relative paths and project defaults.
    pub cwd: &'a Path,
    /// Caller-resolved home directory.
    pub home: &'a Path,
    /// Caller-resolved user configuration directory.
    pub agent_dir: &'a Path,
    /// Project configuration directory name.
    pub config_dir_name: &'a str,
    /// Explicit files or scan directories in precedence order.
    pub skill_paths: &'a [PathBuf],
    /// Whether user and project defaults precede explicit paths.
    pub include_defaults: bool,
}
/// Load defaults before explicit paths, retaining first names and canonical files.
#[must_use]
pub fn load_skills(
    options: LoadSkillsOptions<'_>,
    operations: &dyn ResourceOperations,
) -> LoadSkillsResult {
    let mut result = LoadSkillsResult::default();
    let mut collisions = Vec::new();
    let mut paths = std::collections::HashSet::new();
    if options.include_defaults {
        for (path, source) in [
            (options.agent_dir.join("skills"), "user"),
            (
                crate::paths::normalize_path(
                    &options.cwd.join(options.config_dir_name).join("skills"),
                ),
                "project",
            ),
        ] {
            add_skills(
                &mut result,
                &mut collisions,
                &mut paths,
                operations,
                discovery::scan(&path, source, operations),
            );
        }
    }
    for path in options.skill_paths {
        let path = crate::paths::resolve_skill_path(path, options.cwd, options.home);
        let source = if !options.include_defaults
            && path.starts_with(crate::paths::normalize_path(
                &options.agent_dir.join("skills"),
            )) {
            "user"
        } else if !options.include_defaults
            && path.starts_with(crate::paths::normalize_path(
                &options.cwd.join(options.config_dir_name).join("skills"),
            ))
        {
            "project"
        } else {
            "path"
        };
        let loaded = explicit_path(&path, source, operations);
        add_skills(&mut result, &mut collisions, &mut paths, operations, loaded);
    }
    result.diagnostics.extend(collisions);
    result
}
/// Merge discoveries, retaining the first name and delaying collision messages.
fn add_skills(
    result: &mut LoadSkillsResult,
    collisions: &mut Vec<ResourceDiagnostic>,
    paths: &mut std::collections::HashSet<PathBuf>,
    operations: &dyn ResourceOperations,
    loaded: LoadSkillsResult,
) {
    result.diagnostics.extend(loaded.diagnostics);
    for skill in loaded.skills {
        let real_path = crate::canonicalize_path(&skill.file_path, operations);
        if paths.contains(&real_path) {
            continue;
        }
        if let Some(winner) = result
            .skills
            .iter()
            .find(|winner| winner.name == skill.name)
        {
            collisions.push(ResourceDiagnostic {
                r#type: crate::DiagnosticType::Collision,
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
            paths.insert(real_path);
            result.skills.push(skill);
        }
    }
}
/// Dispatch a supplied path, keeping missing, wrong-kind and native errors distinct.
fn explicit_path(
    path: &Path,
    source: &str,
    operations: &dyn ResourceOperations,
) -> LoadSkillsResult {
    let error = if operations.exists(path) {
        match operations.metadata(path) {
            Ok(ResourceFileType::Directory) => return discovery::scan(path, source, operations),
            Ok(ResourceFileType::File) if path.to_string_lossy().ends_with(".md") => {
                return load_file(path, source, operations);
            }
            Ok(_) => "skill path is not a markdown file".into(),
            Err(cause) => cause.to_string(),
        }
    } else {
        "skill path does not exist".into()
    };
    LoadSkillsResult {
        skills: Vec::new(),
        diagnostics: vec![ResourceDiagnostic::warning(path, error)],
    }
}
